//! Frozen, independently fitted entry previews from one immutable document.
use super::*;
use paintcraft_document::Document;

pub const KNOWN_EXPORT_LOSSES: &[&str] = &[
    "flatten",
    "hidden-rgb",
    "icon-resized",
    crate::export::filter_bake::BAKED_REASON,
];

pub fn check_export_loss_codes(allowed: &[String]) -> Result<()> {
    if let Some(code) = allowed
        .iter()
        .find(|code| !KNOWN_EXPORT_LOSSES.contains(&code.as_str()))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            format!("unknown icon export loss {code:?}"),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntrySpec {
    pub width: u32,
    pub height: u32,
    pub encoding: Encoding,
    pub hotspot: Option<(u16, u16)>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    #[default]
    Lanczos3,
}
#[derive(Debug)]
pub struct PreparedEntry {
    pub spec: EntrySpec,
    rgba: Vec<u8>,
}
impl PreparedEntry {
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    fn borrowed(&self) -> WriteEntry<'_> {
        WriteEntry {
            width: self.spec.width,
            height: self.spec.height,
            rgba: &self.rgba,
            encoding: self.spec.encoding,
            hotspot: self.spec.hotspot,
        }
    }
}
#[derive(Debug)]
pub struct Prepared {
    kind: Kind,
    entries: Vec<PreparedEntry>,
    losses: Vec<String>,
    metadata_omitted: bool,
    filters_baked: bool,
    limits: WriteLimits,
}
impl Prepared {
    pub fn entries(&self) -> &[PreparedEntry] {
        &self.entries
    }
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    /// Encode the exact prepared preview buffers after every loss is accepted.
    pub fn encode(&self, allowed: &[String], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        check_export_loss_codes(allowed)?;
        let missing: Vec<_> = self
            .losses
            .iter()
            .filter(|code| {
                !allowed.contains(code)
                    && !(self.metadata_omitted
                        && code.as_str() == crate::export::metadata_policy::OMITTED_REASON)
                    && !(self.filters_baked
                        && code.as_str() == crate::export::filter_bake::BAKED_REASON)
            })
            .collect();
        if !missing.is_empty() {
            return Err(unsupported(&format!(
                "icon export would lose {}; acknowledge with --allow-loss",
                missing
                    .iter()
                    .map(|code| code.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )));
        }
        let entries: Vec<_> = self.entries.iter().map(PreparedEntry::borrowed).collect();
        encode_entries(self.kind, &entries, self.limits, check)
    }
}

fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn allocate<T>(length: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| resource("cannot allocate icon preparation buffer"))?;
    Ok(values)
}

fn fit(source: (u32, u32), target: (u32, u32)) -> (u32, u32) {
    let (width, height) = source;
    let (tw, th) = target;
    if u64::from(width) * u64::from(th) >= u64::from(height) * u64::from(tw) {
        (
            tw,
            (u64::from(height) * u64::from(tw) / u64::from(width)).max(1) as u32,
        )
    } else {
        (
            (u64::from(width) * u64::from(th) / u64::from(height)).max(1) as u32,
            th,
        )
    }
}

