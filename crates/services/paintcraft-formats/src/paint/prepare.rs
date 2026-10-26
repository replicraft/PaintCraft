use super::*;
use std::io::Write;
#[derive(Debug)]
pub struct PreparedLayer {
    width: u32,
    height: u32,
    origin: (i32, i32),
    rgba: Vec<u8>,
}
impl PreparedLayer {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
#[derive(Debug)]
pub struct Prepared {
    canvas: (u32, u32),
    layers: Vec<PreparedLayer>,
    plan: Plan,
    writer: paintcraft_paint::WriteLimits,
}
impl Prepared {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    pub fn layers(&self) -> &[PreparedLayer] {
        &self.layers
    }
    pub fn encode(&self, allowed: &[String], check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        self.plan.require_consent(allowed)?;
        check()?;
        let mut layers = Vec::new();
        layers
            .try_reserve_exact(self.layers.len())
            .map_err(|_| resource("PAINT layer descriptor allowance"))?;
        for layer in &self.layers {
            layers.push(paintcraft_paint::WriteLayer {
                width: layer.width,
                height: layer.height,
                origin: layer.origin,
                item_name: "",
                rgba: &layer.rgba,
                visible: true,
                opacity: 255,
                metadata: &[],
            });
        }
        let doc = paintcraft_paint::WriteDocument {
            width: self.canvas.0,
            height: self.canvas.1,
            canvas_fill: self.plan.options.canvas_fill,
            layers: &layers,
            metadata: &[],
        };
        paintcraft_paint::encode_document(&doc, &self.writer, check)
    }
    pub fn write_to(
        &self,
        allowed: &[String],
        sink: &mut impl Write,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let bytes = self.encode(allowed, check)?;
        for chunk in bytes.chunks(65536) {
            check()?;
            sink.write_all(chunk).map_err(|_| {
                Error::new(ErrorCode::StorageFailed, "Cannot write staged PAINT copy")
            })?;
        }
        check()
    }
}
/// Freeze supported source semantics, geometry, raw pixels and explicit fill/name
/// policy. There is no resampling, outside-canvas clipping or inferred background.
pub fn prepare_document(
    doc: &Document,
    options: Options,
    limits: &Limits,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    let plan = preflight(doc, options, limits, check)?;
    plan.require_consent(allowed)?;
    let mut layers = Vec::new();
    layers
        .try_reserve_exact(doc.roots.len())
        .map_err(|_| resource("PAINT prepared layer allowance"))?;
    for id in &doc.roots {
        check()?;
        let layer = &doc.layers[id];
        let width = layer.raster.width();
        let height = layer.raster.height();
        let length = usize::try_from(u64::from(width) * u64::from(height) * 4)
            .map_err(|_| resource("PAINT prepared layer size overflow"))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| resource("Cannot allocate PAINT prepared pixels"))?;
        while rgba.len() < length {
            check()?;
            rgba.resize(length.min(rgba.len() + 65536), 0);
        }
        for y in 0..height {
            for x in (0..width).step_by(16384) {
                check()?;
                let pixels = (width - x).min(16384);
                let offset = (y as usize * width as usize + x as usize) * 4;
                layer.raster.read_span(
                    i64::from(y),
                    i64::from(x),
                    &mut rgba[offset..offset + pixels as usize * 4],
                );
            }
        }
        layers.push(PreparedLayer {
            width,
            height,
            origin: origin(layer.origin)?,
            rgba,
        });
    }
    check()?;
    Ok(Prepared {
        canvas: doc.canvas,
        layers,
        plan,
        writer: limits.writer,
    })
}
