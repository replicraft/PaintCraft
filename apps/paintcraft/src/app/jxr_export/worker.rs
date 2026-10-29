//! One admitted composite/pack/encode operation; no UI or destination I/O.
use super::*;
use paintcraft_formats::codec_worker::{jxr_encode as encode, native::JxrEncodeJob};
use paintcraft_runtime::{Cancellation, process};

pub(super) fn quote(layout: encode::Layout) -> Result<u64> {
    let encode_quote = layout
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
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "JPEG XR export quote overflow"))?;
    Ok(encode_quote.max(wire::native::jxr_preview_reservation_bytes(layout)?))
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
    let preview = encoded.consume_decoded_preview(
        Duration::from_secs(10),
        &check,
        &|| Ok(owner),
        |launch, check| {
            process::run_parent_lease(
                launch.executable,
                launch.arguments,
                launch.request.segments(),
                process::Limits {
                    input_bytes: wire::REQUEST_HEADER + launch.request.source.len(),
                    output_bytes: wire::output_limit(wire::Codec::JxrRgb8),
                    deadline: launch.remaining,
                    resource_exit_code: Some(wire::RESOURCE_EXIT),
                },
                check,
            )
            .map(|v| v.bytes)
        },
        |width, height, rgba, check| thumbnail(width, height, rgba, check),
    )?;
    Ok(controller::background::PreparedJxrExport {
        encoded,
        losses,
        preview: Some(preview),
    })
}

fn thumbnail(
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<egui::ColorImage> {
    let scale = (f64::from(wire::native::JXR_PREVIEW_SIDE) / f64::from(width.max(height))).min(1.0);
    let w = (f64::from(width) * scale).floor().max(1.0) as u32;
    let h = (f64::from(height) * scale).floor().max(1.0) as u32;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact((w * h) as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR preview allocation"))?;
    for y in 0..h {
        check()?;
        for x in 0..w {
            let at = ((u64::from(y) * u64::from(height) / u64::from(h)) * u64::from(width)
                + u64::from(x) * u64::from(width) / u64::from(w)) as usize
                * 4;
            pixels.push(egui::Color32::from_rgb(
                rgba[at],
                rgba[at + 1],
                rgba[at + 2],
            ));
        }
    }
    check()?;
    Ok(egui::ColorImage::new([w as usize, h as usize], pixels))
}

#[cfg(test)]
#[path = "worker_preview_tests.rs"]
mod preview_tests;
