//! One immutable monochrome composite with exact two-stage caller admission.
use super::*;
use crate::msp::{self as codec, RgbaLayout, RgbaWritePlan, Version};
/// Bounded actual common loss strings/plan receipt payload; not allocator RSS.
pub const METADATA_BYTES: u64 = 2048;
pub fn render_reservation_bytes(layout: RgbaLayout) -> Result<u64> {
    (layout.render_bytes() as u64)
        .checked_add(METADATA_BYTES)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "MSP preparation reservation overflow",
            )
        })
}
#[derive(Debug)]
pub struct PreparedMsp {
    plan: RgbaWritePlan,
    bytes: Vec<u8>,
    losses: Vec<String>,
    source_state: uuid::Uuid,
}
impl PreparedMsp {
    pub fn plan(&self) -> RgbaWritePlan {
        self.plan
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    pub fn source_state(&self) -> uuid::Uuid {
        self.source_state
    }
    pub fn into_parts(self) -> (Vec<u8>, Vec<String>) {
        (self.bytes, self.losses)
    }
}
/// admit_render runs before the ONE render, passing checked retained source
/// raster payload separately from render_reservation_bytes(layout). admit_encode
/// runs after counting but before full packed/output allocation. Its exact
/// additional_bytes quote excludes the first-stage allowance; retain BOTH leases
/// through the prepared result and publication. No executor or publication owned.
#[allow(clippy::too_many_arguments)]
pub fn prepare_document_with(
    doc: &Document,
    version: Version,
    allowed: &[String],
    admit_render: impl FnOnce(RgbaLayout, u64) -> Result<()>,
    admit_encode: impl FnOnce(RgbaWritePlan) -> Result<()>,
    render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedMsp> {
    check()?;
    crate::export::require_metadata_policy(doc)?;
    check_loss_codes(allowed)?;
    let layout = RgbaLayout::new(version, doc.canvas.0, doc.canvas.1)?;
    render_reservation_bytes(layout)?;
    let mut resident = 0u64;
    for layer in doc.layers.values() {
        check()?;
        let bytes = (layer.raster.resident_tiles() as u64)
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "MSP retained source overflow"))?;
        resident = resident
            .checked_add(bytes)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "MSP retained source overflow"))?;
    }
    admit_render(layout, resident)?;
    check()?;
    let FlatComposite { rgba, losses } = prepare_composite(doc, |doc| render(doc, check), check)?;
    require_losses("MSP", &losses, allowed)?;
    let plan = codec::preflight_rgba_exact(layout, &rgba, check)?;
    admit_encode(plan)?;
    check()?;
    let bytes = plan.encode(&rgba, check)?;
    check()?;
    Ok(PreparedMsp {
        plan,
        bytes,
        losses,
        source_state: doc.state_id,
    })
}
#[cfg(test)]
mod tests;
