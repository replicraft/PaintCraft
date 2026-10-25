//! Observed MIF1/iovl/unci item graph, not a generic HEIF decoder.
//! Layer names/visibility/opacity remain explicitly unverified source semantics.
use super::*;
mod graph;
mod pixels;
use graph::{Graph, Reader};

#[derive(Clone, Copy, Debug)]
pub struct DecodeLimits {
    pub container: Limits,
    pub items: usize,
    pub layers: usize,
    pub dimension: u32,
    /// Aggregate decoded layer pixels, separately from canvas dimensions.
    pub pixels: u64,
    pub decoded_bytes: usize,
}
impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            container: Limits::default(),
            items: 4096,
            layers: 1024,
            dimension: 65_536,
            pixels: 268_435_456,
            decoded_bytes: 1 << 30,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    /// Item names are returned literally; no claim that they are UI layer names.
    /// Layer visibility/opacity/name extensions have not been qualified.
    LayerPropertiesUnverified,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLayer<'a> {
    pub item_id: u32,
    pub item_name: &'a str,
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    /// Exact straight RGBA8 sRGB, including RGB beneath zero alpha.
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded<'a> {
    pub width: u32,
    pub height: u32,
    /// Preserve all 16 bits. Narrowing this document property is a conversion.
    pub canvas_fill: [u16; 4],
    /// Bottom to top, as declared by the primary overlay's dimg references.
    pub layers: Vec<DecodedLayer<'a>>,
    pub diagnostics: [Diagnostic; 1],
}

pub fn decode_layers<'a>(
    bytes: &'a [u8],
    limits: &DecodeLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded<'a>> {
    check()?;
    let graph = Graph::parse(bytes, limits, check)?;
    let primary = graph.item(graph.primary)?;
    if primary.kind != *b"iovl" {
        return Err(unsupported("PAINT primary item must be iovl"));
    }
    let mut overlay = Reader::new(graph.data(primary)?);
    if overlay.u8()? != 0 {
        return Err(unsupported("Unsupported PAINT overlay version"));
    }
    let flags = overlay.u8()?;
    if flags > 1 {
        return Err(unsupported("Unsupported PAINT overlay flags"));
    }
    let fill = [
        overlay.u16()?,
        overlay.u16()?,
        overlay.u16()?,
        overlay.u16()?,
    ];
    let (width, height) = if flags == 0 {
        (u32::from(overlay.u16()?), u32::from(overlay.u16()?))
    } else {
        (overlay.u32()?, overlay.u32()?)
    };
    dimensions(width, height, limits)?;
    if graph.order.is_empty() || graph.order.len() > limits.layers {
        return Err(resource("PAINT layer count exceeds limits"));
    }
    let mut offsets = graph.vector(graph.order.len())?;
    for _ in &graph.order {
        check()?;
        offsets.push(if flags == 0 {
            (i32::from(overlay.i16()?), i32::from(overlay.i16()?))
        } else {
            (overlay.i32()?, overlay.i32()?)
        });
    }
    overlay.end()?;
    pixels::validate_overlay(&graph, primary, (width, height))?;
    let mut plans = graph.vector(graph.order.len())?;
    let mut total = 0u64;
    for (&id, &origin) in graph.order.iter().zip(&offsets) {
        check()?;
        let item = graph.item(id)?;
        let plan = pixels::plan(&graph, item, origin, limits)?;
        total = total
            .checked_add(u64::from(plan.width) * u64::from(plan.height))
            .ok_or_else(|| resource("PAINT pixel count overflow"))?;
        if total > limits.pixels
            || total
                .checked_mul(4)
                .is_none_or(|n| n > limits.decoded_bytes as u64)
        {
            return Err(resource("PAINT aggregate decoded layer budget exceeded"));
        }
        plans.push(plan);
    }
    let mut layers = graph.vector(plans.len())?;
    for plan in plans {
        check()?;
        let rgba = pixels::inflate(plan.compressed, plan.width, plan.height, check)?;
        let rgba = pixels::swizzle(rgba, plan.order, check)?;
        layers.push(DecodedLayer {
            item_id: plan.id,
            item_name: plan.name,
            width: plan.width,
            height: plan.height,
            origin: plan.origin,
            rgba,
        });
    }
    check()?;
    Ok(Decoded {
        width,
        height,
        canvas_fill: fill,
        layers,
        diagnostics: [Diagnostic::LayerPropertiesUnverified],
    })
}
fn dimensions(width: u32, height: u32, limits: &DecodeLimits) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(malformed("PAINT dimensions must be positive"));
    }
    if width > limits.dimension
        || height > limits.dimension
        || u64::from(width) * u64::from(height) > limits.pixels
    {
        return Err(resource("PAINT dimensions exceed limits"));
    }
    Ok(())
}
