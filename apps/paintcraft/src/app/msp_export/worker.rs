//! One composite, exact two-stage MSP allocation, and an encoded-pixel preview.
use super::*;
use paintcraft_core::types::resources::{BudgetLease, ResourceLease};
use paintcraft_runtime::{Cancellation, SharedBudget};
use std::cell::RefCell;
const PREVIEW_SIDE: u32 = 192;
#[derive(Debug)]
pub(crate) struct Prepared {
    pub artifact: export::msp::PreparedMsp,
    pub preview: Option<egui::ColorImage>,
    // Local exact charges are covered by this already admitted executor lease.
    _outer: ResourceLease,
    _render: BudgetLease,
    _encode: BudgetLease,
    #[cfg(test)]
    pub(super) account: SharedBudget,
}
pub(super) fn version(value: Version) -> msp::Version {
    match value {
        Version::UncompressedV1 => msp::Version::UncompressedV1,
        Version::RowRleV2 => msp::Version::RowRleV2,
    }
}
fn priced(layout: msp::RgbaLayout, resident: u64) -> Result<u64> {
    export::msp::render_reservation_bytes(layout)?
        .checked_add(resident)
        // Existing destination capture, write staging buffer and small display copies.
        .and_then(|n| n.checked_add(msp::BYTES_MAX as u64))
        .and_then(|n| n.checked_add(u64::from(PREVIEW_SIDE).pow(2) * 4 * 3))
        .and_then(|n| n.checked_add(128 << 10))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "MSP preparation quote overflow"))
}
pub(super) fn quote(doc: &paintcraft_document::Document, selected: Version) -> Result<u64> {
    let layout = msp::RgbaLayout::new(version(selected), doc.canvas.0, doc.canvas.1)?;
    let resident = doc.layers.values().try_fold(0u64, |sum, layer| {
        (layer.raster.resident_tiles() as u64)
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .and_then(|bytes| sum.checked_add(bytes))
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "MSP retained source overflow"))
    })?;
    priced(layout, resident)
}
pub(super) fn prepare(
    doc: Arc<paintcraft_document::Document>,
    selected: Version,
    quoted: u64,
    capacity: u64,
    cancel: Cancellation,
    policy: export::metadata_policy::Policy,
) -> Result<Prepared> {
    prepare_with(
        doc,
        selected,
        quoted,
        capacity,
        cancel,
        policy,
        |doc, check| paintcraft_render::render_cancellable(doc, check),
    )
}
pub(super) fn prepare_with(
    doc: Arc<paintcraft_document::Document>,
    selected: Version,
    quoted: u64,
    capacity: u64,
    cancel: Cancellation,
    policy: export::metadata_policy::Policy,
    render_pixels: impl FnOnce(
        &paintcraft_document::Document,
        &dyn Fn() -> Result<()>,
    ) -> Result<Vec<u8>>,
) -> Result<Prepared> {
    let outer = cancel.resource_lease().ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "MSP preparation admission missing",
        )
    })?;
    let check = || cancel.check();
    // Nested accounting, never a second reservation from the global executor realm.
    let budget = SharedBudget::new(capacity);
    let render = RefCell::new(None);
    let encode = RefCell::new(None);
    let artifact = export::msp::prepare_document_with_metadata_policy(
        &doc,
        version(selected),
        &export::KNOWN_LOSSES.map(str::to_owned),
        |layout, resident| {
            if priced(layout, resident)? != quoted {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "MSP quote changed",
                ));
            }
            *render.borrow_mut() = Some(budget.reserve(quoted)?);
            check()
        },
        |plan| {
            *encode.borrow_mut() = Some(budget.reserve(plan.additional_bytes() as u64)?);
            check()
        },
        render_pixels,
        &check,
        policy,
    )?;
    // Composite has been released. Decode uses its RGBA allowance and the
    // packed allowance already charged above; encoded bytes stay retained.
    let decoded = msp::decode(artifact.bytes(), &check)?;
    let layout = artifact.plan().layout();
    if decoded.header().version() != layout.version()
        || decoded.header().width() != layout.width()
        || decoded.header().height() != layout.height()
    {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "MSP preview plan changed",
        ));
    }
    let rgba = decoded.rgba(&check)?;
    let (width, height) = (layout.width(), layout.height());
    let divisor = f64::from(width.max(height)) / f64::from(PREVIEW_SIDE);
    let w = (f64::from(width) / divisor.max(1.0)).floor().max(1.0) as u32;
    let h = (f64::from(height) / divisor.max(1.0)).floor().max(1.0) as u32;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact((w * h) as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "MSP preview allocation"))?;
    for y in 0..h {
        check()?;
        for x in 0..w {
            let at = ((u64::from(y) * u64::from(height) / u64::from(h)) * u64::from(width)
                + u64::from(x) * u64::from(width) / u64::from(w)) as usize
                * 4;
            pixels.push(egui::Color32::from_rgba_unmultiplied(
                rgba[at],
                rgba[at + 1],
                rgba[at + 2],
                rgba[at + 3],
            ));
        }
    }
    check()?;
    Ok(Prepared {
        artifact,
        preview: Some(egui::ColorImage::new([w as usize, h as usize], pixels)),
        _outer: outer,
        #[cfg(test)]
        account: budget,
        _render: render.into_inner().ok_or_else(stale)?,
        _encode: encode.into_inner().ok_or_else(stale)?,
    })
}
