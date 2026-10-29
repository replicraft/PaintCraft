//! One admitted composite/pack/encode operation; no UI or destination I/O.
use super::*;
use paintcraft_formats::codec_worker::{jxr_encode as encode, native::JxrEncodeJob};
use paintcraft_runtime::{Cancellation, process};

pub(super) fn quote(layout: encode::Layout) -> Result<u64> {
    layout
        .reservation_bytes()?
        .checked_add(layout.encoded_cap())
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XR destination quote overflow",
            )
        })?
        .checked_add(u64::from(layout.width()) * u64::from(layout.height()) * 4)
        // Source-guard metadata and the bounded final staging copy buffer.
        .and_then(|value| value.checked_add(128 << 10))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "JPEG XR export quote overflow"))
}
pub(super) fn prepare(
    source: Arc<paintcraft_document::Document>,
    owner: wire::Owner,
    cancel: Cancellation,
    policy: export::metadata_policy::Policy,
) -> Result<controller::background::PreparedJxrExport> {
    let lease = cancel
        .resource_lease()
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "JPEG XR export admission missing"))?;
    let check = || cancel.check();
    let allowed = export::KNOWN_LOSSES.map(str::to_owned);
    let opaque = export::jxr::prepare_opaque_rgb_with_metadata_policy(
        &source,
        encode::ENCODED_BYTES_MAX,
        &allowed,
        |layout, rgba| {
            let expected = u64::from(layout.width()) * u64::from(layout.height()) * 4;
            if rgba != expected {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "JPEG XR composite quote disagrees",
                ));
            }
            quote(layout)?;
            check()
        },
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        &check,
        policy,
    )?;
    let (layout, rgb, losses) = opaque.into_parts();
    let encoded = JxrEncodeJob {
        layout,
        owner,
        deadline: Duration::from_secs(10),
    }
    .prepare(
        Arc::new(rgb),
        lease,
        &check,
        &|| Ok(owner),
        |actual, profile| {
            if actual != layout || profile != encode::PROFILE {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "JPEG XR encoder profile disagrees",
                ));
            }
            check()
        },
        |launch, check| {
            process::run_parent_lease(
                launch.executable,
                launch.arguments,
                launch.request.segments(),
                process::Limits {
                    input_bytes: usize::try_from(
                        encode::REQUEST_HEADER as u64 + layout.rgb_bytes(),
                    )
                    .map_err(|_| {
                        Error::new(ErrorCode::ResourceLimit, "JPEG XR input address extent")
                    })?,
                    output_bytes: usize::try_from(layout.response_limit()?).map_err(|_| {
                        Error::new(ErrorCode::ResourceLimit, "JPEG XR output address extent")
                    })?,
                    deadline: launch.remaining,
                    resource_exit_code: Some(wire::RESOURCE_EXIT),
                },
                check,
            )
            .map(|output| output.bytes)
        },
    )?;
    Ok(controller::background::PreparedJxrExport { encoded, losses })
}
