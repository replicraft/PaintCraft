//! Pure opaque RGB8 preparation; no encoder, executor, endpoint or admission gate.
use super::*;
use crate::codec_worker::jxr_encode::Layout;
#[derive(Debug)]
pub struct OpaqueRgb {
    layout: Layout,
    rgb: Vec<u8>,
    losses: Vec<String>,
}
impl OpaqueRgb {
    pub fn layout(&self) -> Layout {
        self.layout
    }
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    /// Move the packed buffer into Arc::new for the supervised encoder without
    /// copying its payload. Caller admission must outlive every retained buffer.
    pub fn into_parts(self) -> (Layout, Vec<u8>, Vec<String>) {
        (self.layout, self.rgb, self.losses)
    }
}
/// `admit` runs before invoking the renderer or allocating packed RGB. Its Layout
/// quote already includes packed RGB exactly once; `rgba_bytes` is a separate
/// caller-composite allowance, not another RGB or resident-tile quote. Document
/// residency/scratch and the lifetime of caller reservations remain caller-owned.
/// The renderer must honor the existing default-v1 pixel/cancellation contract.
/// No matte, alpha loss or metadata loss is inferred from an acknowledgement.
pub fn prepare_opaque_rgb_checked(
    doc: &Document,
    encoded_cap: u64,
    allowed: &[String],
    admit: impl FnOnce(Layout, u64) -> Result<()>,
    render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<OpaqueRgb> {
    prepare_opaque_rgb_with_metadata_policy(
        doc,
        encoded_cap,
        allowed,
        admit,
        render,
        check,
        crate::export::metadata_policy::Policy::default(),
    )
}
pub fn prepare_opaque_rgb_with_metadata_policy(
    doc: &Document,
    encoded_cap: u64,
    allowed: &[String],
    admit: impl FnOnce(Layout, u64) -> Result<()>,
    render: impl FnOnce(&Document, &dyn Fn() -> Result<()>) -> Result<Vec<u8>>,
    check: &dyn Fn() -> Result<()>,
    policy: crate::export::metadata_policy::Policy,
) -> Result<OpaqueRgb> {
    check()?;
    let metadata_omitted = policy.validate(
        doc,
        crate::export::metadata_policy::Target::new(
            crate::registry::Family::Jxr,
            crate::registry::Operation::ExportFlat,
        ),
    )?;
    check_loss_codes(allowed)?;
    let layout = Layout::new(doc.canvas.0, doc.canvas.1, encoded_cap)?;
    let rgba_bytes = u64::from(layout.width())
        .checked_mul(u64::from(layout.height()))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "JXR composite extent overflow"))?;
    admit(layout, rgba_bytes)?;
    check()?;
    let FlatComposite { rgba, mut losses } =
        prepare_composite(doc, |doc| render(doc, check), check)?;
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if index.is_multiple_of(16384) {
            check()?;
        }
        if pixel[3] != 255 {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "JXR RGB8 export requires an entirely opaque composite; no matte or alpha discard is supported",
            ));
        }
    }
    require_losses("JXR RGB8", &losses, allowed)?;
    if metadata_omitted {
        losses.push(crate::export::metadata_policy::OMITTED_REASON.to_owned());
    }
    check()?;
    let length = usize::try_from(layout.rgb_bytes())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR RGB address extent"))?;
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR RGB allocation"))?;
    for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if index.is_multiple_of(16384) {
            check()?;
        }
        rgb.extend_from_slice(&pixel[..3]);
    }
    check()?;
    Ok(OpaqueRgb {
        layout,
        rgb,
        losses,
    })
}
#[cfg(test)]
mod tests;
