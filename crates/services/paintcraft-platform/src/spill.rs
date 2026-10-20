//! Session-only append segments. Recovery checkpoints have a separate lifecycle.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    storage::{SpillBlob, SpillStore, SpillUsage},
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use uuid::Uuid;
const MAGIC: &[u8; 8] = b"PCSPIL01";
const HEADER: u64 = 48;
const MAX_BLOB: usize = 256 * 256 * 4;
fn private_file(path: &Path) -> std::io::Result<File> {
    let mut options = File::options();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
fn private_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
fn storage(e: std::io::Error) -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        format!("history segment I/O: {}", e.kind()),
    )
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "history disk allowance exhausted")
}
fn corrupt() -> Error {
    Error::new(
        ErrorCode::CodecFailed,
        "history segment integrity check failed",
    )
}

#[derive(Debug)]
struct FileRecord {
    bytes: u64,
    lease: Weak<Segment>,
}
#[derive(Debug)]
struct Registry {
    files: HashMap<PathBuf, FileRecord>,
    used: u64,
    failures: u64,
}
#[derive(Debug)]
struct Owner {
    path: PathBuf,
    lock: Option<File>,
    registry: Mutex<Registry>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        drop(self.lock.take());
        let _ = fs::remove_dir_all(&self.path);
    }
}
#[derive(Debug)]
struct Segment {
    dirty: std::sync::atomic::AtomicBool,
    path: PathBuf,
    owner: Arc<Owner>,
}
impl Drop for Segment {
    fn drop(&mut self) {
        let mut r = self
            .owner
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match fs::remove_file(&self.path) {
            Ok(()) => {
                if let Some(bytes) = r.files.remove(&self.path) {
                    r.used -= bytes.bytes;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(bytes) = r.files.remove(&self.path) {
                    r.used -= bytes.bytes;
                }
            }
            Err(_) => r.failures += 1,
        }
    }
}
#[derive(Debug)]
struct Appender {
    file: File,
    segment: Arc<Segment>,
    length: u64,
}
#[derive(Debug)]
struct Extent {
    segment: Arc<Segment>,
    offset: u64,
    length: usize,
    digest: [u8; 32],
}
impl SpillBlob for Extent {
    fn byte_len(&self) -> usize {
        self.length
    }
    fn read_into(&self, out: &mut [u8]) -> Result<()> {
        if out.len() != self.length {
            return Err(corrupt());
        }
        let mut file = File::open(&self.segment.path).map_err(storage)?;
        file.seek(SeekFrom::Start(self.offset)).map_err(storage)?;
        let mut header = [0u8; 48];
        file.read_exact(&mut header).map_err(storage)?;
        if &header[..8] != MAGIC
            || u64::from_le_bytes(header[8..16].try_into().unwrap()) != self.length as u64
            || header[16..] != self.digest
        {
            return Err(corrupt());
        }
        file.read_exact(out).map_err(storage)?;
        if <[u8; 32]>::from(Sha256::digest(out)) != self.digest {
            return Err(corrupt());
        }
        Ok(())
    }
}
/// Each store owns a unique locked directory; handles can outlive the store.
#[derive(Debug)]
pub struct NativeSpillStore {
    owner: Arc<Owner>,
    active: Mutex<Option<Appender>>,
    quota: u64,
    segment_target: u64,
    #[cfg(test)]
    fail_body: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_rollback: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_flush: std::sync::atomic::AtomicBool,
}
impl NativeSpillStore {
    pub fn new(parent: &Path, quota: u64, segment_target: u64) -> Result<Self> {
        fs::create_dir_all(parent).map_err(storage)?;
        let path = parent.join(format!("session-{}", Uuid::new_v4()));
        private_directory(&path).map_err(storage)?;
        let result = (|| {
            let mut lock = private_file(&path.join("owner.lock")).map_err(storage)?;
            lock.try_lock()
                .map_err(|_| Error::new(ErrorCode::StorageFailed, "cannot lock history store"))?;
            lock.write_all(MAGIC).map_err(storage)?;
            lock.sync_all().map_err(storage)?;
            Ok(Self {
                owner: Arc::new(Owner {
                    path: path.clone(),
                    lock: Some(lock),
                    registry: Mutex::new(Registry {
                        files: HashMap::new(),
                        used: 0,
                        failures: 0,
                    }),
                }),
                active: Mutex::new(None),
                quota,
                segment_target: segment_target.max(HEADER + 1),
                #[cfg(test)]
                fail_body: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                fail_rollback: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                fail_flush: std::sync::atomic::AtomicBool::new(false),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(path);
        }
        result
    }
    pub fn directory(&self) -> &Path {
        &self.owner.path
    }
    fn flush_active(&self, active: &mut Option<Appender>) -> Result<()> {
        let Some(app) = active.as_mut() else {
            return Ok(());
        };
        if !app.segment.dirty.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        #[cfg(test)]
        let fail = self
            .fail_flush
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        #[cfg(not(test))]
        let fail = false;
        let result = if fail {
            Err(std::io::ErrorKind::StorageFull.into())
        } else {
            app.file.sync_all()
        };
        if let Err(error) = result {
            drop(active.take());
            return Err(storage(error));
        }
        app.segment
            .dirty
            .store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }
    fn appender(&self) -> Result<Appender> {
        let path = self.owner.path.join(format!("{}.segment", Uuid::new_v4()));
        let mut r = self
            .owner
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        r.files.try_reserve(1).map_err(|_| limit())?;
        let file = private_file(&path).map_err(storage)?;
        let segment = Arc::new(Segment {
            dirty: std::sync::atomic::AtomicBool::new(false),
            path: path.clone(),
            owner: self.owner.clone(),
        });
        r.files.insert(
            path,
            FileRecord {
                bytes: 0,
                lease: Arc::downgrade(&segment),
            },
        );
        Ok(Appender {
            file,
            segment,
            length: 0,
        })
    }
}
impl SpillStore for NativeSpillStore {
    fn write(&self, bytes: &[u8]) -> Result<Arc<dyn SpillBlob>> {
        if bytes.is_empty() || bytes.len() > MAX_BLOB {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "history extent size out of bounds",
            ));
        }
        let total = HEADER + bytes.len() as u64;
        let mut guard = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if guard
            .as_ref()
            .is_some_and(|a| a.length != 0 && a.length.saturating_add(total) > self.segment_target)
        {
            self.flush_active(&mut guard)?;
            drop(guard.take());
        }
        if guard.is_none() {
            *guard = Some(self.appender()?);
        }
        let app = guard.as_mut().unwrap();
        {
            let mut r = self
                .owner
                .registry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if r.used.checked_add(total).is_none_or(|n| n > self.quota) {
                return Err(limit());
            }
            r.used += total;
            r.files.get_mut(&app.segment.path).unwrap().bytes += total;
        }
        let offset = app.length;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        app.segment
            .dirty
            .store(true, std::sync::atomic::Ordering::Release);
        let result: std::io::Result<()> = (|| {
            app.file.seek(SeekFrom::Start(offset))?;
            app.file.write_all(MAGIC)?;
            app.file.write_all(&(bytes.len() as u64).to_le_bytes())?;
            app.file.write_all(&digest)?;
            #[cfg(test)]
            if self
                .fail_body
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err(std::io::ErrorKind::StorageFull.into());
            }
            app.file.write_all(bytes)?;
            Ok(())
        })();
        if let Err(error) = result {
            #[cfg(test)]
            let rollback_allowed = !self
                .fail_rollback
                .swap(false, std::sync::atomic::Ordering::SeqCst);
            #[cfg(not(test))]
            let rollback_allowed = true;
            if rollback_allowed && app.file.set_len(offset).is_ok() {
                let mut r = self
                    .owner
                    .registry
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                r.used -= total;
                r.files.get_mut(&app.segment.path).unwrap().bytes -= total;
            }
            drop(guard.take());
            return Err(storage(error));
        }
        app.length += total;
        Ok(Arc::new(Extent {
            segment: app.segment.clone(),
            offset,
            length: bytes.len(),
            digest,
        }))
    }
    fn flush(&self) -> Result<()> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        // A collector can close an uncommitted append file while another caller
        // still owns provisional extents. Track dirty segments beyond the writer.
        // Keep upgraded leases until after releasing the registry lock.
        let mut segments = Vec::new();
        {
            let r = self
                .owner
                .registry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            segments
                .try_reserve_exact(r.files.len())
                .map_err(|_| limit())?;
            for record in r.files.values() {
                if let Some(segment) = record.lease.upgrade() {
                    segments.push(segment);
                }
            }
        }
        for segment in segments {
            if !segment.dirty.load(std::sync::atomic::Ordering::Acquire) {
                continue;
            }
            #[cfg(test)]
            let fail = self
                .fail_flush
                .swap(false, std::sync::atomic::Ordering::SeqCst);
            #[cfg(not(test))]
            let fail = false;
            let result = if fail {
                Err(std::io::ErrorKind::StorageFull.into())
            } else if let Some(app) = active
                .as_ref()
                .filter(|a| Arc::ptr_eq(&a.segment, &segment))
            {
                app.file.sync_all()
            } else {
                File::options()
                    .read(true)
                    .write(true)
                    .open(&segment.path)
                    .and_then(|f| f.sync_all())
            };
            if let Err(error) = result {
                drop(active.take());
                return Err(storage(error));
            }
            segment
                .dirty
                .store(false, std::sync::atomic::Ordering::Release);
        }
        Ok(())
    }
    fn collect_garbage(&self) -> Result<SpillUsage> {
        drop(self.active.lock().unwrap_or_else(|e| e.into_inner()).take());
        let mut r = self
            .owner
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut reclaimed = 0;
        let mut failures = 0;
        let mut error = None;
        r.files.retain(|path, record| {
            if record.lease.strong_count() != 0 {
                return true;
            }
            match fs::remove_file(path) {
                Ok(()) => {
                    reclaimed += record.bytes;
                    false
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    reclaimed += record.bytes;
                    false
                }
                Err(e) => {
                    failures += 1;
                    if error.is_none() {
                        error = Some(e);
                    }
                    true
                }
            }
        });
        r.used -= reclaimed;
        r.failures += failures;
        if let Some(error) = error {
            return Err(storage(error));
        }
        Ok(SpillUsage {
            reserved_bytes: r.used,
            segments: r.files.len(),
            cleanup_failures: r.failures,
        })
    }
    fn usage(&self) -> SpillUsage {
        let r = self
            .owner
            .registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        SpillUsage {
            reserved_bytes: r.used,
            segments: r.files.len(),
            cleanup_failures: r.failures,
        }
    }
}

/// Remove only marked, unlocked session directories left by terminated owners.
pub fn remove_abandoned(parent: &Path) -> Result<usize> {
    let entries = match fs::read_dir(parent) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(storage(e)),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(storage)?;
        if !entry.file_type().map_err(storage)?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name
            .strip_prefix("session-")
            .and_then(|n| Uuid::parse_str(n).ok())
            .is_none()
        {
            continue;
        }
        let path = entry.path();
        let mut lock = match File::options()
            .read(true)
            .write(true)
            .open(path.join("owner.lock"))
        {
            Ok(f) => f,
            Err(_) => continue,
        };
        if lock.try_lock().is_err() {
            continue;
        }
        let mut marker = [0; 8];
        if lock.read_exact(&mut marker).is_err() || &marker != MAGIC {
            continue;
        }
        drop(lock);
        fs::remove_dir_all(path).map_err(storage)?;
        removed += 1;
    }
    Ok(removed)
}
#[cfg(test)]
mod tests;
