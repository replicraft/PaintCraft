//! Defer scan reclamation until bounded validation and its final check succeed.
use super::*;
use paintcraft_platform::source::SourceIdentity;
// Bound retained namespace locks/control records. Extra candidates remain eligible
// on the next scan, so this batch limit cannot permanently prevent reclamation.
const CANDIDATES_MAX: usize = 64;
struct Candidate {
    directory: PathBuf,
    session: Uuid,
    lock: SourceIdentity,
    cleared: bool,
}
#[derive(Default)]
pub(super) struct Batch {
    candidates: Vec<Candidate>,
}
impl Batch {
    pub(super) fn defer(
        &mut self,
        directory: &Path,
        session: Uuid,
        lock: File,
        cleared: bool,
        issues: &mut Vec<String>,
    ) -> Result<()> {
        if self.candidates.len() == CANDIDATES_MAX {
            if !issues
                .iter()
                .any(|s| s == "Additional recovery cleanup was deferred until a later scan")
            {
                issues.push("Additional recovery cleanup was deferred until a later scan".into());
            }
            return Ok(());
        }
        self.candidates.try_reserve(1).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "recovery cleanup control allocation",
            )
        })?;
        self.candidates.push(Candidate {
            directory: directory.to_owned(),
            session,
            lock: SourceIdentity::from_file(lock)?,
            cleared,
        });
        Ok(())
    }
    /// Caller performs its final control check BEFORE entering this phase. No
    /// later callback may retroactively report cancellation after deletion.
    pub(super) fn commit(self, issues: &mut Vec<String>) {
        for candidate in self.candidates {
            if let Err(error) = candidate.commit() {
                issues.push(format!("Recovery cleanup retained: {error}"));
            }
        }
    }
}
impl Candidate {
    fn commit(self) -> Result<()> {
        let path = self.directory.join(format!("{}.lock", self.session));
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_file())
            || !self.lock.matches_path(&path)?
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "recovery cleanup lock namespace changed",
            ));
        }
        if self.cleared {
            let marker = tombstone(&self.directory, self.session);
            if read_bounded(&marker, RECORD_MAX)? != cleared_bytes(self.session) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "recovery cleanup marker changed",
                ));
            }
            clear_owned(&self.directory, self.session)?;
            remove_if_present(&marker)?;
            remove_if_present(&path)?;
            sync_directory(&self.directory)
        } else {
            let names = owned_paths(&self.directory, self.session)?;
            let sidecar = self.directory.join(format!("{}.json", self.session));
            if !names.iter().all(|p| p == &path || p == &sidecar) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "recovery cleanup is no longer empty",
                ));
            }
            remove_if_present(&sidecar)?;
            remove_if_present(&path)?;
            sync_directory(&self.directory)
        }
    }
}
