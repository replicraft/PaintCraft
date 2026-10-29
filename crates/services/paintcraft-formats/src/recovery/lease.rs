//! Claim an orphan before restoring it. The read identity and advisory lock are
//! retained through preparation; discard consumes that same ownership capability.
use super::*;

#[derive(Debug)]
pub struct RecoveryLease {
    directory: PathBuf,
    session: Uuid,
    _lock: File,
}
#[derive(Debug)]
pub struct ClaimedRecovery {
    pub document: Document,
    pub lease: RecoveryLease,
    /// Length of the exact bounded payload verified during the claim.
    pub snapshot_bytes: u64,
}
impl RecoveryLease {
    pub(super) fn acquire(directory: &Path, session: Uuid) -> Result<Self> {
        let path = directory.join(format!("{session}.lock"));
        let metadata =
            fs::symlink_metadata(&path).map_err(|e| storage("cannot inspect recovery lock", &e))?;
        if !metadata.is_file() {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "invalid recovery lock type",
            ));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| storage("cannot open recovery lock", &e))?;
        lock.try_lock().map_err(|_| {
            Error::new(
                ErrorCode::StorageFailed,
                "recovery session is already in use",
            )
        })?;
        Ok(Self {
            directory: directory.to_owned(),
            session,
            _lock: lock,
        })
    }
    /// Called only after a replacement checkpoint or user-authorized discard.
    /// Failure preserves the truthful partial cleanup state under the discard marker.
    pub fn discard(self) -> Result<()> {
        clear_owned(&self.directory, self.session)?;
        remove_if_present(&tombstone(&self.directory, self.session))?;
        remove_if_present(&self.directory.join(format!("{}.lock", self.session)))?;
        sync_directory(&self.directory)
    }
}
impl Recovery {
    pub fn claim(found: &Found) -> Result<ClaimedRecovery> {
        admission::native(|metadata| {
            Self::claim_with_metadata_admission_checked(found, metadata, &|| Ok(()))
        })
    }
    pub fn claim_with_metadata_admission_checked(
        found: &Found,
        metadata: crate::pcnd::MetadataAdmission<'_>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ClaimedRecovery> {
        let context = admission::Context::new(metadata, check);
        context.check()?;
        let directory = found
            .checkpoint
            .parent()
            .ok_or_else(|| Error::new(ErrorCode::TargetInvalid, "recovery path has no parent"))?;
        let name = found
            .checkpoint
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::new(ErrorCode::TargetInvalid, "invalid recovery filename"))?;
        let legacy = name == format!("{}.pcnd", found.session);
        let payload = name
            .strip_prefix(&format!("{}.payload-", found.session))
            .and_then(|n| n.strip_suffix(".pcnd"))
            .is_some_and(|id| Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id));
        if !legacy && !payload {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "checkpoint is outside its recovery namespace",
            ));
        }
        let lease = RecoveryLease::acquire(directory, found.session)?;
        if read_bounded(&tombstone(directory, found.session), RECORD_MAX)
            .is_ok_and(|bytes| bytes == cleared_bytes(found.session))
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "recovery was discarded after it was offered",
            ));
        }
        let bytes = context.file(&found.checkpoint, SNAPSHOT_BYTES_MAX)?;
        if hash(&bytes) != found.sha256 {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "recovery content changed after it was offered",
            ));
        }
        let document = context.read(&bytes)?;
        if document.document_id != found.document_id || document.state_id != found.state_id {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "recovery identity changed after it was offered",
            ));
        }
        context.check()?;
        Ok(ClaimedRecovery {
            document,
            lease,
            snapshot_bytes: bytes.len() as u64,
        })
    }
}
