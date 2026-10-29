//! Immutable file observation only; no document, ticket or adoption authority.
use super::*;
use paintcraft_formats::source_inspection::{self, Limits};
use paintcraft_platform::source::SourceIdentity;
#[path = "inspect_source/json.rs"]
mod json;
#[derive(Clone, Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[arg(long, default_value_t = source_inspection::INPUT_BYTES_MAX)]
    max_input_bytes: u64,
    #[arg(long, default_value_t = image::DECODE_PIXELS_MAX)]
    max_pixels: u64,
    /// Lower adapter-specific metadata/ICC allowance, not an aggregate memory cap.
    #[arg(long)]
    max_metadata_bytes: Option<u64>,
}
fn acquired(
    path: &Path,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Vec<u8>, [u8; 32], SourceIdentity)> {
    check()?;
    let identity = SourceIdentity::existing_destination(path)?.ok_or_else(|| {
        Error::new(
            ErrorCode::StorageFailed,
            "Source inspection requires a regular file",
        )
    })?;
    let length = identity
        .file()
        .metadata()
        .map_err(|e| storage("inspection source", &e))?
        .len();
    let bytes = identity.capture_bytes(length, limits.input_limit(), check)?;
    let mut hash = Sha256::new();
    for block in bytes.chunks(65536) {
        check()?;
        hash.update(block);
    }
    check()?;
    Ok((bytes, hash.finalize().into(), identity))
}
pub(super) fn inspect_path(
    path: &Path,
    expected: Option<&str>,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Value, String)> {
    let (bytes, sha, identity) = acquired(path, limits, check)?;
    guard(expected, &hex(&sha))?;
    let source = source_inspection::inspect(&bytes, limits, check)?;
    if source.source_sha256() != sha || source.source_bytes() != bytes.len() as u64 {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "Inspection source digest mismatch",
        ));
    }
    let value = json::report(&source, limits)?;
    check()?;
    #[cfg(test)]
    tests::before_revalidation(path);
    identity.verify_bytes(path, bytes.len() as u64, &sha, check)?;
    check()?;
    let (w, h) = source.dimensions();
    Ok((
        value,
        format!(
            "{} source {w}x{h}, sha256 {}; pixels not decoded; no adoption authority",
            source.format(),
            hex(&sha)
        ),
    ))
}
pub fn run(options: Options, json: bool) -> Reply {
    bare(
        json,
        inspect_path(
            &options.input,
            options.expect_input_sha256.as_deref(),
            Limits {
                input_bytes: options.max_input_bytes,
                pixels: options.max_pixels,
                metadata_bytes: options.max_metadata_bytes,
            },
            &|| Ok(()),
        ),
    )
}
#[cfg(test)]
#[path = "inspect_source/tests.rs"]
pub(crate) mod tests;

pub(super) fn schema() -> Result<Value> {
    json::schema()
}
