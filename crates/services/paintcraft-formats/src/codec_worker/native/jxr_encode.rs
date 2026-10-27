//! Supervised encoding with the exact final contract; no export gate or executor.
use super::super::jxr_encode as wire;
use super::*;
use paintcraft_core::types::resources::ResourceLease;
use std::{ffi::OsString, ops::Range};
#[derive(Debug)]
pub struct JxrEncodeJob {
    pub layout: wire::Layout,
    pub owner: Owner,
    pub deadline: Duration,
}
/// The callback must use the existing parent-lease supervisor and kill/reap/join
/// before returning or unwinding. The request owns immutable Arc-backed segments.
#[derive(Debug)]
pub struct JxrEncodeLaunch<'a> {
    pub executable: &'a Path,
    pub request: &'a wire::Request,
    pub arguments: &'a [OsString],
    pub remaining: Duration,
}
/// One response allocation owns the encoded body; no whole-file copy is made.
/// Its resource lease remains alive through caller staging/publication or discard.
pub struct PreparedJxrEncode {
    response: Vec<u8>,
    encoded: Range<usize>,
    header: wire::Header,
    sha256: [u8; 32],
    heap_peak: u64,
    inspection: crate::jxr::Inspection,
    helper: codec_helper::VerifiedEncoderHelper,
    _lease: ResourceLease,
}
impl std::fmt::Debug for PreparedJxrEncode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedJxrEncode")
            .field("encoded_bytes", &self.encoded.len())
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}
impl PreparedJxrEncode {
    pub fn bytes(&self) -> &[u8] {
        &self.response[self.encoded.clone()]
    }
    pub fn header(&self) -> wire::Header {
        self.header
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn helper_sha256(&self) -> [u8; 32] {
        self.helper.sha256()
    }
    pub fn heap_peak(&self) -> u64 {
        self.heap_peak
    }
    pub fn inspection(&self) -> crate::jxr::Inspection {
        self.inspection
    }
    /// Repeat this in the existing Save preparation callback after staging,
    /// before publication. Never turn an acknowledged replacement into an error
    /// through a post-commit stale check. This checkpoint is not a filesystem CAS.
    pub fn check_current(
        &self,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
    ) -> Result<()> {
        let check = || check_owner(self.header.owner(), host_check, current_owner);
        check()?;
        self.helper.checked_executable(&check)?;
        check()
    }
}
fn check_owner(
    expected: Owner,
    host_check: &dyn Fn() -> Result<()>,
    current_owner: &dyn Fn() -> Result<Owner>,
) -> Result<()> {
    host_check()?;
    if current_owner()? != expected {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "JXR encode owner changed",
        ));
    }
    Ok(())
}
fn installed(check: &dyn Fn() -> Result<()>) -> Result<codec_helper::VerifiedEncoderHelper> {
    let record = wire::qualified::build_record(env!("CARGO_PKG_VERSION"))?;
    codec_helper::discover_encoder(
        codec_helper::EncoderContract {
            decode: codec_helper::Contract {
                application_version: env!("CARGO_PKG_VERSION"),
                protocol_version: PROTOCOL_VERSION,
                profile: wire::qualified::NATIVE_PROFILE,
                requested_heap_ceiling_bytes: HEAP_BYTES,
                qualified_import_variants: wire::qualified::IMPORT_VARIANTS,
                build_marker: wire::qualified::BUILD_RECORD_MARKER,
                build_record: &record,
            },
            encode_protocol_version: wire::VERSION,
            qualified_export_variants: wire::qualified::EXPORT_VARIANTS,
        },
        check,
    )
}
impl JxrEncodeJob {
    /// Caller admits Layout::reservation_bytes before invocation and separately
    /// admits its document/composite. No RGBA interpretation or alpha drop occurs.
    pub fn prepare(
        self,
        rgb: Arc<Vec<u8>>,
        lease: ResourceLease,
        host_check: &dyn Fn() -> Result<()>,
        current_owner: &dyn Fn() -> Result<Owner>,
        admit: impl FnOnce(wire::Layout, &'static str) -> Result<()>,
        launch: impl FnOnce(JxrEncodeLaunch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    ) -> Result<PreparedJxrEncode> {
        if self.deadline.is_zero() || self.deadline > Duration::from_secs(120) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid JXR encode deadline",
            ));
        }
        self.layout.reservation_bytes()?;
        let started = Instant::now();
        let check = || {
            check_owner(self.owner, host_check, current_owner)?;
            if started.elapsed() >= self.deadline {
                return Err(resource("JXR encode deadline exceeded"));
            }
            Ok(())
        };
        check()?;
        let request = wire::Request::capture(self.layout, rgb, self.owner, &check)?;
        admit(self.layout, wire::PROFILE)?;
        check()?;
        // The capability is obtained here from one exact installed qualified helper;
        // callers cannot inject a generic VerifiedHelper or an executable path.
        let helper = installed(&check)?;
        let executable = helper.checked_executable(&check)?;
        let remaining = self
            .deadline
            .checked_sub(started.elapsed())
            .ok_or_else(|| resource("JXR encode deadline exceeded"))?;
        let arguments = [OsString::from("--jxr-encode")];
        let response = launch(
            JxrEncodeLaunch {
                executable,
                request: &request,
                arguments: &arguments,
                remaining,
            },
            &check,
        )?;
        if response.len() as u64 > self.layout.response_limit()? {
            return Err(resource("JXR encode response exceeds admitted allowance"));
        }
        helper.checked_executable(&check)?;
        let frame = wire::decode_response(&response, request.header(), current_owner()?, &check)?;
        let length = frame.bytes().len();
        let sha256 = frame.sha256();
        let heap_peak = frame.heap_peak();
        let inspection = frame.inspection();
        let start = response
            .len()
            .checked_sub(length)
            .ok_or_else(|| invalid("JXR encoded body extent"))?;
        helper.checked_executable(&check)?;
        check()?;
        Ok(PreparedJxrEncode {
            encoded: start..response.len(),
            response,
            header: request.header(),
            sha256,
            heap_peak,
            inspection,
            helper,
            _lease: lease,
        })
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod installed_tests;
