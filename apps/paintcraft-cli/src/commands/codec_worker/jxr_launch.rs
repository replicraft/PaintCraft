//! Parent owns seekable JXR backing until supervised termination and reap.
use super::*;
pub(super) fn run(
    executable: &Path,
    request: &wire::Request,
    remaining: Duration,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    paintcraft_formats::jxr::inspect_checked(
        request.source.as_slice(),
        u64::from(request.header.pixels),
        check,
    )?;
    let source = paintcraft_platform::source::PrivateSource::create(
        request.source.as_slice(),
        "jxr",
        wire::INPUT_BYTES_MAX as u64,
        check,
    )?;
    source.identity().verify_bytes(
        source.path(),
        request.header.length,
        &request.header.sha256,
        check,
    )?;
    let args = [
        std::ffi::OsString::from("--jxr-source"),
        source.path().as_os_str().to_owned(),
    ];
    let result = process::run_parent_lease(
        executable,
        &args,
        request.segments(),
        process::Limits {
            input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: remaining,
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    );
    // The supervisor has stopped/reaped the child and joined pipes on success,
    // failure or cancellation before this explicit cleanup releases its backing.
    let cleanup = source.close();
    let output = result?;
    cleanup?;
    Ok(output.bytes)
}
