//! One native sRGB composite becomes one explicit fresh 2D DDS texture.
use super::*;
use crate::dds::{self as texture, EncodedTexture, TextureOptions, TexturePlan};
mod transfer;
pub use transfer::{LINEAR_POLICY, LINEAR_QUANTIZATION_LOSS, TransferReceipt};
/// At most five actual loss names, their Vec/String payloads and bounded missing-
/// consent diagnostics. This is logical metadata payload, not allocator/RSS.
pub const METADATA_PAYLOAD_BYTES: u64 = 2048;
pub fn preparation_reservation_bytes(plan: &TexturePlan) -> Result<u64> {
    plan.reservation_bytes()?
        .checked_add(METADATA_PAYLOAD_BYTES)
        .ok_or_else(|| resource("DDS document reservation overflow"))
}
#[derive(Debug)]
pub struct PreparedDds {
    encoded: EncodedTexture,
    losses: Vec<String>,
    transfer: TransferReceipt,
    source_state: uuid::Uuid,
}
impl PreparedDds {
    pub fn encoded(&self) -> &EncodedTexture {
        &self.encoded
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    pub fn transfer(&self) -> TransferReceipt {
        self.transfer
    }
    pub fn source_state(&self) -> uuid::Uuid {
        self.source_state
    }
    /// Additional generated surfaces are derived data, not discarded level zero
    /// or a claim that pre-existing custom mips were preserved.
    pub fn generated_levels(&self) -> usize {
        self.encoded.plan().levels().len() - 1
    }
    pub fn into_parts(self) -> (EncodedTexture, Vec<String>, TransferReceipt) {
        (self.encoded, self.losses, self.transfer)
    }
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
/// Same admitted resource contract as prepare_document_with, with the existing
/// cancellable default-v1 renderer. No session, destination or scheduler owned.
pub fn prepare_document(
    doc: &Document,
    options: TextureOptions,
    allowed: &[String],
    admit: impl FnOnce(&TexturePlan, u64) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedDds> {
    prepare_document_with(
        doc,
        options,
        allowed,
        admit,
        |doc, check| paintcraft_render::render_cancellable(doc, check),
        check,
    )
}
/// `admit` runs before rendering, conversion or codec allocation. The plan's
/// reservation includes the ONE composite RGBA buffer exactly once, plus output,
/// one mip and selected codec payloads. Use preparation_reservation_bytes(plan)
/// to include bounded facade metadata; its second argument is checked retained
/// source raster bytes, separate from that transient quote. The caller retains
/// its resource lease through prepared-result lifetime/publication. Renderer
/// caches or other caller-owned scratch must be priced separately by the caller.
///
/// The renderer must produce native default-v1 straight RGBA8 encoded sRGB.
/// There is no ICC/alternate-precision metadata in Document to assign implicitly.
/// Linear output is a real conversion into 8-bit linear-light sRGB primaries,
/// not retagging. Only the private composite is changed; Document is immutable.
/// Source/destination/owner/cancellation guards at publication remain host-owned.
pub fn prepare_document_with(
    doc: &Document,
    options: TextureOptions,
    allowed: &[String],
    admit: impl FnOnce(&TexturePlan, u64) -> Result<()>,
    render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedDds> {
    check()?;
    check_loss_codes(allowed)?;
    let plan = TexturePlan::new(doc.canvas.0, doc.canvas.1, options)?;
    preparation_reservation_bytes(&plan)?;
    let mut resident = 0u64;
    for layer in doc.layers.values() {
        check()?;
        let bytes = u64::try_from(layer.raster.resident_tiles())
            .ok()
            .and_then(|n| n.checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64))
            .ok_or_else(|| resource("DDS retained source size overflow"))?;
        resident = resident
            .checked_add(bytes)
            .ok_or_else(|| resource("DDS retained source size overflow"))?;
    }
    admit(&plan, resident)?;
    check()?;
    let FlatComposite {
        mut rgba,
        mut losses,
    } = prepare_composite(doc, |doc| render(doc, check), check)?;
    let transfer = transfer::convert(&mut rgba, options.transfer, check)?;
    for loss in plan.losses() {
        append_loss(&mut losses, loss);
    }
    if transfer.quantized_rgb_samples() > 0 {
        append_loss(&mut losses, LINEAR_QUANTIZATION_LOSS);
    }
    // This is the same explicit sample predicate as the backend. Check it before
    // encoding to avoid speculative codec work when its actual loss is unallowed.
    if options.filter == texture::MipFilter::BoxStored && plan.levels().len() > 1 {
        for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            if index.is_multiple_of(1024) {
                check()?;
            }
            if pixel[3] == 0 && pixel[..3] != [0, 0, 0] {
                append_loss(&mut losses, texture::MIP_HIDDEN_RGB_LOSS);
                break;
            }
        }
    }
    require_losses("DDS texture", &losses, allowed)?;
    check()?;
    // Move the admitted plan, including its one Vec inventory, into the same
    // encoder used by the low-level public API. No second plan or renderer.
    let encoded = texture::encode_admitted_texture(plan, &rgba, check)?;
    for loss in encoded.losses() {
        append_loss(&mut losses, loss);
    }
    require_losses("DDS texture", &losses, allowed)?;
    check()?;
    Ok(PreparedDds {
        encoded,
        losses,
        transfer,
        source_state: doc.state_id,
    })
}
fn append_loss(losses: &mut Vec<String>, loss: &str) {
    if !losses.iter().any(|existing| existing == loss) {
        losses.push(loss.to_owned());
    }
}

#[cfg(test)]
mod tests;
