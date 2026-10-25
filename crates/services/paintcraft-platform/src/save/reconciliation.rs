//! Reinspect one attempt without encoding or replacing its destination again.
//!
//! Local Unix file identity checks guard the opened descriptor across observation
//! and persistence barriers. These are best-effort checks against external edits,
//! not exclusion of noncooperating writers or qualification of remote filesystems.

use super::*;
use std::io::{Seek as _, SeekFrom};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconcileMode {
    /// Observe content without issuing persistence barriers.
    Inspect,
    /// If replacement was acknowledged and expected bytes remain, sync them.
    ConfirmDurability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Staged,
    Original,
    Other,
    Missing,
    Unreadable,
}

#[derive(Debug)]
pub struct Reconciliation {
    /// A revised receipt for the same operation; the original outcome is unchanged.
    pub outcome: Outcome,
    pub observation: Observation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Observe,
    FileSync,
    DirectorySync,
    Validate,
}

impl Outcome {
    /// Inspect captured destination evidence, never the caller's current path alias.
    /// This does not retry replacement or authorize writing new bytes. Currently
    /// supported for local Unix descriptors; other adapters remain unqualified.
    pub fn reconcile(&self, mode: ReconcileMode) -> Result<Reconciliation, Error> {
        reconcile(self, mode, |_| Ok(()))
    }
}

fn reconcile(
    original: &Outcome,
    mode: ReconcileMode,
    mut barrier: impl FnMut(Phase) -> std::io::Result<()>,
) -> Result<Reconciliation, Error> {
    let attempt = original.attempt.as_ref().ok_or_else(unavailable)?;
    if !original.is_reconcilable() {
        return Err(unavailable());
    }
    let mut outcome = original.clone();
    let base = *outcome
        .reconciliation_notes
        .get_or_insert(outcome.reasons.len());
    outcome.reasons.truncate(base);
    outcome.durable = false;
    outcome.observed_sha256 = None;
    // Public diagnostic hashes are not authority over the captured attempt.
    outcome.staged_sha256 = Some(attempt.staged.hash.clone());
    let observed = barrier(Phase::Observe).and_then(|()| observe(attempt));
    let observation = match observed {
        Ok(Some((mut file, fingerprint))) => {
            outcome.observed_sha256 = Some(fingerprint.hash.clone());
            let mut observation = classify(attempt, &fingerprint);
            if observation == Observation::Staged
                && attempt.replacement_acknowledged
                && mode == ReconcileMode::ConfirmDurability
            {
                let mut validating = false;
                let durable = (|| {
                    let revision = file.metadata()?;
                    barrier(Phase::FileSync)?;
                    file.sync_all()?;
                    barrier(Phase::DirectorySync)?;
                    let parent = attempt.destination.parent().ok_or_else(changed)?;
                    let directory = File::open(parent)?;
                    check_identity(&directory.metadata()?, &fs::symlink_metadata(parent)?)?;
                    directory.sync_all()?;
                    validating = true;
                    barrier(Phase::Validate)?;
                    // Syncing an old open descriptor cannot establish durability
                    // of a replacement at this namespace. Verify identity and bytes.
                    let after = read_checked(&mut file, &attempt.destination)?;
                    check_revision(&revision, &file.metadata()?)?;
                    check_identity(&directory.metadata()?, &fs::symlink_metadata(parent)?)?;
                    outcome.observed_sha256 = Some(after.hash.clone());
                    observation = classify(attempt, &after);
                    Ok::<bool, std::io::Error>(observation == Observation::Staged)
                })();
                match durable {
                    Ok(confirmed) => outcome.durable = confirmed,
                    Err(error) => {
                        outcome.reasons.push(format!(
                            "reconciliation durability not confirmed: {}",
                            error.kind()
                        ));
                        // A failed barrier alone does not negate the bytes observed
                        // earlier. A failed final validation cannot certify them now.
                        if validating {
                            observation = Observation::Unreadable;
                            outcome.observed_sha256 = None;
                        }
                    }
                }
            }
            observation
        }
        Ok(None) => Observation::Missing,
        Err(error) => {
            outcome.reasons.push(format!(
                "reconciliation inspection failed: {}",
                error.kind()
            ));
            Observation::Unreadable
        }
    };
    // No hash can turn a syscall with no positive acknowledgement into proof of
    // replacement: the expected bytes might even predate this operation.
    outcome.replacement = if observation == Observation::Staged && attempt.replacement_acknowledged
    {
        Replacement::Replaced
    } else {
        outcome.durable = false;
        Replacement::Unknown
    };
    outcome.reasons.push(format!(
        "reconciliation observed {observation:?}; no replacement was retried"
    ));
    Ok(Reconciliation {
        outcome,
        observation,
    })
}

fn classify(attempt: &Attempt, observed: &Fingerprint) -> Observation {
    if *observed == attempt.staged {
        Observation::Staged
    } else if attempt.original.as_ref() == Some(observed) {
        Observation::Original
    } else {
        Observation::Other
    }
}

fn observe(attempt: &Attempt) -> std::io::Result<Option<(File, Fingerprint)>> {
    match fs::symlink_metadata(&attempt.destination) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(changed()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let mut file = File::open(&attempt.destination)?;
    let fingerprint = read_checked(&mut file, &attempt.destination)?;
    Ok(Some((file, fingerprint)))
}

fn read_checked(file: &mut File, path: &Path) -> std::io::Result<Fingerprint> {
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(changed());
    }
    check_identity(&before, &fs::symlink_metadata(path)?)?;
    if before.len() > STAGED_FILE_BYTES_MAX {
        return Err(std::io::ErrorKind::FileTooLarge.into());
    }
    file.seek(SeekFrom::Start(0))?;
    let hash = hash_reader(&mut *file, before.len())?;
    let after = file.metadata()?;
    check_identity(&after, &fs::symlink_metadata(path)?)?;
    check_revision(&before, &after)?;
    Ok(Fingerprint {
        bytes: before.len(),
        hash,
    })
}

#[cfg(unix)]
fn check_identity(open: &fs::Metadata, named: &fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if ((open.is_file() && named.is_file()) || (open.is_dir() && named.is_dir()))
        && open.dev() == named.dev()
        && open.ino() == named.ino()
    {
        Ok(())
    } else {
        Err(changed())
    }
}

#[cfg(unix)]
fn check_revision(before: &fs::Metadata, after: &fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if before.len() == after.len()
        && before.nlink() == after.nlink()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
    {
        Ok(())
    } else {
        Err(changed())
    }
}

#[cfg(not(unix))]
fn check_identity(_: &fs::Metadata, _: &fs::Metadata) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
#[cfg(not(unix))]
fn check_revision(_: &fs::Metadata, _: &fs::Metadata) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

fn changed() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "destination identity or content changed",
    )
}
fn unavailable() -> Error {
    Error::new(
        ErrorCode::TargetInvalid,
        "only incomplete replacement attempts can be reconciled",
    )
}

#[cfg(all(test, unix))]
mod tests;