/// Fit each selected size independently, preserving aspect ratio and centering
/// on transparent padding. Nearest samples pixel centers. Lanczos3 filters
/// premultiplied encoded-sRGB samples, then unpremultiplies and rounds to RGBA8.
/// Source and aggregate destination budgets are soft admission bounds, not a
/// hard upstream scratch-memory or within-call cancellation-tail guarantee.
pub fn prepare_document(
    doc: &Document,
    kind: Kind,
    specs: &[EntrySpec],
    filter: Filter,
    limits: WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    prepare_document_with_metadata_policy(
        doc,
        kind,
        specs,
        filter,
        limits,
        check,
        crate::export::metadata_policy::Policy::default(),
    )
}
pub fn prepare_document_with_metadata_policy(
    doc: &Document,
    kind: Kind,
    specs: &[EntrySpec],
    filter: Filter,
    limits: WriteLimits,
    check: &dyn Fn() -> Result<()>,
    policy: crate::export::metadata_policy::Policy,
) -> Result<Prepared> {
    prepare_document_with_policies(
        doc,
        kind,
        specs,
        filter,
        limits,
        check,
        crate::export::CopyPolicies {
            source_metadata: policy,
            ..Default::default()
        },
    )
}
pub fn prepare_document_with_policies(
    doc: &Document,
    kind: Kind,
    specs: &[EntrySpec],
    filter: Filter,
    limits: WriteLimits,
    check: &dyn Fn() -> Result<()>,
    policies: crate::export::CopyPolicies,
) -> Result<Prepared> {
    let target = crate::export::metadata_policy::Target::new(
        match kind {
            Kind::Icon => crate::registry::Family::Ico,
            Kind::Cursor => crate::registry::Family::Cur,
        },
        crate::registry::Operation::ExportAsset,
    );
    let metadata_omitted = policies.source_metadata.validate(doc, target)?;
    if specs.is_empty() || specs.len() > limits.entries.min(ENTRIES_MAX) {
        return Err(resource("icon output entry count outside writer budget"));
    }
    let mut pixels = 0u64;
    for spec in specs {
        pixels += write::admit_layout(kind, spec.width, spec.height, spec.hotspot)?;
    }
    if pixels > limits.pixels.min(WRITE_PIXELS_MAX)
        || 6 + specs.len() * 16 > limits.encoded_bytes.min(ENCODED_BYTES_MAX)
    {
        return Err(resource("icon output set exceeds preparation budget"));
    }
    admit_source(doc)?;
    let filters_baked = policies.filter_bake.validate(doc, target, check)?;
    let sizes: Vec<_> = specs.iter().map(|spec| (spec.width, spec.height)).collect();
    let (buffers, mut losses) = prepare_pixels(doc, &sizes, filter, check)?;
    if metadata_omitted {
        losses.push(crate::export::metadata_policy::OMITTED_REASON.to_owned());
    }
    if filters_baked {
        losses.push(crate::export::filter_bake::BAKED_REASON.to_owned());
    }
    let entries: Vec<_> = specs
        .iter()
        .copied()
        .zip(buffers)
        .map(|(spec, rgba)| PreparedEntry { spec, rgba })
        .collect();
    let borrowed: Vec<_> = entries.iter().map(PreparedEntry::borrowed).collect();
    write::preflight(kind, &borrowed, limits)?;
    check()?;
    Ok(Prepared {
        kind,
        entries,
        losses,
        metadata_omitted,
        filters_baked,
        limits,
    })
}

/// Pure source-layout admission shared by icon/ICNS preparation, before callback
/// work or pixel allocation. This does not authorize any editable-data loss.
pub(crate) fn admit_source(doc: &Document) -> Result<()> {
    if doc.canvas.0 == 0
        || doc.canvas.1 == 0
        || u64::from(doc.canvas.0) * u64::from(doc.canvas.1) > WRITE_PIXELS_MAX
    {
        return Err(resource("icon source exceeds preparation pixel budget"));
    }
    Ok(())
}

