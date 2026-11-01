//! Final source-local alpha coverage, separate from editable filter stages.
//! Structural query pricing: see the pinned revision1 source-action ledger.
use crate::*;
use paintcraft_document::Layer;

pub(crate) type Identity = Option<(bool, [u8; 32])>;
pub(crate) fn identity(layer: &Layer) -> Identity {
    layer
        .mask
        .as_ref()
        .map(|m| (m.enabled(), m.coverage().digest()))
}
pub(crate) fn active(layer: &Layer) -> bool {
    layer.mask.as_ref().is_some_and(|m| m.enabled())
}
pub(crate) fn active_document(doc: &Document) -> bool {
    doc.layers.values().any(active)
}
pub(crate) fn relevant(doc: &Document, region: Rect) -> bool {
    doc.layers_in_order().any(|l| {
        l.visible && l.opacity != 0 && active(l) && region.intersect(&l.extent()).is_some()
    })
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Layer mask appearance requires an explicit shared workspace admission",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Layer mask structural work overflows caller admission",
    )
}
pub(crate) fn require_document(doc: &Document) -> Result<()> {
    if active_document(doc) && crate::spatial::current().is_none() {
        return Err(missing());
    }
    Ok(())
}
/// Backing storage, not clipped canonical stats, bounds both binary searches.
/// Each search has ceil(log2(n))+1 comparisons and storage bytes >=2*n.
/// None/disabled retains the old callback-free work quote exactly.
pub(crate) fn work(layer: &Layer, samples: u64) -> Result<u64> {
    let Some(mask) = layer.mask.as_ref().filter(|m| m.enabled()) else {
        return Ok(0);
    };
    let bytes = mask
        .coverage()
        .storage_usage()
        .map_or(0, |(_, bytes)| bytes);
    let bits = u64::from(u64::BITS - bytes.leading_zeros());
    samples.checked_mul(48 + 2 * bits).ok_or_else(limit)
}
pub(crate) fn reserve(layer: &Layer, samples: u64) -> Result<()> {
    if !active(layer) {
        return Ok(());
    }
    let a = crate::spatial::current().ok_or_else(missing)?;
    let work = work(layer, samples)?;
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&a)?.reserve_components(crate::noise_work::WorkComponents {
            base: work,
            noise: 0,
        })
    } else {
        a.reserve_work(work)
    }
}
/// The caller has reserved this exact final-output pass before extraction or
/// writes. Spatial plans use their whole-plan reservation; rows use reserve().
pub(crate) fn apply_precharged(layer: &Layer, point: (i64, i64), pixel: &mut Rgba) {
    if let Some(mask) = layer.mask.as_ref().filter(|m| m.enabled()) {
        let m = mask.coverage().coverage(point.0, point.1);
        pixel.0[3] = ((u16::from(pixel.0[3]) * u16::from(m) + 127) / 255) as u8;
    }
}

#[cfg(test)]
#[path = "layer_mask/tests.rs"]
mod tests;

/// Pure revision1 own-mask base work for a final source-local output rectangle.
/// Validates the finite source even for None/disabled masks. No clipping, halo,
/// counters, callbacks, owners or allocation; callers walk their actual output
/// tiles and defer this subtotal from any bulk debit before renderer execution.
/// Enabled zero/full fields retain their fixed structural query price. Disabled
/// and absent fields cost zero; this does not grant capability negotiation.
pub fn quote_work(layer: &Layer, output: Rect) -> Result<u64> {
    if output.x0 < 0
        || output.y0 < 0
        || output.x1 <= output.x0
        || output.y1 <= output.y0
        || output.x1 > i64::from(layer.raster.width())
        || output.y1 > i64::from(layer.raster.height())
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Layer mask output is outside its finite source",
        ));
    }
    // Finite u32 raster dimensions bound these subtractions before Rect::area.
    let samples = u64::try_from(output.area()).map_err(|_| limit())?;
    work(layer, samples)
}

#[cfg(test)]
#[path = "layer_mask/quote_tests.rs"]
mod quote_tests;
