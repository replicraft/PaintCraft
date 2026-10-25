//! Explicit SVG rasterization. No filename, network or system-font resolution.
//! Frontend admission and publication remain separate from this private adapter.
use paintcraft_core::types::{Error, ErrorCode, Result};
use resvg::{tiny_skia, usvg};
use std::sync::Arc;
mod document;
mod fonts;
mod input;
mod resources;
#[cfg(test)]
mod tests;
mod tree;
pub use fonts::{FONT_BYTES_MAX, Fonts};

pub const SOURCE_MAX: usize = 8 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
pub const SIDE_MAX: u32 = 16384;
const NODES_MAX: u32 = 16384;
const DEPTH_MAX: usize = 64;
pub const KNOWN_LOSSES: [&str; 2] = ["svg-rasterized", "svg-font-substituted"];
pub const IMPORT_VARIANT: &str = "svg-rasterize-explicit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    pub compressed: bool,
    pub has_text: bool,
}
/// Validate an explicit SVG candidate before selecting rasterization options.
/// This never resolves a font, decodes an image or runs the renderer.
pub fn inspect_source(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<SourceInfo> {
    let source = input::source(bytes, check)?;
    let xml = input::parse(&source, check)?;
    let inventory = input::inspect(&xml, check)?;
    Ok(SourceInfo {
        compressed: bytes.starts_with(&[31, 139]),
        has_text: inventory.has_text,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    Contain,
    Stretch,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
    pub fit: Fit,
}
impl Options {
    pub fn admit(self, pixel_limit: u64) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "SVG output width and height must be positive",
            ));
        }
        if self.width > SIDE_MAX
            || self.height > SIDE_MAX
            || u64::from(self.width) * u64::from(self.height) > pixel_limit.min(PIXELS_MAX)
        {
            return Err(resource("SVG output exceeds rasterization pixel allowance"));
        }
        if !self.dpi.is_finite() || !(1.0..=2400.0).contains(&self.dpi) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "SVG rasterization DPI must be between 1 and 2400",
            ));
        }
        Ok(())
    }
}
pub struct Prepared {
    tree: usvg::Tree,
    transform: tiny_skia::Transform,
    options: Options,
    losses: Vec<String>,
}
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSvg")
            .field("options", &self.options)
            .field("intrinsic_size", &self.intrinsic_size())
            .field("losses", &self.losses)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub losses: Vec<String>,
    options: Options,
}
impl Prepared {
    pub fn losses(&self) -> &[String] {
        &self.losses
    }
    pub fn intrinsic_size(&self) -> (f32, f32) {
        (self.tree.size().width(), self.tree.size().height())
    }
    /// resvg has no cooperative callback within a render call. Checks surround
    /// it and run during output conversion; no hard cancellation-tail claim.
    pub fn render(&self, check: &dyn Fn() -> Result<()>) -> Result<Raster> {
        check()?;
        let mut pixmap = tiny_skia::Pixmap::new(self.options.width, self.options.height)
            .ok_or_else(|| resource("cannot allocate SVG output pixmap"))?;
        resvg::render(&self.tree, self.transform, &mut pixmap.as_mut());
        check()?;
        let mut rgba = pixmap.take();
        for row in rgba.chunks_mut(self.options.width as usize * 4) {
            check()?;
            for pixel in row.as_chunks_mut::<4>().0 {
                // tiny-skia emits premultiplied sRGB. Native rasters are straight.
                let alpha = u32::from(pixel[3]);
                for sample in &mut pixel[..3] {
                    *sample = (u32::from(*sample) * 255 + alpha / 2)
                        .checked_div(alpha)
                        .unwrap_or(0)
                        .min(255) as u8;
                }
            }
        }
        Ok(Raster {
            width: self.options.width,
            height: self.options.height,
            rgba,
            losses: self.losses.clone(),
            options: self.options,
        })
    }
}

/// All sizes are explicit. Contain fits the SVG viewport onto transparent
/// padding; Stretch applies independent axis scales. DPI resolves physical units.
pub fn prepare(
    bytes: &[u8],
    options: Options,
    fonts: &Fonts,
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    options.admit(pixel_limit)?;
    let source = input::source(bytes, check)?;
    let xml = input::parse(&source, check)?;
    let inventory = input::inspect(&xml, check)?;
    let normalized = input::normalize_hints(&source, &xml)?;
    let normalized_xml = normalized
        .as_deref()
        .map(|text| input::parse(text, check))
        .transpose()?;
    if inventory.has_text && fonts.database.is_empty() {
        return Err(unsupported(
            "SVG text requires an explicitly supplied font collection",
        ));
    }
    let (images, mut losses) = resources::prepare(&inventory.images, pixel_limit, check)?;
    loss(&mut losses, "svg-rasterized");
    let font_events = fonts::Events::default();
    let parsed = usvg::Options {
        dpi: options.dpi,
        font_family: fonts.default_family.clone(),
        fontdb: Arc::clone(&fonts.database),
        font_resolver: font_events.resolver(),
        default_size: usvg::Size::from_wh(options.width as f32, options.height as f32).unwrap(),
        image_href_resolver: images.resolver(),
        ..Default::default()
    };
    check()?;
    let svg = usvg::Tree::from_xmltree(normalized_xml.as_ref().unwrap_or(&xml), &parsed)
        .map_err(|error| malformed(&format!("SVG parsing failed: {error}")))?;
    check()?;
    font_events.finish(&mut losses)?;
    let width = svg.size().width();
    let height = svg.size().height();
    let mut sx = options.width as f32 / width;
    let mut sy = options.height as f32 / height;
    if options.fit == Fit::Contain {
        sx = sx.min(sy);
        sy = sx;
    }
    if !sx.is_finite() || !sy.is_finite() || sx <= 0.0 || sy <= 0.0 {
        return Err(unsupported(
            "SVG viewport cannot be fitted at the selected size",
        ));
    }
    let dx = (options.width as f32 - width * sx) / 2.0;
    let dy = (options.height as f32 - height * sy) / 2.0;
    let transform = tiny_skia::Transform::from_row(sx, 0.0, 0.0, sy, dx, dy);
    tree::admit(&svg, transform, options, check)?;
    Ok(Prepared {
        tree: svg,
        transform,
        options,
        losses,
    })
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn loss(losses: &mut Vec<String>, code: &str) {
    if !losses.iter().any(|known| known == code) {
        losses.push(code.into());
    }
}