/// Shared fitted pixel preparation for explicit ICO/CUR and ICNS entry sets.
/// Type/density selection and encoding admission remain with each container.
pub(crate) fn prepare_pixels(
    doc: &Document,
    sizes: &[(u32, u32)],
    filter: Filter,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Vec<Vec<u8>>, Vec<String>)> {
    if sizes.is_empty() || sizes.len() > ENTRIES_MAX {
        return Err(resource(
            "icon output entry count outside preparation budget",
        ));
    }
    let mut pixels = 0u64;
    for &(width, height) in sizes {
        if width == 0 || height == 0 {
            return Err(resource("empty icon output dimensions"));
        }
        pixels = pixels
            .checked_add(u64::from(width) * u64::from(height))
            .filter(|count| *count <= WRITE_PIXELS_MAX)
            .ok_or_else(|| resource("icon output set exceeds preparation pixel budget"))?;
    }
    admit_source(doc)?;
    check()?;
    let rgba = paintcraft_render::render_cancellable(doc, check)?;
    let mut losses = Vec::new();
    if doc.roots.len() > 1 {
        losses.push("flatten".into());
    }
    if doc
        .layers
        .values()
        .any(|layer| layer.raster.has_hidden_rgb())
    {
        losses.push("hidden-rgb".into());
    }
    if sizes.iter().any(|size| *size != doc.canvas) {
        losses.push("icon-resized".into());
    }
    let mut entries = allocate(sizes.len())?;
    let mut premultiplied = None;
    for &(target_width, target_height) in sizes {
        check()?;
        let (width, height) = fit(doc.canvas, (target_width, target_height));
        let mut fitted = allocate(target_width as usize * target_height as usize * 4)?;
        fitted.resize(target_width as usize * target_height as usize * 4, 0u8);
        let offset = ((target_width - width) / 2, (target_height - height) / 2);
        let sampled = if filter == Filter::Lanczos3 && (width, height) != doc.canvas {
            if premultiplied.is_none() {
                premultiplied = Some(premultiply(&rgba, doc.canvas, check)?);
            }
            Some(lanczos(
                premultiplied.as_ref().unwrap(),
                (width, height),
                check,
            )?)
        } else {
            None
        };
        for y in 0..height {
            check()?;
            for x in 0..width {
                let pixel = if let Some(sampled) = &sampled {
                    &sampled[((y * width + x) * 4) as usize..][..4]
                } else {
                    let sx = ((u64::from(x) * 2 + 1) * u64::from(doc.canvas.0)
                        / (u64::from(width) * 2)) as u32;
                    let sy = ((u64::from(y) * 2 + 1) * u64::from(doc.canvas.1)
                        / (u64::from(height) * 2)) as u32;
                    &rgba
                        [((u64::from(sy) * u64::from(doc.canvas.0) + u64::from(sx)) * 4) as usize..]
                        [..4]
                };
                let target = (((y + offset.1) * target_width + x + offset.0) * 4) as usize;
                fitted[target..target + 4].copy_from_slice(pixel);
            }
        }
        entries.push(fitted);
    }
    check()?;
    Ok((entries, losses))
}

fn premultiply(
    rgba: &[u8],
    source: (u32, u32),
    check: &dyn Fn() -> Result<()>,
) -> Result<::image::Rgba32FImage> {
    let mut values = allocate(rgba.len())?;
    for chunk in rgba.chunks(65536) {
        check()?;
        for pixel in chunk.as_chunks::<4>().0 {
            let alpha = f32::from(pixel[3]) / 255.0;
            values.extend([
                f32::from(pixel[0]) / 255.0 * alpha,
                f32::from(pixel[1]) / 255.0 * alpha,
                f32::from(pixel[2]) / 255.0 * alpha,
                alpha,
            ]);
        }
    }
    let image = ::image::Rgba32FImage::from_raw(source.0, source.1, values)
        .ok_or_else(|| malformed("invalid icon resize source"))?;
    Ok(image)
}

fn lanczos(
    image: &::image::Rgba32FImage,
    size: (u32, u32),
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let resized = ::image::imageops::resize(
        image,
        size.0,
        size.1,
        ::image::imageops::FilterType::Lanczos3,
    );
    check()?;
    let mut result = allocate(size.0 as usize * size.1 as usize * 4)?;
    for pixel in resized.pixels() {
        let alpha = pixel.0[3].clamp(0.0, 1.0);
        let a = (alpha * 255.0).round() as u8;
        if a == 0 {
            result.extend([0; 4]);
        } else {
            result.extend([
                (pixel.0[0] / alpha * 255.0).clamp(0.0, 255.0).round() as u8,
                (pixel.0[1] / alpha * 255.0).clamp(0.0, 255.0).round() as u8,
                (pixel.0[2] / alpha * 255.0).clamp(0.0, 255.0).round() as u8,
                a,
            ]);
        }
    }
    Ok(result)
}
