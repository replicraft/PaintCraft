//! Crash recovery checkpoints.
//!
//! Each editing session owns `<session>.lock` (held with an advisory lock for
//! as long as the process lives), `<session>.pcnd` (the latest checkpoint) and
//! `<session>.json` (which file the document came from). A checkpoint is
//! replaced atomically, so a crash at any moment leaves either the previous
//! complete checkpoint or the new one. A session whose lock can be taken is
//! orphaned: its owner is gone, so its checkpoint is offered for recovery.
//! Checkpoints are deliberately separate from the user's file and from undo
//! history: recovery restores the latest document, not the history.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use paintcraft_types::json::{self, Value};
use paintcraft_types::{Error, ErrorCode, Result};
use uuid::Uuid;

use crate::save;

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

#[derive(Debug)]
pub struct Recovery {
    dir: PathBuf,
    session: Uuid,
    /// Held for the life of the session; dropping it marks the session orphaned.
    _lock: File,
}

/// An orphaned checkpoint that can be recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub session: Uuid,
    pub checkpoint: PathBuf,
    /// The file the document was opened from or saved to, if any.
    pub original: Option<String>,
    pub modified: SystemTime,
}

impl Recovery {
    /// Begin a session: create the directory and take this session's lock.
    pub fn start(dir: &Path) -> Result<Recovery> {
        fs::create_dir_all(dir).map_err(|e| storage("cannot create the recovery directory", &e))?;
        let session = Uuid::new_v4();
        let lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(format!("{session}.lock")))
            .map_err(|e| storage("cannot create the session lock", &e))?;
        lock.try_lock().map_err(|e| {
            storage(
                "cannot lock the session",
                &std::io::Error::other(e.to_string()),
            )
        })?;
        Ok(Recovery {
            dir: dir.to_path_buf(),
            session,
            _lock: lock,
        })
    }

    pub fn session(&self) -> Uuid {
        self.session
    }

    fn path(&self, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{ext}", self.session))
    }

    /// Atomically replace this session's checkpoint with `pcnd_bytes`.
    pub fn checkpoint(&self, pcnd_bytes: &[u8], original: Option<&Path>) -> Result<()> {
        let meta = json::canonical(&Value::object([(
            "original",
            original.map_or(Value::Null, |p| Value::str(p.to_string_lossy())),
        )]));
        let outcome = save::save(&self.path("json"), &meta, true);
        if !outcome.is_success() {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                format!(
                    "recovery metadata not saved: {}",
                    outcome.reasons.join("; ")
                ),
            ));
        }
        let outcome = save::save(&self.path("pcnd"), pcnd_bytes, true);
        if outcome.is_success() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::StorageFailed,
                format!("checkpoint not saved: {}", outcome.reasons.join("; ")),
            ))
        }
    }

    /// The document was saved or closed cleanly: nothing is left to recover.
    pub fn clear(&self) {
        for ext in ["pcnd", "json"] {
            let _ = fs::remove_file(self.path(ext));
        }
    }

    /// Orphaned sessions that have a checkpoint, oldest first.
    pub fn scan(dir: &Path) -> Vec<Found> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "lock") {
                continue;
            }
            let Some(session) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            let Ok(lock) = OpenOptions::new().write(true).open(&path) else {
                continue;
            };
            if lock.try_lock().is_err() {
                continue; // a live session owns it
            }
            let checkpoint = dir.join(format!("{session}.pcnd"));
            let Ok(meta) = fs::metadata(&checkpoint) else {
                // Orphaned with nothing to recover: tidy up.
                let _ = fs::remove_file(&path);
                let _ = fs::remove_file(dir.join(format!("{session}.json")));
                continue;
            };
            let original = fs::read(dir.join(format!("{session}.json")))
                .ok()
                .and_then(|b| json::parse(&b).ok())
                .and_then(|v| v.get("original").and_then(Value::as_str).map(str::to_owned));
            found.push(Found {
                session,
                checkpoint,
                original,
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
        found.sort_by_key(|f| f.modified);
        found
    }

    /// Delete an orphaned session's files (the user declined or finished recovery).
    pub fn discard(dir: &Path, session: Uuid) {
        for ext in ["pcnd", "json", "lock"] {
            let _ = fs::remove_file(dir.join(format!("{session}.{ext}")));
        }
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        // The lock file goes with a clean end of session; a checkpoint that was not
        // cleared stays behind (it has no lock, so it is offered next start).
        if !self.path("pcnd").exists() {
            let _ = fs::remove_file(self.path("lock"));
            let _ = fs::remove_file(self.path("json"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "paintcraft-recovery-{name}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_live_session_is_not_offered_but_a_crashed_one_is() {
        let dir = scratch("crash");
        let r = Recovery::start(&dir).unwrap();
        r.checkpoint(b"first", Some(Path::new("/tmp/a.pcnd")))
            .unwrap();
        assert!(Recovery::scan(&dir).is_empty(), "the owner is alive");
        let session = r.session();
        // Simulate a crash: the lock is released but the checkpoint stays.
        drop(r);
        let found = Recovery::scan(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].session, found[0].original.as_deref()),
            (session, Some("/tmp/a.pcnd"))
        );
        assert_eq!(fs::read(&found[0].checkpoint).unwrap(), b"first");
        Recovery::discard(&dir, session);
        assert!(Recovery::scan(&dir).is_empty());
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "nothing left behind"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn checkpoints_replace_atomically_and_clear_removes_everything() {
        let dir = scratch("replace");
        let r = Recovery::start(&dir).unwrap();
        for n in 0..5u8 {
            r.checkpoint(&[n; 1000], None).unwrap();
        }
        assert_eq!(
            fs::read(dir.join(format!("{}.pcnd", r.session()))).unwrap(),
            vec![4u8; 1000]
        );
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            3,
            "lock, checkpoint and metadata only; no staging files"
        );
        r.clear();
        drop(r);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn orphaned_locks_without_checkpoints_are_tidied_and_garbage_is_ignored() {
        let dir = scratch("tidy");
        let r = Recovery::start(&dir).unwrap();
        let s = r.session();
        std::mem::forget(r); // keeps the lock fd open: still "live" in this process
        assert!(Recovery::scan(&dir).is_empty());
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        fs::write(dir.join("not-a-uuid.lock"), b"").unwrap();
        assert!(Recovery::scan(&dir).is_empty());
        assert!(
            dir.join("notes.txt").exists() && dir.join("not-a-uuid.lock").exists(),
            "unrelated files are untouched"
        );
        assert!(dir.join(format!("{s}.lock")).exists());
        // A missing directory is simply nothing to recover.
        assert!(Recovery::scan(&dir.join("absent")).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failure_to_write_is_reported_not_swallowed() {
        let dir = scratch("fail");
        let r = Recovery::start(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            r.checkpoint(b"x", None).unwrap_err().code,
            ErrorCode::StorageFailed
        );
    }
}
