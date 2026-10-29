//! Private, checksummed recovery journal. Immutable payloads become durable before
//! commit segments, then alternating heads publish a validated journal position.
//! A head anchors a full semantic snapshot; only contiguous backed tail records
//! may advance it. Recovery restores content, not the entire undo history.
mod admission;
mod cleanup;
mod journal;
mod lease;
mod reader;
use crate::save;
use journal::{
    Commit, Head, RECORD_MAX, SNAPSHOT_BYTES_MAX, TAIL_RECORDS_MAX, commit_path, hash, head_path,
    payload_path, read_bounded,
};
pub use lease::{ClaimedRecovery, RecoveryLease};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
};
use paintcraft_document::Document;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};
use uuid::Uuid;
fn storage(what: &str, e: &std::io::Error) -> Error {
    Error::new(ErrorCode::StorageFailed, format!("{what}: {}", e.kind()))
}
/// Where checkpoints live. `PAINTCRAFT_RECOVERY_DIR` overrides the platform default.
pub fn default_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("PAINTCRAFT_RECOVERY_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        home.unwrap_or_default()
            .join("Library/Application Support/PaintCraft/recovery")
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("PaintCraft/recovery")
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".local/state")))
            .unwrap_or_default()
            .join("paintcraft/recovery")
    }
}

/// Limits are provisional disk/admission policy, not a performance qualification.
const RETAINED_BYTES_MAX: u64 = 4 << 30;
const OWNED_FILES_MAX: usize = 256;
#[derive(Debug, Default)]
struct JournalState {
    sequence: u64,
    previous: Option<String>,
    heads: [Option<Head>; 2],
    retained: Vec<Commit>,
    orphans: Vec<(Uuid, u64)>,
    blocked: bool,
    cleared: bool,
}
#[derive(Debug)]
pub struct Recovery {
    dir: PathBuf,
    session: Uuid,
    state: Mutex<JournalState>,
    epoch: AtomicU64,
    _lock: File,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub session: Uuid,
    pub checkpoint: PathBuf,
    pub original: Option<String>,
    pub modified: SystemTime,
    /// Zero denotes a validated legacy standalone checkpoint.
    pub sequence: u64,
    pub document_id: Uuid,
    pub state_id: Uuid,
    pub sha256: String,
}
/// Owner-scoped publication fence. A coordinator invalidates first, then queues
/// cleanup before admitting work under the returned token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryToken {
    session: Uuid,
    epoch: u64,
}

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub sequence: u64,
    pub state_id: Uuid,
    pub completed: SystemTime,
    pub warnings: Vec<String>,
}
#[derive(Debug, Default)]
pub struct ScanReport {
    /// Future versions/features are retained without falling back to older content.
    pub unsupported_sessions: usize,
    pub found: Vec<Found>,
    pub issues: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    BeforePayload,
    AfterPayload,
    AfterCommit,
    AfterHead,
    BeforeGc,
}

