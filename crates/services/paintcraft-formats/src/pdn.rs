//! Fresh native-document PDN3 preparation. Public writer qualification is separate.
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use paintcraft_document::Document;
mod prepare;
pub use prepare::{Prepared, PreparedLayer, prepare_document};
pub const TITLE_LOSS: &str = "pdn-document-title-not-exported";
pub const BACKGROUND_LOSS: &str = "pdn-background-role-assigned";
pub const GEOMETRY_LOSS: &str = "pdn-layer-geometry-materialized";
pub const CLIP_LOSS: &str = "pdn-outside-canvas-samples-not-retained";
pub const KNOWN_LOSSES: &[&str] = &[TITLE_LOSS, BACKGROUND_LOSS, GEOMETRY_LOSS, CLIP_LOSS];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundRole {
    Ordinary,
    Bottom,
}
impl BackgroundRole {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ordinary => "ordinary",
            Self::Bottom => "bottom-background",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geometry {
    Exact,
    Materialize,
}
impl Geometry {
    pub fn id(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Materialize => "full-canvas-materialize",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub background: BackgroundRole,
    pub geometry: Geometry,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_pixels: u64,
    pub source_resident_bytes: usize,
    pub prepared_pixels: u64,
    pub transient_bytes: usize,
    pub writer: paintcraft_pdn::WriteLimits,
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
    pub clipped_samples: u64,
    pub transient_bytes: usize,
    pub output_capacity: usize,
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
                "Unknown PDN copy loss consent",
            ));
        }
        if let Some(loss) = self.losses.iter().find(|code| !allowed.contains(code)) {
            return Err(unsupported(&format!(
                "PDN copy requires acknowledgment of {loss}"
            )));
        }
        Ok(())
    }
    pub fn receipt(&self) -> Value {
        Value::object([
            ("profile", Value::str("pdn3-normal-full-canvas-v1")),
            ("background_role", Value::str(self.options.background.id())),
            ("geometry", Value::str(self.options.geometry.id())),
            ("source_pixels", Value::Int(self.source_pixels as i64)),
            ("prepared_pixels", Value::Int(self.prepared_pixels as i64)),
            (
                "outside_canvas_samples_not_retained",
                Value::Int(self.clipped_samples as i64),
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
        .ok_or_else(|| resource("PDN preparation size overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource("PDN preparation size overflow"))
}
/// Destination and source-local coordinates of the visible bounded intersection.
fn overlap(
    doc: &Document,
    layer: &paintcraft_document::Layer,
) -> Option<(u32, u32, u32, u32, u32, u32)> {
    let x = i128::from(layer.origin.0);
    let y = i128::from(layer.origin.1);
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + i128::from(layer.raster.width())).min(i128::from(doc.canvas.0));
    let y1 = (y + i128::from(layer.raster.height())).min(i128::from(doc.canvas.1));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
        (x0 - x) as u32,
        (y0 - y) as u32,
    ))
}
/// Exhaustive source inventory and independent source/prepared storage admission.
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
    crate::filter_policy::require_unfiltered_layers(doc)?;
    if doc.roots.is_empty() || doc.roots.len() > 64 || doc.layers.len() != doc.roots.len() {
        return Err(unsupported(
            "PDN copy requires a consistent 1..64 raster-layer stack",
        ));
    }
    let mut names = [0usize; 64];
    let mut source_pixels = 0u64;
    let mut resident = 0usize;
    let mut clipped = 0u64;
    let mut geometry_changed = false;
    let mut metadata = 1024usize;
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id && !doc.roots[..index].contains(id))
            .ok_or_else(|| malformed("PDN source has duplicate or missing layers"))?;
        let paintcraft_document::Layer {
            id: _,
            name: _,
            origin: _,
            visible: _,
            opacity: _,
            filters: _,
            raster: _,
        } = layer;
        let pixels = u64::from(layer.raster.width()) * u64::from(layer.raster.height());
        if pixels == 0 {
            return Err(unsupported("PDN source layer cannot be empty"));
        }
        source_pixels = source_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource("PDN source pixel overflow"))?;
        if source_pixels > limits.source_pixels.min(4_194_304) {
            return Err(resource("PDN source pixel allowance"));
        }
        resident = add(
            resident,
            mul(
                layer.raster.resident_tiles(),
                paintcraft_raster::RasterLayer::TILE_BYTES,
            )?,
        )?;
        if resident > limits.source_resident_bytes {
            return Err(resource("PDN source resident-tile allowance"));
        }
        names[index] = layer.name.len();
        if layer.name.len() > 4096 || layer.name.contains('\0') {
            return Err(unsupported("PDN layer name contains NUL"));
        }
        metadata = add(metadata, layer.name.len())?;
        let changed =
            layer.origin != (0, 0) || (layer.raster.width(), layer.raster.height()) != doc.canvas;
        if changed && options.geometry == Geometry::Exact {
            return Err(unsupported(
                "PDN exact geometry requires full-canvas layers at zero origin",
            ));
        }
        geometry_changed |= changed;
        let visible = overlap(doc, layer).map_or(0, |(_, _, width, height, _, _)| {
            u64::from(width) * u64::from(height)
        });
        clipped = clipped
            .checked_add(pixels - visible)
            .ok_or_else(|| resource("PDN clipped sample count overflow"))?;
    }
    let mut writer = limits.writer;
    writer.pixels = writer.pixels.min(limits.prepared_pixels);
    let backend = paintcraft_pdn::estimate_write(
        doc.canvas.0,
        doc.canvas.1,
        &names[..doc.roots.len()],
        &writer,
    )?;
    metadata = add(
        metadata,
        mul(
            doc.roots.len(),
            std::mem::size_of::<PreparedLayer>()
                + std::mem::size_of::<paintcraft_pdn::RasterLayer<'_>>(),
        )?,
    )?;
    let transient = add(add(resident, backend.transient_bytes)?, metadata)?;
    if transient > limits.transient_bytes {
        return Err(resource("PDN preparation transient allowance"));
    }
    let mut losses = Vec::new();
    if doc.title.is_some() {
        losses.push(TITLE_LOSS.into());
    }
    if options.background == BackgroundRole::Bottom {
        losses.push(BACKGROUND_LOSS.into());
    }
    if geometry_changed {
        losses.push(GEOMETRY_LOSS.into());
    }
    if clipped > 0 {
        losses.push(CLIP_LOSS.into());
    }
    Ok(Plan {
        options,
        source_pixels,
        source_resident_bytes: resident,
        prepared_pixels: (backend.source_bytes / 4) as u64,
        clipped_samples: clipped,
        transient_bytes: transient,
        output_capacity: backend.output_capacity,
        losses,
    })
}
