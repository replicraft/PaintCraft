use super::*;
use std::io::Write;
#[derive(Debug)]
pub struct PreparedLayer {
    name: String,
    rgba: Vec<u8>,
    visible: bool,
    opacity: u8,
    is_background: bool,
}
impl PreparedLayer {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn opacity(&self) -> u8 {
        self.opacity
    }
    pub fn is_background(&self) -> bool {
        self.is_background
    }
}
#[derive(Debug)]
pub struct Prepared {
    canvas: (u32, u32),
    layers: Vec<PreparedLayer>,
    plan: Plan,
    writer: paintcraft_pdn::WriteLimits,
    producer: [u32; 4],
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
            .map_err(|_| resource("PDN layer descriptor allowance"))?;
        for layer in &self.layers {
            layers.push(paintcraft_pdn::RasterLayer {
                width: self.canvas.0,
                height: self.canvas.1,
                name: &layer.name,
                visible: layer.visible,
                opacity: layer.opacity,
                is_background: layer.is_background,
                rgba: &layer.rgba,
                metadata: &[],
            });
        }
        let doc = paintcraft_pdn::RasterDocument {
            width: self.canvas.0,
            height: self.canvas.1,
            layers: &layers,
            saved_with: self.producer,
            metadata: &[],
        };
        paintcraft_pdn::encode_document(&doc, &self.writer, check)
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
                Error::new(ErrorCode::StorageFailed, "Cannot write staged PDN copy")
            })?;
        }
        check()
    }
}
fn producer() -> Result<[u32; 4]> {
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let mut version = [0; 4];
    for value in &mut version[..3] {
        *value = parts
            .next()
            .and_then(|part| part.parse::<u32>().ok())
            .filter(|value| *value <= i32::MAX as u32)
            .ok_or_else(|| {
                unsupported("PaintCraft version is not representable by PDN producer fields")
            })?;
    }
    if parts.next().is_some() {
        return Err(unsupported(
            "PaintCraft version has unsupported PDN producer fields",
        ));
    }
    Ok(version)
}
/// Own exactly these full-canvas layer buffers and policies through encoding.
/// Consent is resolved before preparation allocation, never a gate override.
pub fn prepare_document(
    doc: &Document,
    options: Options,
    limits: &Limits,
    allowed: &[String],
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    let plan = preflight(doc, options, limits, check)?;
    plan.require_consent(allowed)?;
    let producer = producer()?;
    let mut layers = Vec::new();
    layers
        .try_reserve_exact(doc.roots.len())
        .map_err(|_| resource("PDN prepared layer allowance"))?;
    let length = usize::try_from(plan.prepared_pixels / doc.roots.len() as u64 * 4)
        .map_err(|_| resource("PDN prepared layer size overflow"))?;
    for (index, id) in doc.roots.iter().enumerate() {
        check()?;
        let layer = &doc.layers[id];
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(length)
            .map_err(|_| resource("Cannot allocate PDN full-canvas layer"))?;
        while rgba.len() < length {
            check()?;
            rgba.resize(length.min(rgba.len() + 65536), 0);
        }
        if let Some((x, y, width, height, sx, sy)) = overlap(doc, layer) {
            for row in 0..height {
                check()?;
                let offset = ((y + row) as usize * doc.canvas.0 as usize + x as usize) * 4;
                layer.raster.read_span(
                    i64::from(sy + row),
                    i64::from(sx),
                    &mut rgba[offset..offset + width as usize * 4],
                );
            }
        }
        let mut name = String::new();
        name.try_reserve_exact(layer.name.len())
            .map_err(|_| resource("PDN prepared name allowance"))?;
        name.push_str(&layer.name);
        layers.push(PreparedLayer {
            name,
            rgba,
            visible: layer.visible,
            opacity: layer.opacity,
            is_background: options.background == BackgroundRole::Bottom && index == 0,
        });
    }
    let mut writer = limits.writer;
    writer.pixels = writer.pixels.min(limits.prepared_pixels);
    check()?;
    Ok(Prepared {
        canvas: doc.canvas,
        layers,
        plan,
        writer,
        producer,
    })
}