/// Publish newly created directory entries from the leaf through the first
/// existing parent. Later payload barriers may rely on this namespace surviving.
pub(crate) fn ensure_directory(dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for ancestor in dir.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => {
                return Err(Error::new(
                    ErrorCode::StorageFailed,
                    "recovery parent is not a directory",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing.push(ancestor),
            Err(error) => return Err(storage("cannot inspect recovery parent", &error)),
        }
    }
    fs::create_dir_all(dir).map_err(|e| storage("cannot create recovery directory", &e))?;
    for created in missing {
        sync_directory(created)?;
        let parent = created
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_directory(dir: &Path) -> Result<()> {
    File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| storage("recovery directory sync failed", &e))
}
fn outcome(out: &save::Outcome, what: &str) -> Result<()> {
    if out.is_success() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::StorageFailed,
            format!("{what}: {}", out.reasons.join("; ")),
        ))
    }
}
fn tombstone(dir: &Path, session: Uuid) -> PathBuf {
    dir.join(format!("{session}.cleared"))
}
fn cleared_bytes(session: Uuid) -> Vec<u8> {
    json::canonical(&Value::object([
        ("schema", Value::str("paintcraft.recovery-cleared/v1")),
        ("session", Value::str(session.to_string())),
    ]))
}
fn owned(name: &str, session: Uuid) -> bool {
    if let Some((stem, temporary)) = name
        .strip_prefix('.')
        .and_then(|name| name.rsplit_once(".paintcraft-"))
        && temporary
            .strip_suffix(".tmp")
            .is_some_and(|id| Uuid::parse_str(id).is_ok_and(|uuid| uuid.simple().to_string() == id))
    {
        return owned(stem, session);
    }
    let prefix = format!("{session}.");
    let Some(suffix) = name.strip_prefix(&prefix) else {
        return false;
    };
    if matches!(
        suffix,
        "lock" | "json" | "pcnd" | "cleared" | "head-0.json" | "head-1.json" | "save.json"
    ) {
        return true;
    }
    if let Some(sequence) = suffix
        .strip_prefix("journal-")
        .and_then(|s| s.strip_suffix(".json"))
    {
        return sequence.len() == 20
            && sequence
                .parse::<u64>()
                .is_ok_and(|n| n > 0 && format!("{n:020}") == sequence);
    }
    suffix
        .strip_prefix("payload-")
        .and_then(|s| s.strip_suffix(".pcnd"))
        .is_some_and(|s| Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s))
}
fn owned_paths(dir: &Path, session: Uuid) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (count, entry) in fs::read_dir(dir)
        .map_err(|e| storage("cannot list recovery files", &e))?
        .enumerate()
    {
        if count >= 100_000 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "recovery directory scan limit reached",
            ));
        }
        let entry = entry.map_err(|e| storage("cannot inspect recovery entry", &e))?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|s| owned(s, session))
        {
            if paths.len() >= OWNED_FILES_MAX {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "recovery session file limit reached",
                ));
            }
            paths.push(entry.path());
        }
    }
    Ok(paths)
}
fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(storage("cannot remove recovery file", &e)),
    }
}
impl Recovery {
    pub fn start(dir: &Path) -> Result<Self> {
        ensure_directory(dir)?;
        let session = Uuid::new_v4();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.join(format!("{session}.lock")))
            .map_err(|e| storage("cannot create recovery lock", &e))?;
        lock.try_lock().map_err(|_| {
            Error::new(
                ErrorCode::StorageFailed,
                "cannot acquire recovery session lock",
            )
        })?;
        sync_directory(dir)?;
        Ok(Self {
            dir: dir.to_owned(),
            session,
            state: Mutex::new(JournalState::default()),
            epoch: AtomicU64::new(0),
            _lock: lock,
        })
    }
    pub fn session(&self) -> Uuid {
        self.session
    }
    pub fn token(&self) -> RecoveryToken {
        RecoveryToken {
            session: self.session,
            epoch: self.epoch.load(Ordering::Acquire),
        }
    }
    pub fn is_current(&self, token: RecoveryToken) -> bool {
        token.session == self.session && token.epoch == self.epoch.load(Ordering::Acquire)
    }
    pub fn invalidate(&self) -> Result<RecoveryToken> {
        let previous = self
            .epoch
            .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "recovery epoch exhausted"))?;
        Ok(RecoveryToken {
            session: self.session,
            epoch: previous + 1,
        })
    }
    fn check_token(&self, token: RecoveryToken) -> Result<()> {
        if self.is_current(token) {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "recovery publication was superseded",
            ))
        }
    }

    /// Compatibility entry points use the existing process-owned metadata realm.
    pub fn checkpoint(&self, bytes: &[u8], original: Option<&Path>) -> Result<()> {
        admission::native(|metadata| {
            self.checkpoint_with_metadata_admission_checked(bytes, original, metadata, &|| Ok(()))
        })
    }
    pub fn checkpoint_with_metadata_admission_checked(
        &self,
        bytes: &[u8],
        original: Option<&Path>,
        metadata: crate::pcnd::MetadataAdmission<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if bytes.len() as u64 > SNAPSHOT_BYTES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "recovery snapshot exceeds its limit",
            ));
        }
        let context = admission::Context::new(metadata, check);
        let document = context.read(bytes)?;
        context.check()?;
        // This final check admits the existing durable publication transaction.
        // Later owner-token fences retain their original crash/partial-write rules;
        // do not add a caller cancellation after reclamation has already begun.
        self.checkpoint_document(&document, original).map(|_| ())
    }
    pub fn read_checkpoint(path: &Path) -> Result<Document> {
        admission::native(|metadata| {
            Self::read_checkpoint_with_metadata_admission_checked(path, metadata, &|| Ok(()))
        })
    }
    pub fn read_checkpoint_with_metadata_admission_checked(
        path: &Path,
        metadata: crate::pcnd::MetadataAdmission<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Document> {
        let context = admission::Context::new(metadata, check);
        let document = context.read(&context.file(path, SNAPSHOT_BYTES_MAX)?)?;
        context.check()?;
        Ok(document)
    }

    pub fn checkpoint_document(
        &self,
        document: &Document,
        original: Option<&Path>,
    ) -> Result<Checkpoint> {
        self.checkpoint_document_at(self.token(), document, original)
    }
    pub fn checkpoint_document_at(
        &self,
        token: RecoveryToken,
        document: &Document,
        original: Option<&Path>,
    ) -> Result<Checkpoint> {
        self.checkpoint_at_impl(token, document, original, |_| Ok(()))
    }
    #[cfg(test)]
    fn checkpoint_impl(
        &self,
        document: &Document,
        original: Option<&Path>,
        barrier: impl FnMut(Phase) -> Result<()>,
    ) -> Result<Checkpoint> {
        self.checkpoint_at_impl(self.token(), document, original, barrier)
    }
    fn checkpoint_at_impl(
        &self,
        token: RecoveryToken,
        document: &Document,
        original: Option<&Path>,
        mut barrier: impl FnMut(Phase) -> Result<()>,
    ) -> Result<Checkpoint> {
        self.check_token(token)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "recovery coordinator poisoned"))?;
        self.check_token(token)?;
        if state.blocked {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "recovery requires reconciliation after an uncertain journal write",
            ));
        }
        if state.cleared {
            remove_if_present(&tombstone(&self.dir, self.session))?;
            sync_directory(&self.dir)?;
            state.cleared = false;
        }
        // Reclaim only unreferenced payloads. A committed payload is never removed
        // here, even if its head publication failed.
        state.orphans.retain(|(id, _)| {
            remove_if_present(&payload_path(&self.dir, self.session, *id)).is_err()
        });
        let retained = state
            .retained
            .iter()
            .map(|c| c.bytes)
            .chain(state.orphans.iter().map(|(_, bytes)| *bytes))
            .try_fold(0u64, u64::checked_add)
            .ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "recovery byte charge overflows")
            })?;
        let remaining = RETAINED_BYTES_MAX
            .saturating_sub(retained)
            .min(SNAPSHOT_BYTES_MAX);
        if remaining == 0 || state.retained.len() >= TAIL_RECORDS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "recovery retention exhausted before head publication",
            ));
        }
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "recovery sequence exhausted"))?;
        let payload = Uuid::new_v4();
        let path = payload_path(&self.dir, self.session, payload);
        barrier(Phase::BeforePayload)?;
        self.check_token(token)?;
        let mut encoding_error = None;
        let written = save::save_with(&path, false, remaining, |sink| {
            crate::pcnd::write_to(document, sink).inspect_err(|e| encoding_error = Some(e.clone()))
        });
        if let Some(error) = encoding_error {
            return Err(error);
        }
        if !written.is_success() {
            state.blocked = matches!(
                written.replacement,
                save::Replacement::Replaced | save::Replacement::Unknown
            );
            // No journal reference exists yet. Retain uncertain output for diagnosis.
            outcome(&written, "recovery payload not durably saved")?;
        }
        let bytes = fs::metadata(&path)
            .map_err(|e| storage("cannot inspect saved recovery payload", &e))?
            .len();
        state.orphans.push((payload, bytes));
        barrier(Phase::AfterPayload)?;
        self.check_token(token)?;
        let commit = Commit {
            sequence,
            previous: state.previous.clone(),
            payload,
            bytes,
            sha256: written.staged_sha256.expect("successful staged digest"),
            document: document.document_id,
            state: document.state_id,
            original: original.map(|p| p.to_string_lossy().into_owned()),
        };
        let record = commit.encode();
        Commit::decode(&record)?; // Check record/path/size policy before publishing.
        let committed = save::save(
            &commit_path(&self.dir, self.session, sequence),
            &record,
            false,
        );
        if !committed.is_success() {
            state.blocked = matches!(
                committed.replacement,
                save::Replacement::Replaced | save::Replacement::Unknown
            );
            return outcome(&committed, "recovery journal commit not durable")
                .map(|_| unreachable!());
        }
        state.orphans.retain(|(id, _)| *id != payload);
        state.retained.push(commit);
        state.sequence = sequence;
        state.previous = Some(hash(&record));
        barrier(Phase::AfterCommit)?;
        self.check_token(token)?;
        let head = Head {
            sequence,
            commit_sha256: hash(&record),
        };
        let slot = sequence % 2;
        let head_path = head_path(&self.dir, self.session, slot);
        if fs::symlink_metadata(&head_path).is_ok_and(|m| !m.is_file()) {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "invalid recovery head file type",
            ));
        }
        outcome(
            &save::save(&head_path, &head.encode(), true),
            "recovery head not durable",
        )?;
        state.heads[slot as usize] = Some(head);
        barrier(Phase::AfterHead)?;
        barrier(Phase::BeforeGc)?;
        let completed = SystemTime::now();
        let cleanup = (|| -> Result<()> {
            let floor = state
                .heads
                .iter()
                .flatten()
                .map(|h| h.sequence)
                .min()
                .expect("published head");
            let obsolete: Vec<_> = state
                .retained
                .iter()
                .filter(|old| old.sequence < floor)
                .cloned()
                .collect();
            for old in &obsolete {
                remove_if_present(&commit_path(&self.dir, self.session, old.sequence))?;
            }
            // Persist reference removal before reclaiming its payload, so an interrupted
            // GC cannot leave a durable segment pointing to deleted data.
            sync_directory(&self.dir)?;
            for old in &obsolete {
                remove_if_present(&payload_path(&self.dir, self.session, old.payload))?;
            }
            sync_directory(&self.dir)?;
            state.retained.retain(|old| old.sequence >= floor);
            Ok(())
        })();
        Ok(Checkpoint {
            sequence,
            state_id: document.state_id,
            completed,
            warnings: cleanup
                .err()
                .map(|e| format!("recovery garbage collection pending: {e}"))
                .into_iter()
                .collect(),
        })
    }
    /// Record discard durably before cleanup so an interrupted cleanup cannot revive it.
    pub fn clear(&self) -> Result<()> {
        self.clear_at(self.invalidate()?)
    }
    /// Called in the same FIFO before work using this token. Stale cleanup cannot
    /// remove a newer episode. Invalidation itself never waits for filesystem work.
    pub fn clear_at(&self, token: RecoveryToken) -> Result<()> {
        self.check_token(token)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "recovery coordinator poisoned"))?;
        self.check_token(token)?;
        if let Err(error) = clear_owned(&self.dir, self.session) {
            state.blocked = true;
            return Err(error);
        }
        *state = JournalState {
            cleared: true,
            ..Default::default()
        };
        Ok(())
    }
    pub fn scan(dir: &Path) -> Vec<Found> {
        Self::scan_report(dir).found
    }
    pub fn scan_report(dir: &Path) -> ScanReport {
        admission::native(|metadata| {
            Self::scan_report_with_metadata_admission_checked(dir, metadata, &|| Ok(()))
        })
        .unwrap_or_else(|error| ScanReport {
            issues: vec![format!("Recovery inspection retained its files: {error}")],
            ..Default::default()
        })
    }
    pub fn scan_report_with_metadata_admission_checked(
        dir: &Path,
        metadata: crate::pcnd::MetadataAdmission<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ScanReport> {
        let context = admission::Context::new(metadata, check);
        let mut cleanup = cleanup::Batch::default();
        let mut report = reader::scan_bounded(dir, 100_000, &context, &mut cleanup)?;
        let saves = dir.join(crate::save_recovery::DIRECTORY);
        match fs::symlink_metadata(&saves) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                let extra = reader::scan_bounded(
                    &saves,
                    crate::save_recovery::FILES_MAX,
                    &context,
                    &mut cleanup,
                )?;
                report.found.extend(extra.found);
                report.issues.extend(extra.issues);
                report.unsupported_sessions += extra.unsupported_sessions;
            }
            Ok(_) => report
                .issues
                .push("Invalid saved-attempt recovery directory was retained".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => report.issues.push(format!(
                "Cannot inspect saved-attempt recovery: {}",
                error.kind()
            )),
        }
        report.found.sort_by_key(|found| found.modified);
        context.check()?;
        cleanup.commit(&mut report.issues);
        Ok(report)
    }
    /// Refuse to remove a live session's store.
    pub fn discard(dir: &Path, session: Uuid) -> Result<()> {
        RecoveryLease::acquire(dir, session)?.discard()
    }
}
fn clear_owned(dir: &Path, session: Uuid) -> Result<()> {
    let marker = tombstone(dir, session);
    if fs::symlink_metadata(&marker).is_ok_and(|m| !m.is_file()) {
        return Err(Error::new(
            ErrorCode::TargetInvalid,
            "invalid recovery discard marker type",
        ));
    }
    outcome(
        &save::save(&marker, &cleared_bytes(session), true),
        "recovery discard not durable",
    )?;
    for path in owned_paths(dir, session)? {
        if path == marker
            || path.file_name().and_then(|n| n.to_str()) == Some(format!("{session}.lock").as_str())
        {
            continue;
        }
        remove_if_present(&path)?;
    }
    sync_directory(dir)
}
impl Drop for Recovery {
    fn drop(&mut self) {
        if owned_paths(&self.dir, self.session).is_ok_and(|paths| {
            paths.iter().all(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n == format!("{}.lock", self.session)
                        || n == format!("{}.cleared", self.session)
                })
            })
        }) {
            let _ = remove_if_present(&tombstone(&self.dir, self.session));
            let _ = remove_if_present(&self.dir.join(format!("{}.lock", self.session)));
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod metadata_tests;
