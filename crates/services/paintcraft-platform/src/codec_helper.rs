//! Installed helper consistency checks. The application supplies the codec
//! contract; this OS adapter does not depend on formats or enable any reader.
//! Open identities pin acquired files and hashes describe the observed bytes.
//! They do not lock writers or authenticate the manifest. Installation permissions
//! must have the same trust as the running application; publisher/code signatures
//! are not verified here. A checked path is not atomic execution-by-handle, and
//! observed-change checks do not eliminate concurrent traversal/write/spawn races.
mod files;
mod target;
use crate::source::SourceIdentity;
use files::{Pinned, inspect, read_manifest};
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use target::Target;

pub const MANIFEST_NAME: &str = "paintcraft-codec-worker.json";
const MANIFEST_BYTES_MAX: u64 = 8192;
const EXECUTABLE_BYTES_MAX: u64 = 256 << 20;
/// Values originate at the format/application composition boundary, never from
/// the selected file or a helper's self-reported manifest.
#[derive(Clone, Copy, Debug)]
pub struct Contract<'a> {
    pub application_version: &'a str,
    pub protocol_version: u16,
    pub profile: &'a str,
    pub requested_heap_ceiling_bytes: u64,
    pub qualified_import_variants: &'a [&'a str],
    pub build_marker: &'a [u8],
    pub build_record: &'a [u8],
}
impl Contract<'_> {
    fn validate(self) -> Result<()> {
        if self.application_version.is_empty()
            || self.application_version.len() > 64
            || !self.application_version.is_ascii()
            || self.profile.is_empty()
            || self.profile.len() > 128
            || self.qualified_import_variants.len() > 8
            || self
                .qualified_import_variants
                .iter()
                .enumerate()
                .any(|(index, variant)| {
                    variant.is_empty()
                        || variant.len() > 128
                        || !variant.is_ascii()
                        || self.qualified_import_variants[..index].contains(variant)
                })
            || self.protocol_version == 0
            || self.requested_heap_ceiling_bytes == 0
            || self.requested_heap_ceiling_bytes > i64::MAX as u64
            || self.build_marker.is_empty()
            || self.build_marker.len() > 64
            || self.build_record.len() > 512
            || !self.build_record.starts_with(self.build_marker)
            || self.build_record.len() <= self.build_marker.len()
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid installed codec contract",
            ));
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct VerifiedHelper {
    executable: PathBuf,
    manifest: PathBuf,
    helper_pin: Pinned,
    manifest_pin: Pinned,
}
impl VerifiedHelper {
    /// Reopen and hash the currently named files, checking their acquired file
    /// identities as well as bytes. A later path swap before spawn is still a
    /// race; installations must have the same trust as the running application.
    pub fn checked_executable(&self, check: &dyn Fn() -> Result<()>) -> Result<&Path> {
        self.helper_pin.revalidate(&self.executable, check)?;
        self.manifest_pin.revalidate(&self.manifest, check)?;
        check()?;
        Ok(&self.executable)
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.helper_pin.digest
    }
}
fn unavailable(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn changed(message: &str) -> Error {
    Error::new(ErrorCode::ConflictStaleBase, message)
}
fn storage(error: std::io::Error) -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        format!("Cannot inspect installed helper: {}", error.kind()),
    )
}
/// Reject ordinary symlinks and non-directory ancestors before opening. These
/// checks do not make namespace traversal atomic against concurrent changes.
fn plain_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Helper discovery requires an absolute application path",
        ));
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                unavailable("Packaged codec helper is missing")
            } else {
                storage(e)
            }
        })?;
        if metadata.file_type().is_symlink() || ancestor != path && !metadata.is_dir() {
            return Err(unavailable(
                "Symlinked or non-directory helper installation path",
            ));
        }
    }
    Ok(())
}
/// Discover only within this running application's installation; never search
/// PATH, working directory, user-selected locations or global fallback folders.
pub fn discover(contract: Contract<'_>, check: &dyn Fn() -> Result<()>) -> Result<VerifiedHelper> {
    let application = std::env::current_exe().map_err(storage)?;
    for_application(&application, contract, check)
}
/// `application` must be the host-owned executable path, not an imported file or
/// user-selected helper. Exposed for composition and deterministic installation
/// tests; matching architecture is the compiled application's target.
pub fn for_application(
    application: &Path,
    contract: Contract<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedHelper> {
    verify(application, contract, Target::current()?, check)
}
fn verify(
    application: &Path,
    contract: Contract<'_>,
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<VerifiedHelper> {
    check()?;
    contract.validate()?;
    plain_path(application)?;
    let app = files::open(application, EXECUTABLE_BYTES_MAX)?;
    target.verify(&app)?;
    let directory = application
        .parent()
        .ok_or_else(|| unavailable("Application has no installation directory"))?;
    let executable = directory.join(target.executable());
    let adjacent = directory.join(MANIFEST_NAME);
    let manifest = match fs::symlink_metadata(&adjacent) {
        Ok(_) => adjacent,
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && target.linux()
                && directory.file_name().is_some_and(|n| n == "bin") =>
        {
            directory
                .parent()
                .ok_or_else(|| unavailable("Linux helper installation prefix missing"))?
                .join("share/paintcraft")
                .join(MANIFEST_NAME)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => adjacent,
        Err(e) => return Err(storage(e)),
    };
    let (manifest_pin, bytes) = read_manifest(&manifest, MANIFEST_BYTES_MAX, check)?;
    let value = paintcraft_core::types::json::parse(&bytes)
        .map_err(|_| unavailable("Installed helper manifest is not strict JSON"))?;
    let expected_fields = 10;
    if value.as_object().is_none_or(|v| v.len() != expected_fields)
        || value.get("schema_version").and_then(Value::as_i64) != Some(2)
        || value
            .get("embedded_build_record_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || value.get("application_version").and_then(Value::as_str)
            != Some(contract.application_version)
        || value.get("protocol_version").and_then(Value::as_i64)
            != Some(i64::from(contract.protocol_version))
        || value.get("target").and_then(Value::as_str) != Some(target.triple())
        || value.get("executable").and_then(Value::as_str) != Some(target.executable())
        || value.get("profile").and_then(Value::as_str) != Some(contract.profile)
        || value
            .get("requested_heap_ceiling_bytes")
            .and_then(Value::as_i64)
            != Some(contract.requested_heap_ceiling_bytes as i64)
        || value
            .get("qualified_import_variants")
            .and_then(Value::as_array)
            .is_none_or(|variants| {
                variants.len() != contract.qualified_import_variants.len()
                    || variants
                        .iter()
                        .zip(contract.qualified_import_variants)
                        .any(|(actual, expected)| actual.as_str() != Some(*expected))
            })
    {
        return Err(unavailable(
            "Installed helper manifest does not match host contract",
        ));
    }
    let expected = value
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| unavailable("Installed helper digest missing"))?;
    let helper_pin = inspect(&executable, EXECUTABLE_BYTES_MAX, Some(contract), check)?;
    target.verify(helper_pin.identity.file())?;
    if paintcraft_core::types::policy::hex(&helper_pin.digest) != expected {
        return Err(unavailable(
            "Installed helper SHA-256 does not match manifest",
        ));
    }
    let result = VerifiedHelper {
        executable,
        manifest,
        helper_pin,
        manifest_pin,
    };
    result.checked_executable(check)?;
    Ok(result)
}
#[cfg(test)]
mod tests;
