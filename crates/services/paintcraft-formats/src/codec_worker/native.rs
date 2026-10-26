//! Verified native composition; callers perform exact-profile admission. The caller
//! admits source/output/helper bytes before invoking this service, retains its
//! existing resource lease through consume/publication, and supplies the actual
//! kill/reap supervisor. This module never launches arbitrary PATH-selected code.
mod import;
mod launch;
use super::*;
pub use import::{AdoptedImport, PreparedImport};
pub use launch::Launch;
use paintcraft_core::types::policy::hex;
use paintcraft_platform::{codec_helper, source::SourceIdentity};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// An already acquired source identity and immutable logical job owner. Construct
/// only after host admission; owner must describe the originating document/tab.
#[derive(Debug)]
pub struct Job {
    pub input: PathBuf,
    pub identity: SourceIdentity,
    pub length: u64,
    pub codec: Codec,
    pub owner: Owner,
    pub pixels: u64,
    pub deadline: Duration,
    pub application_version: String,
    pub expected_sha256: Option<String>,
}
impl Job {
    /// Launch borrows the verified installation path, captured request and
    /// service-owned arguments/backing; ordinary codecs receive no arguments.
    /// It must bound pipe IO, retain the sole stdin parent lease, and terminate,
    /// reap and join IO workers on every return/unwind before its caller's lease
    /// can drop. An arbitrary spawn/read-to-end callback violates this contract.
    pub fn consume<T>(
        self,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        launch: impl FnOnce(Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
        consume: impl FnOnce(&Request, Frame<'_>, [u8; 32], &dyn Fn() -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        if self.deadline.is_zero() || self.deadline > Duration::from_secs(120) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid native codec deadline",
            ));
        }
        let start = Instant::now();
        let check = || {
            host_check()?;
            if current_owner()? != self.owner {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Codec job owner changed",
                ));
            }
            if start.elapsed() >= self.deadline {
                return Err(resource("Codec job deadline exceeded"));
            }
            Ok(())
        };
        check()?;
        let source = self
            .identity
            .capture_bytes(self.length, INPUT_BYTES_MAX as u64, &check)?;
        let request = Request::capture_for(self.codec, Arc::new(source), self.owner, self.pixels)?;
        if let Some(expected) = &self.expected_sha256 {
            if expected.len() != 64
                || !expected
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Expected input SHA-256 must be lowercase hexadecimal",
                ));
            }
            if *expected != hex(&request.header.sha256) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Captured codec source digest changed",
                ));
            }
        }
        self.identity
            .verify_bytes(&self.input, self.length, &request.header.sha256, &check)?;
        let helper = installed_helper(self.codec, &self.application_version, &check)?;
        let executable = helper.checked_executable(&check)?;
        let backing = launch::Backing::new(&request, &check)?;
        let remaining = self
            .deadline
            .checked_sub(start.elapsed())
            .ok_or_else(|| resource("Codec job deadline exceeded"))?;
        // Backing stays owned across the injected kill/reap supervisor.
        let output = backing.invoke(executable, &request, remaining, &check, launch)?;
        if output.len() > OUTPUT_BYTES_MAX {
            return Err(resource("Native codec output exceeds allowance"));
        }
        helper.checked_executable(&check)?;
        self.identity
            .verify_bytes(&self.input, self.length, &request.header.sha256, &check)?;
        let frame = decode(&output, &request, current_owner()?)?;
        check()?;
        let prepared = consume(&request, frame, helper.sha256(), &check)?;
        // Consumption may build a document or hash planes. Recheck after that
        // work, before any prepared value leaves the admitted host job.
        helper.checked_executable(&check)?;
        self.identity
            .verify_bytes(&self.input, self.length, &request.header.sha256, &check)?;
        check()?;
        Ok(prepared)
    }
}
fn installed_helper(
    codec: Codec,
    version: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<codec_helper::VerifiedHelper> {
    let legacy_record = native_build_record(version)?;
    let avif_record = native_avif_build_record(version)?;
    let base = codec_helper::Contract {
        application_version: version,
        protocol_version: PROTOCOL_VERSION,
        profile: NATIVE_PROFILE,
        requested_heap_ceiling_bytes: HEAP_BYTES,
        qualified_import_variants: QUALIFIED_NATIVE_IMPORT_VARIANTS,
        build_marker: BUILD_RECORD_MARKER,
        build_record: &legacy_record,
    };
    let avif = codec_helper::Contract {
        application_version: version,
        protocol_version: PROTOCOL_VERSION,
        profile: AVIF_NATIVE_PROFILE,
        requested_heap_ceiling_bytes: HEAP_BYTES,
        qualified_import_variants: QUALIFIED_AVIF_IMPORT_VARIANTS,
        build_marker: AVIF_BUILD_RECORD_MARKER,
        build_record: &avif_record,
    };
    if codec == Codec::AvifSrgb8 {
        codec_helper::discover(avif, check)
    } else {
        codec_helper::discover_compatible(base, avif, check)
    }
}
#[cfg(test)]
mod tests;
