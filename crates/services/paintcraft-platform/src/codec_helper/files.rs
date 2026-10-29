use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};
#[derive(Debug)]
pub(super) struct Pinned {
    pub identity: SourceIdentity,
    pub digest: [u8; 32],
    stamp: Stamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    bytes: u64,
    modified: Option<SystemTime>,
    mode: u32,
}
impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode = u32::from(metadata.permissions().readonly());
        Self {
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
            mode,
        }
    }
}
pub(super) fn open(path: &Path, limit: u64) -> Result<File> {
    plain_path(path)?;
    let before = fs::symlink_metadata(path).map_err(storage)?;
    if !before.is_file() {
        return Err(unavailable(
            "Installed helper payload must be a regular file",
        ));
    }
    if before.len() == 0 || before.len() > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Installed helper file size allowance",
        ));
    }
    let file = File::open(path).map_err(storage)?;
    if !file.metadata().map_err(storage)?.is_file() {
        return Err(changed("Helper file changed while opening"));
    }
    plain_path(path)?;
    Ok(file)
}
fn snapshot(
    path: &Path,
    limit: u64,
    contract: Option<Contract<'_>>,
    collect: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Pinned, Vec<u8>)> {
    check()?;
    let file = open(path, limit)?;
    let stamp = Stamp::of(&file.metadata().map_err(storage)?);
    if stamp.bytes == 0 || stamp.bytes > limit {
        return Err(changed("Helper file size changed while acquiring"));
    }
    let identity = SourceIdentity::from_file(file)?;
    let mut input = identity.file();
    input.seek(SeekFrom::Start(0)).map_err(storage)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut block = [0; 65536];
    let mut bytes = Vec::new();
    if collect {
        bytes
            .try_reserve_exact(stamp.bytes as usize)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Helper manifest allocation"))?;
    }
    let mut record = contract.map(Record::new);
    loop {
        check()?;
        let count = input.read(&mut block).map_err(storage)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| changed("Helper byte count overflow"))?;
        if total > stamp.bytes || total > limit {
            return Err(changed("Helper file grew during verification"));
        }
        hash.update(&block[..count]);
        if collect {
            bytes.extend_from_slice(&block[..count]);
        }
        if let Some(record) = &mut record {
            record.push(&block[..count])?;
        }
    }
    if let Some(record) = record {
        record.finish()?;
    }
    let after = Stamp::of(&identity.file().metadata().map_err(storage)?);
    if total != stamp.bytes || after != stamp {
        return Err(changed("Helper file changed while hashing"));
    }
    plain_path(path)?;
    if !identity.matches_path(path)? {
        return Err(changed("Helper pathname no longer names captured file"));
    }
    Ok((
        Pinned {
            identity,
            digest: hash.finalize().into(),
            stamp,
        },
        bytes,
    ))
}
pub(super) fn inspect(
    path: &Path,
    limit: u64,
    contract: Option<Contract<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Pinned> {
    let result = snapshot(path, limit, contract, false, check)?.0;
    #[cfg(unix)]
    if result.stamp.mode & 0o111 == 0 {
        return Err(unavailable("Installed codec helper is not executable"));
    }
    Ok(result)
}
pub(super) fn read_manifest(
    path: &Path,
    limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Pinned, Vec<u8>)> {
    snapshot(path, limit, None, true, check)
}
impl Pinned {
    pub fn revalidate(&self, path: &Path, check: &dyn Fn() -> Result<()>) -> Result<()> {
        let next = snapshot(path, self.stamp.bytes, None, false, check)?.0;
        if self.stamp != next.stamp
            || self.digest != next.digest
            || !self.identity.matches(&next.identity)
        {
            return Err(changed(
                "Installed codec helper or manifest changed after verification",
            ));
        }
        Ok(())
    }
}
/// Scan a bounded rolling window while hashing the exact acquired bytes.
struct Record<'a> {
    contract: Contract<'a>,
    tail: Vec<u8>,
    found: usize,
}
impl<'a> Record<'a> {
    fn new(contract: Contract<'a>) -> Self {
        Self {
            contract,
            tail: Vec::with_capacity(65536 + 1024),
            found: 0,
        }
    }
    fn push(&mut self, bytes: &[u8]) -> Result<()> {
        self.tail.extend_from_slice(bytes);
        // Keep enough lookahead for a different bounded version and full tail.
        let full = self
            .tail
            .len()
            .saturating_sub(self.contract.build_record.len() + 65);
        self.scan(full)?;
        self.tail.drain(..full);
        Ok(())
    }
    fn scan(&mut self, count: usize) -> Result<()> {
        let marker = self.contract.build_marker;
        let record = self.contract.build_record;
        let version = self.contract.application_version.as_bytes();
        let rest_expected = &record[marker.len()..];
        let suffix = rest_expected
            .strip_prefix(version)
            .filter(|suffix| suffix.starts_with(b"\0") && suffix.len() > 1);
        for index in 0..count {
            let current = &self.tail[index..];
            if !current.starts_with(marker) {
                continue;
            }
            if current.starts_with(record) {
                self.found += 1;
                if self.found > 1 {
                    return Err(unavailable("Installed helper build record is duplicated"));
                }
                continue;
            }
            let Some(suffix) = suffix else {
                return Err(unavailable(
                    "Installed helper embedded version/protocol/heap record mismatch",
                ));
            };
            let rest = &current[marker.len()..];
            let shaped = |bytes: &[u8]| {
                bytes.first().is_some_and(u8::is_ascii_digit)
                    && bytes
                        .iter()
                        .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte))
            };
            let declared = match rest.iter().take(65).position(|byte| *byte == 0) {
                Some(length) => rest[length..].starts_with(suffix) || shaped(&rest[..length]),
                None => shaped(&rest[..rest.len().min(65)]),
            };
            if declared {
                return Err(unavailable(
                    "Installed helper embedded version/protocol/heap record mismatch",
                ));
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<()> {
        self.scan(self.tail.len())?;
        if self.found != 1 {
            Err(unavailable(
                "Installed helper build record missing or truncated",
            ))
        } else {
            Ok(())
        }
    }
}
