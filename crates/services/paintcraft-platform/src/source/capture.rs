//! Bounded reads from the acquired descriptor, not a reopened source pathname.
use super::*;
use sha2::{Digest, Sha256};
use std::fs::File;
fn conflict() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Acquired source changed during worker preparation",
    )
}
fn metadata(file: &File) -> Result<(u64, Option<std::time::SystemTime>)> {
    let value = file.metadata().map_err(identity_error)?;
    if !value.is_file() {
        return Err(conflict());
    }
    Ok((value.len(), value.modified().ok()))
}
fn at(file: &File, bytes: &mut [u8], offset: u64) -> std::io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_at(bytes, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        file.seek_read(bytes, offset)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, bytes, offset);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "offset reads unavailable",
        ))
    }
}
impl SourceIdentity {
    /// The caller must admit `expected_bytes` before calling. Refuse observed
    /// size/metadata changes, allocate exactly that byte allowance, and read
    /// from this already-acquired descriptor. This does not lock other writers.
    pub fn capture_bytes(
        &self,
        expected_bytes: u64,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        check()?;
        if expected_bytes > limit {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Source byte allowance",
            ));
        }
        let before = metadata(self.file())?;
        if before.0 != expected_bytes {
            return Err(conflict());
        }
        let len = usize::try_from(expected_bytes)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Source address allowance"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Source allocation allowance"))?;
        bytes.resize(len, 0);
        let mut offset = 0;
        while offset < len {
            check()?;
            let end = (offset + 65536).min(len);
            let count =
                at(self.file(), &mut bytes[offset..end], offset as u64).map_err(identity_error)?;
            if count == 0 {
                return Err(conflict());
            }
            offset += count;
        }
        let mut extra = [0];
        if at(self.file(), &mut extra, expected_bytes).map_err(identity_error)? != 0
            || metadata(self.file())? != before
        {
            return Err(conflict());
        }
        check()?;
        Ok(bytes)
    }
    /// Check the exact acquired descriptor and that its current pathname still
    /// belongs to it. SHA is compared to captured bytes, including same-length
    /// in-place changes. Namespace/write races can recur after this returns.
    pub fn verify_bytes(
        &self,
        path: &Path,
        expected_bytes: u64,
        expected_sha256: &[u8; 32],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        if !self.matches_path(path)? {
            return Err(conflict());
        }
        let before = metadata(self.file())?;
        if before.0 != expected_bytes {
            return Err(conflict());
        }
        let mut digest = Sha256::new();
        let mut offset = 0;
        let mut block = [0; 65536];
        while offset < expected_bytes {
            check()?;
            let remaining = (expected_bytes - offset).min(block.len() as u64) as usize;
            let count = at(self.file(), &mut block[..remaining], offset).map_err(identity_error)?;
            if count == 0 {
                return Err(conflict());
            }
            digest.update(&block[..count]);
            offset += count as u64;
        }
        let mut extra = [0];
        if at(self.file(), &mut extra, expected_bytes).map_err(identity_error)? != 0
            || <[u8; 32]>::from(digest.finalize()) != *expected_sha256
            || metadata(self.file())? != before
            || !self.matches_path(path)?
        {
            return Err(conflict());
        }
        check()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
