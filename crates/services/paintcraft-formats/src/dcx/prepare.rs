//! One composite document produces exactly one page, using the shared PCX policy.
use super::*;
use crate::{export, image::ImageFormat, pcx};
use paintcraft_document::Document;

#[derive(Clone, Copy, Debug)]
pub struct DocumentLimits {
    pub pixels: u64,
    pub source_resident_bytes: usize,
    pub encoded_bytes: usize,
    pub transient_bytes: usize,
}
impl Default for DocumentLimits {
    fn default() -> Self {
        Self {
            pixels: pcx::PIXELS_MAX,
            source_resident_bytes: 64 << 20,
            encoded_bytes: 64 << 20,
            transient_bytes: 256 << 20,
        }
    }
}
/// Immutable encoded page and inherited explicit PCX loss policy. No document
/// layers are mapped to extra pages; supplied palette mode is never implicit.
#[derive(Debug)]
pub struct PreparedDocument {
    bytes: Vec<u8>,
    losses: Vec<String>,
    mode: pcx::Mode,
}
impl PreparedDocument {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    pub fn mode(&self) -> pcx::Mode {
        self.mode
    }
    pub fn into_export(self) -> export::FlatExport {
        export::FlatExport {
            bytes: self.bytes,
            losses: self.losses,
        }
    }
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource("DCX document size overflow"))
}
/// Prepare through the existing default compositor, checking cancellation per
/// renderer tile/row and during PCX conversion/encoding/container copy.
pub fn prepare_document(
    doc: &Document,
    mode: pcx::Mode,
    allowed: &[String],
    limits: DocumentLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<PreparedDocument> {
    prepare_document_with(doc, mode, allowed, limits, check, |doc| {
        paintcraft_render::render_cancellable(doc, check)
    })
}
/// Caller-owned rendering supports the existing runtime pool. Caller retains
/// source/document ownership, scheduling and staged-output publication authority.
pub fn prepare_document_with(
    doc: &Document,
    mode: pcx::Mode,
    allowed: &[String],
    limits: DocumentLimits,
    check: &dyn Fn() -> Result<()>,
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
) -> Result<PreparedDocument> {
    prepare_document_with_metadata_policy(
        doc,
        mode,
        allowed,
        limits,
        check,
        render,
        crate::export::metadata_policy::Policy::default(),
    )
}
pub fn prepare_document_with_metadata_policy(
    doc: &Document,
    mode: pcx::Mode,
    allowed: &[String],
    limits: DocumentLimits,
    check: &dyn Fn() -> Result<()>,
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
    policy: crate::export::metadata_policy::Policy,
) -> Result<PreparedDocument> {
    prepare_document_with_policies(
        doc,
        mode,
        allowed,
        limits,
        check,
        render,
        crate::export::CopyPolicies {
            source_metadata: policy,
            ..Default::default()
        },
    )
}
pub fn prepare_document_with_policies(
    doc: &Document,
    mode: pcx::Mode,
    allowed: &[String],
    limits: DocumentLimits,
    check: &dyn Fn() -> Result<()>,
    render: impl FnOnce(&Document) -> Result<Vec<u8>>,
    policies: crate::export::CopyPolicies,
) -> Result<PreparedDocument> {
    check()?;
    let (metadata_omitted, filters_baked) = policies.validate(
        doc,
        crate::export::metadata_policy::Target::new(
            crate::registry::Family::Dcx,
            crate::registry::Operation::ExportFlat,
        ),
        check,
    )?;
    export::check_loss_codes(allowed)?;
    let (width, height) = doc.canvas;
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.pixels.min(pcx::PIXELS_MAX) {
        return Err(resource("DCX document pixel allowance"));
    }
    let pcx_capacity = pcx::encoded_capacity(width, height, (72, 72), mode != pcx::Mode::Rgb24)?;
    let capacity = add(DIRECTORY_BYTES, pcx_capacity)?;
    if capacity > limits.encoded_bytes.min(64 << 20) {
        return Err(resource("DCX document output allowance"));
    }
    let mut resident = 0usize;
    for layer in doc.layers.values() {
        check()?;
        let bytes = layer
            .raster
            .resident_tiles()
            .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES)
            .ok_or_else(|| resource("DCX source resident overflow"))?;
        resident = add(resident, bytes)?;
        if resident > limits.source_resident_bytes {
            return Err(resource("DCX source resident allowance"));
        }
    }
    // Logical peak bound: source tiles + RGBA + RGB + indices + bounded quantizer
    // training/lookup headroom + encoded PCX and outer container. Not an RSS cap.
    // PCX limits training to32768RGBA samples (128KiB). Selected color_quant1.1.0
    // holds256 neurons(32B),colors(16B),indices(usize),bias/frequency(16B);
    // palette/lookup/maps are256entries. The1MiB logical headroom exceeds these
    // fixed payloads; allocator metadata/capacity rounding are not an RSS bound.
    let working = usize::try_from(pixels)
        .ok()
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| resource("DCX preparation size overflow"))?;
    let transient = add(
        add(resident, working)?,
        add(add(capacity, pcx_capacity)?, 1 << 20)?,
    )?;
    if transient > limits.transient_bytes {
        return Err(resource("DCX document transient allowance"));
    }
    let mut options = export::FlatOptions::new(ImageFormat::Pcx, 100);
    options.pcx_mode = mode;
    if matches!(
        policies.source_metadata,
        crate::export::metadata_policy::Policy::Omit(_)
    ) {
        options.source_metadata = crate::export::metadata_policy::Policy::capture(
            doc,
            crate::export::metadata_policy::Target::new(
                crate::registry::Family::Pcx,
                crate::registry::Operation::ExportFlat,
            ),
            crate::export::metadata_policy::Intent::Omit,
        )?;
    }
    if matches!(
        policies.filter_bake,
        crate::export::filter_bake::Policy::Bake(_)
    ) {
        options.filter_bake = crate::export::filter_bake::Policy::capture(
            doc,
            crate::export::metadata_policy::Target::new(
                crate::registry::Family::Pcx,
                crate::registry::Operation::ExportFlat,
            ),
            crate::export::filter_bake::Intent::Bake,
            check,
        )?;
    }
    debug_assert_eq!(
        filters_baked,
        !doc.layers.values().all(|layer| layer.filters.is_empty())
            && matches!(
                policies.filter_bake,
                crate::export::filter_bake::Policy::Bake(_)
            )
    );
    debug_assert_eq!(
        metadata_omitted,
        doc.source_metadata.is_some()
            && matches!(
                policies.source_metadata,
                crate::export::metadata_policy::Policy::Omit(_)
            )
    );
    let flat = export::export_flat_options_with_check(doc, options, allowed, render, check)?;
    check()?;
    let (actual_w, actual_h) = image::inspect_pcx(&flat.bytes, pixels)?;
    if (actual_w, actual_h) != (width, height) || flat.bytes.len() > pcx_capacity {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "DCX prepared PCX disagrees with admission",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| resource("Cannot allocate DCX document output"))?;
    bytes.resize(DIRECTORY_BYTES, 0);
    bytes[..4].copy_from_slice(&MAGIC.to_le_bytes());
    bytes[4..8].copy_from_slice(&(DIRECTORY_BYTES as u32).to_le_bytes());
    for chunk in flat.bytes.chunks(65536) {
        check()?;
        bytes.extend_from_slice(chunk);
    }
    check()?;
    Ok(PreparedDocument {
        bytes,
        losses: flat.losses,
        mode,
    })
}
