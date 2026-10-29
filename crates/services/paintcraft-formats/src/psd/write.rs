//! Fresh flat RGB8 raster interchange only. No preserving-save authority.
//! Selected encoding has no cooperative callback; check immediately around its
//! bounded call. Logical allocation estimates are not an OS/RSS allocator cap.
use super::*;
use paintcraft_document::Document;
use std::io::Write;
mod model;
mod profile;
pub const TITLE_LOSS: &str = "psd-document-title-not-exported";
#[derive(Clone, Copy, Debug)]
pub struct WriteLimits {
    pub encoded_bytes: usize,
    pub metadata_bytes: usize,
    pub transient_bytes: u64,
    pub pixels: u64,
    pub layers: usize,
    pub name_utf16: usize,
}
impl Default for WriteLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: ENCODED_MAX,
            metadata_bytes: METADATA_MAX,
            transient_bytes: 512 << 20,
            pixels: PIXELS_MAX,
            layers: LAYERS_MAX,
            name_utf16: LAYER_NAME_UTF16_MAX,
        }
    }
}
#[derive(Clone, Debug)]
pub struct WritePlan {
    pub version: Version,
    pub layers: usize,
    pub layer_pixels: u64,
    pub composite_pixels: u64,
    /// Private selected-writer capacity, including the inserted ICC resource.
    pub output_capacity: usize,
    /// Selected writer's audited composite PackBits scratch allocation.
    pub packbits_scratch: usize,
    /// Conservative closed-model copies, row IO, output and codec scratch.
    pub transient_bytes: u64,
    pub losses: Vec<String>,
}
impl WritePlan {
    /// Resolve only the named losses from this exact preflight before staging.
    pub fn require_consent(&self, allowed: &[String]) -> Result<()> {
        consent(self, allowed)
    }
}
fn sum(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| resource("PSD writer size overflow"))
}
fn product(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| resource("PSD writer size overflow"))
}
fn dimension(width: u32, height: u32, version: Version, limit: u64) -> Result<u64> {
    let side = match version {
        Version::Psd => 30000,
        Version::Psb => 300000,
    };
    if width == 0 || height == 0 || width > side || height > side {
        return Err(unsupported(
            "PSD/PSB writer dimensions outside selected version",
        ));
    }
    let pixels = product(u64::from(width), u64::from(height))?;
    if pixels > limit.min(PIXELS_MAX) {
        return Err(resource("PSD writer pixel allowance"));
    }
    Ok(pixels)
}
/// Validate the complete document before allocating pixel/model/output buffers.
/// Core documents are flat Normal straight encoded-sRGB RGBA8 by contract; no
/// unsupported Photoshop source objects or opaque source bytes are copied.
pub fn preflight(
    doc: &Document,
    version: Version,
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WritePlan> {
    // Exhaustive source patterns make future document/layer semantics a compile
    // review point instead of silently dropping newly introduced properties.
    let Document {
        document_id: _,
        state_id: _,
        canvas: _,
        roots: _,
        layers: _,
        title: _,
        source_metadata: _,
    } = doc;
    if doc.source_metadata.is_some() {
        return Err(unsupported(
            "Source metadata preservation is not qualified for this foreign writer",
        ));
    }
    check()?;
    let composite_pixels = dimension(doc.canvas.0, doc.canvas.1, version, limits.pixels)?;
    if doc.roots.is_empty() {
        return Err(unsupported(
            "PSD writer requires explicit editable raster layers",
        ));
    }
    if doc.roots.len() != doc.layers.len() {
        return Err(malformed("PSD writer layer inventory is inconsistent"));
    }
    if doc.roots.len() > limits.layers.min(LAYERS_MAX) {
        return Err(resource("PSD writer layer count allowance"));
    }
    let mut layer_pixels = 0;
    let mut names = 0;
    let mut largest_row = 0;
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id && !doc.roots[..index].contains(id))
            .ok_or_else(|| malformed("PSD writer duplicate or missing layer"))?;
        let paintcraft_document::Layer {
            id: _,
            name: _,
            origin: _,
            visible: _,
            opacity: _,
            raster: _,
        } = layer;
        let width = layer.raster.width();
        let height = layer.raster.height();
        let pixels = dimension(width, height, version, limits.pixels)?;
        layer_pixels = sum(layer_pixels, pixels)?;
        if layer_pixels > limits.pixels.min(PIXELS_MAX) {
            return Err(resource("PSD writer aggregate layer allowance"));
        }
        for (origin, size) in [(layer.origin.0, width), (layer.origin.1, height)] {
            let end = origin
                .checked_add(i64::from(size))
                .ok_or_else(|| unsupported("PSD layer bounds overflow"))?;
            if i32::try_from(origin).is_err() || i32::try_from(end).is_err() {
                return Err(unsupported("PSD layer bounds exceed signed32"));
            }
        }
        if layer.name.len() > limits.metadata_bytes.min(METADATA_MAX) {
            return Err(resource("PSD layer name exceeds metadata allowance"));
        }
        let chars = layer.name.encode_utf16().count();
        if chars > limits.name_utf16.min(LAYER_NAME_UTF16_MAX) || layer.name.contains('\0') {
            return Err(unsupported("PSD writer layer name not representable"));
        }
        names = sum(names, product(chars as u64, 2)?)?;
        largest_row = largest_row.max(u64::from(width));
    }
    let metadata = sum(4096, sum(product(doc.roots.len() as u64, 512)?, names)?)?;
    if metadata > limits.metadata_bytes.min(METADATA_MAX) as u64 {
        return Err(resource("PSD writer metadata allowance"));
    }
    // Selected rle_scratch_size: (2*w*h + row_table_bytes*h)*4. Raw layer
    // channels avoid a second layer PackBits policy; only composite uses this.
    let row_bytes = if version == Version::Psb { 4 } else { 2 };
    let scratch = product(
        sum(
            product(composite_pixels, 2)?,
            product(u64::from(doc.canvas.1), row_bytes)?,
        )?,
        4,
    )?;
    let layer_bytes = product(layer_pixels, 4)?;
    let composite_bytes = product(composite_pixels, 4)?;
    let capacity = sum(
        sum(layer_bytes, scratch)?,
        sum(metadata, profile::RESOURCE_BYTES as u64)?,
    )?;
    if capacity > limits.encoded_bytes.min(ENCODED_MAX) as u64 {
        return Err(resource("PSD writer encoded allowance"));
    }
    // Model -> flattened list -> layer channel records retain three copies of
    // typed raw layers. Allow one extra full layer set, model/working composite
    // plus a worst-case PackBits result up to twice its bytes, doubled output
    // growth defensively, scratch and row conversion ownership.
    let structs = product(
        doc.roots.len() as u64,
        std::mem::size_of::<ag_psd::psd::Layer>() as u64,
    )?;
    let transient = sum(
        sum(product(layer_bytes, 4)?, product(composite_bytes, 4)?)?,
        sum(
            sum(product(capacity, 2)?, scratch)?,
            sum(
                product(structs, 4)?,
                sum(product(names, 4)?, product(largest_row, 4)?)?,
            )?,
        )?,
    )?;
    let transient = sum(
        transient,
        sum(
            product(std::mem::size_of::<ag_psd::psd::Psd>() as u64, 2)?,
            sum(
                product(
                    product(doc.roots.len() as u64, 16)?,
                    std::mem::size_of::<ag_psd::psd::LayerRawDataChannel>() as u64,
                )?,
                product(metadata, 4)?,
            )?,
        )?,
    )?;
    if transient > limits.transient_bytes {
        return Err(resource(
            "PSD writer transient declaration exceeds allowance",
        ));
    }
    Ok(WritePlan {
        version,
        layers: doc.roots.len(),
        layer_pixels,
        composite_pixels,
        output_capacity: usize::try_from(capacity)
            .map_err(|_| resource("PSD output address space"))?,
        packbits_scratch: usize::try_from(scratch)
            .map_err(|_| resource("PSD scratch address space"))?,
        transient_bytes: transient,
        losses: if doc.title.is_some() {
            vec![TITLE_LOSS.into()]
        } else {
            vec![]
        },
    })
}
fn consent(plan: &WritePlan, allowed: &[String]) -> Result<()> {
    if allowed.iter().any(|code| code != TITLE_LOSS) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Unknown PSD fresh-copy loss consent",
        ));
    }
    if plan.losses.iter().any(|code| !allowed.contains(code)) {
        return Err(unsupported(
            "PSD fresh copy requires document-title omission consent",
        ));
    }
    Ok(())
}
/// Private output is returned only after selected encoding, resource insertion,
/// final structural checks and cancellation. Caller owns source/output guards.
pub fn encode_document(
    doc: &Document,
    version: Version,
    limits: &WriteLimits,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let plan = preflight(doc, version, limits, check)?;
    consent(&plan, allowed)?;
    let model = model::build(doc, version, check)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(plan.output_capacity)
        .map_err(|_| resource("PSD private output allocation"))?;
    buffer.resize(plan.output_capacity, 0);
    let mut writer = ag_psd::writer::PsdWriter {
        buffer,
        offset: 0,
        temp_buffer: None,
    };
    let options = ag_psd::psd::WriteOptions {
        psb: Some(version == Version::Psb),
        trim_image_data: Some(false),
        no_background: Some(true),
        generate_thumbnail: Some(false),
        compress: Some(false),
        ..Default::default()
    };
    check()?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ag_psd::writer::write_psd_to_writer(&mut writer, &model, &options)
    }))
    .map_err(|_| malformed("Selected PSD writer refused admitted model"))?;
    check()?;
    drop(model);
    if writer.buffer.len() != plan.output_capacity
        || writer.offset > plan.output_capacity
        || writer
            .temp_buffer
            .as_ref()
            .is_none_or(|scratch| scratch.len() > plan.packbits_scratch)
    {
        return Err(resource(
            "Selected PSD writer exceeded its audited declaration",
        ));
    }
    writer.buffer.truncate(writer.offset);
    drop(writer.temp_buffer);
    profile::insert(&mut writer.buffer, version, plan.output_capacity, check)?;
    let inspection = inspect(&writer.buffer, limits.pixels, check)?;
    if inspection.version != version
        || inspection.width != doc.canvas.0
        || inspection.height != doc.canvas.1
        || inspection.layers.len() != doc.roots.len()
    {
        return Err(malformed("PSD writer structural qualification failed"));
    }
    check()?;
    Ok(writer.buffer)
}
/// Write only to a private staged sink. Atomic publication and source/owner
/// guards remain the existing caller's responsibility, never this codec's.
pub fn write_document_to(
    doc: &Document,
    version: Version,
    limits: &WriteLimits,
    allowed: &[String],
    sink: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let bytes = encode_document(doc, version, limits, allowed, check)?;
    for chunk in bytes.chunks(65536) {
        check()?;
        sink.write_all(chunk)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "Cannot write staged PSD output"))?;
    }
    check()
}
