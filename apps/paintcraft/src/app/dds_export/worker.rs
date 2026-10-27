//! One facade preparation and an actual encoded-pixel display preview.
use super::*;
use paintcraft_core::types::resources::ResourceLease;
use paintcraft_formats::dds::{self, TextureOptions, TexturePlan};
use paintcraft_runtime::Cancellation;
pub(crate) const PREVIEW_SIDE: u32 = 192;
#[derive(Debug)]
pub(crate) struct Prepared {
    pub artifact: export::dds::PreparedDds,
    pub preview: Option<egui::ColorImage>,
    // The executor's reservation follows the exact artifact through publication.
    _lease: ResourceLease,
}
pub(super) fn options(settings: Settings) -> TextureOptions {
    use paintcraft_ui::dds_export as ui;
    TextureOptions {
        encoding: match settings.encoding {
            ui::Encoding::Rgba8 => dds::Encoding::Rgba8,
            ui::Encoding::Bc1 => dds::Encoding::Bc1,
            ui::Encoding::Bc2 => dds::Encoding::Bc2,
            ui::Encoding::Bc3 => dds::Encoding::Bc3,
        },
        transfer: match settings.transfer {
            ui::Transfer::Srgb => dds::Transfer::Srgb,
            ui::Transfer::Linear8 => dds::Transfer::Linear,
        },
        alpha: if settings.encoding == ui::Encoding::Bc1 {
            dds::SourceAlpha::Opaque
        } else {
            dds::SourceAlpha::Straight
        },
        mips: match settings.mips {
            ui::Mips::None => dds::MipCount::Levels(1),
            ui::Mips::Full => dds::MipCount::Full,
            ui::Mips::Custom(n) => dds::MipCount::Levels(n),
        },
        filter: match settings.filter {
            ui::Filter::Nearest => dds::MipFilter::Nearest,
            ui::Filter::BoxStored => dds::MipFilter::BoxStored,
        },
        quality: match settings.quality {
            ui::Quality::Normal => dds::TextureQuality::Normal,
            ui::Quality::High => dds::TextureQuality::High,
        },
    }
}
fn priced(plan: &TexturePlan, resident: u64) -> Result<u64> {
    // Composite RGBA is freed when the facade returns. Actual top-mip decode
    // then reuses that logical RGBA allowance; it never overlaps that composite.
    // Three small preview copies cover CPU image/upload staging, not GPU/RSS.
    export::dds::preparation_reservation_bytes(plan)?
        .checked_add(resident)
        // An existing destination can fill the capture allowance during picking.
        .and_then(|n| n.checked_add(dds::BYTES_MAX as u64))
        .and_then(|n| n.checked_add(u64::from(PREVIEW_SIDE).pow(2) * 4 * 3))
        .and_then(|n| n.checked_add(128 << 10))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "DDS preview preparation overflow"))
}
pub(super) fn quote(doc: &paintcraft_document::Document, settings: Settings) -> Result<u64> {
    let plan = TexturePlan::new(doc.canvas.0, doc.canvas.1, options(settings))?;
    let resident = doc.layers.values().try_fold(0u64, |sum, layer| {
        (layer.raster.resident_tiles() as u64)
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .and_then(|bytes| sum.checked_add(bytes))
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "DDS retained source overflow"))
    })?;
    priced(&plan, resident)
}
pub(super) fn prepare(
    doc: Arc<paintcraft_document::Document>,
    settings: Settings,
    quoted: u64,
    cancel: Cancellation,
) -> Result<Prepared> {
    let lease = cancel.resource_lease().ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "DDS preparation admission missing",
        )
    })?;
    let check = || cancel.check();
    let artifact = export::dds::prepare_document(
        &doc,
        options(settings),
        &export::KNOWN_LOSSES.map(str::to_owned),
        |plan, resident| {
            if priced(plan, resident)? != quoted {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "DDS preparation quote changed",
                ));
            }
            check()
        },
        &check,
    )?;
    let decoded = dds::decode_top_mip_checked(artifact.encoded().bytes(), dds::PIXELS_MAX, &check)?;
    let (width, height) = (decoded.info.image.width, decoded.info.image.height);
    let scale = PREVIEW_SIDE as f64 / f64::from(width.max(height));
    let w = (f64::from(width) * scale.min(1.0)).floor().max(1.0) as u32;
    let h = (f64::from(height) * scale.min(1.0)).floor().max(1.0) as u32;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact((w * h) as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "DDS preview allocation"))?;
    for y in 0..h {
        check()?;
        for x in 0..w {
            let at = ((u64::from(y) * u64::from(height) / u64::from(h)) * u64::from(width)
                + u64::from(x) * u64::from(width) / u64::from(w)) as usize
                * 4;
            let mut pixel: [u8; 4] = decoded.rgba[at..at + 4].try_into().unwrap();
            if artifact.transfer().output() == dds::Transfer::Linear {
                for value in &mut pixel[..3] {
                    *value = egui::ecolor::gamma_u8_from_linear_f32(f32::from(*value) / 255.0);
                }
            }
            pixels.push(egui::Color32::from_rgba_unmultiplied(
                pixel[0], pixel[1], pixel[2], pixel[3],
            ));
        }
    }
    check()?;
    Ok(Prepared {
        artifact,
        preview: Some(egui::ColorImage::new([w as usize, h as usize], pixels)),
        _lease: lease,
    })
}
