//! Fresh PAINT preparation; original Windows Paint/public qualification is separate.
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use paintcraft_document::Document;
mod prepare;
pub use prepare::{Prepared, PreparedLayer, prepare_document};
pub const TITLE_LOSS: &str = "paint-document-title-not-exported";
pub const NAMES_LOSS: &str = "paint-layer-names-not-retained";
pub const FILL_LOSS: &str = "paint-canvas-fill-assigned";
pub const KNOWN_LOSSES: &[&str] = &[TITLE_LOSS, NAMES_LOSS, FILL_LOSS];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerNames {
    RequireEmpty,
    Omit,
}
impl LayerNames {
    pub fn id(self) -> &'static str {
        match self {
            Self::RequireEmpty => "require-empty",
            Self::Omit => "omit",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub canvas_fill: [u16; 4],
    pub layer_names: LayerNames,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_pixels: u64,
    pub source_resident_bytes: usize,
    pub prepared_pixels: u64,
    pub transient_bytes: usize,
    pub writer: paintcraft_paint::WriteLimits,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_pixels: 4_194_304,
            source_resident_bytes: 64 << 20,
            prepared_pixels: 4_194_304,
            transient_bytes: 512 << 20,
            writer: Default::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub options: Options,
    pub source_pixels: u64,
    pub source_resident_bytes: usize,
    pub prepared_pixels: u64,
    pub transient_bytes: usize,
    pub output_capacity: usize,
    pub omitted_names: usize,
    pub losses: Vec<String>,
}
impl Plan {
    pub fn require_consent(&self, allowed: &[String]) -> Result<()> {
        if allowed
            .iter()
            .any(|code| !KNOWN_LOSSES.contains(&code.as_str()))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown PAINT copy loss consent",
            ));
        }
        if let Some(loss) = self.losses.iter().find(|code| !allowed.contains(code)) {
            return Err(unsupported(&format!(
                "PAINT copy requires acknowledgment of {loss}"
            )));
        }
        Ok(())
    }
    pub fn receipt(&self) -> Value {
        Value::object([
            ("profile", Value::str("paint-iovl-unci-rgba8-v1")),
            (
                "canvas_fill16",
                Value::Array(
                    self.options
                        .canvas_fill
                        .iter()
                        .map(|value| Value::Int(i64::from(*value)))
                        .collect(),
                ),
            ),
            ("layer_names", Value::str(self.options.layer_names.id())),
            (
                "omitted_nonempty_names",
                Value::Int(self.omitted_names as i64),
            ),
            ("source_pixels", Value::Int(self.source_pixels as i64)),
            ("prepared_pixels", Value::Int(self.prepared_pixels as i64)),
            (
                "geometry",
                Value::str("retained-layer-extents-and-signed32-origins"),
            ),
        ])
    }
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource("PAINT preparation size overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource("PAINT preparation size overflow"))
}
fn origin(value: (i64, i64)) -> Result<(i32, i32)> {
    Ok((
        i32::try_from(value.0).map_err(|_| unsupported("PAINT source origin exceeds signed32"))?,
        i32::try_from(value.1).map_err(|_| unsupported("PAINT source origin exceeds signed32"))?,
    ))
}
/// Inventory every source semantic before owning pixels. Native composition
/// starts transparent black; an explicit other fill adds target background behavior.
pub fn preflight(
    doc: &Document,
    options: Options,
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Plan> {
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
    if doc.roots.is_empty() || doc.roots.len() > 64 || doc.layers.len() != doc.roots.len() {
        return Err(unsupported(
            "PAINT copy requires a consistent 1..64 raster-layer stack",
        ));
    }
    let mut layouts = [paintcraft_paint::WriteLayout::default(); 64];
    let mut source_pixels = 0u64;
    let mut resident = 0usize;
    let mut names = 0usize;
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id && !doc.roots[..index].contains(id))
            .ok_or_else(|| malformed("PAINT source has duplicate or missing layers"))?;
        let paintcraft_document::Layer {
            id: _,
            name: _,
            origin: _,
            visible: _,
            opacity: _,
            raster: _,
        } = layer;
        if !layer.visible || layer.opacity != 255 {
            return Err(unsupported(
                "PAINT hidden-layer or per-layer opacity semantics are unqualified",
            ));
        }
        if !layer.name.is_empty() {
            if options.layer_names == LayerNames::RequireEmpty {
                return Err(unsupported(
                    "PAINT UI layer-name semantics are unqualified; explicit omission required",
                ));
            }
            names += 1;
        }
        let pixels = u64::from(layer.raster.width()) * u64::from(layer.raster.height());
        if pixels == 0 {
            return Err(unsupported("PAINT source layer cannot be empty"));
        }
        source_pixels = source_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource("PAINT source pixel overflow"))?;
        if source_pixels > limits.source_pixels.min(4_194_304) {
            return Err(resource("PAINT source pixel allowance"));
        }
        if source_pixels > limits.prepared_pixels.min(4_194_304) {
            return Err(resource("PAINT prepared pixel allowance"));
        }
        resident = add(
            resident,
            mul(
                layer.raster.resident_tiles(),
                paintcraft_raster::RasterLayer::TILE_BYTES,
            )?,
        )?;
        if resident > limits.source_resident_bytes {
            return Err(resource("PAINT source resident-tile allowance"));
        }
        layouts[index] = paintcraft_paint::WriteLayout {
            width: layer.raster.width(),
            height: layer.raster.height(),
            origin: origin(layer.origin)?,
            item_name_bytes: 0,
        };
    }
    let backend = paintcraft_paint::estimate_write(
        doc.canvas.0,
        doc.canvas.1,
        &layouts[..doc.roots.len()],
        &limits.writer,
    )?;
    let metadata = add(
        1024,
        mul(
            doc.roots.len(),
            std::mem::size_of::<PreparedLayer>()
                + std::mem::size_of::<paintcraft_paint::WriteLayer<'_>>(),
        )?,
    )?;
    let transient = add(add(resident, backend.transient_bytes)?, metadata)?;
    if transient > limits.transient_bytes {
        return Err(resource("PAINT preparation transient allowance"));
    }
    let mut losses = Vec::new();
    if doc.title.is_some() {
        losses.push(TITLE_LOSS.into());
    }
    if names > 0 {
        losses.push(NAMES_LOSS.into());
    }
    if options.canvas_fill != [0; 4] {
        losses.push(FILL_LOSS.into());
    }
    Ok(Plan {
        options,
        source_pixels,
        source_resident_bytes: resident,
        prepared_pixels: source_pixels,
        transient_bytes: transient,
        output_capacity: backend.output_capacity,
        omitted_names: names,
        losses,
    })
}
