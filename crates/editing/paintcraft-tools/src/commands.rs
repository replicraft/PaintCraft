//! Shared raster command preparation and Paint-style canvas ground.
use crate::{Stroke, Tool};
use paintcraft_core::types::{Result, Rgba, json::Value};
use uuid::Uuid;

const POINTS_PER_OP: usize = 65_536;
const OPS_PER_BATCH: usize = 1024;

fn layer_ref(id: Uuid) -> Value {
    Value::object([("id", Value::str(id.to_string()))])
}
fn point(p: (i64, i64)) -> Value {
    Value::object([("x", Value::Int(p.0)), ("y", Value::Int(p.1))])
}

pub fn shape(
    layer: Uuid,
    tool: Tool,
    from: (i64, i64),
    to: (i64, i64),
    stroke: Option<(Rgba, u32)>,
    fill: Option<Rgba>,
) -> Value {
    let mut pairs = vec![
        ("op", Value::str("raster.shape")),
        ("layer_ref", layer_ref(layer)),
        (
            "kind",
            Value::str(match tool {
                Tool::Line => "line",
                Tool::Rect => "rect",
                _ => "ellipse",
            }),
        ),
        ("from", point(from)),
        ("to", point(to)),
        ("mode", Value::str("source-over")),
    ];
    if let Some((color, size)) = stroke {
        pairs.push((
            "stroke",
            Value::object([
                ("color", Value::str(color.to_hex())),
                ("size", Value::Int(i64::from(size))),
            ]),
        ));
    }
    if let Some(color) = fill {
        pairs.push(("fill", Value::str(color.to_hex())));
    }
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

pub fn bitmap_text(layer: Uuid, at: (i64, i64), text: &str, scale: u32, color: Rgba) -> Value {
    Value::object([
        ("op", Value::str("raster.text")),
        ("layer_ref", layer_ref(layer)),
        ("at", point(at)),
        ("text", Value::str(text)),
        ("scale", Value::Int(i64::from(scale))),
        ("color", Value::str(color.to_hex())),
        ("mode", Value::str("source-over")),
    ])
}

pub fn stroke_operations(stroke: &Stroke, check: &impl Fn() -> Result<()>) -> Result<Vec<Value>> {
    check()?;
    let Stroke {
        layer,
        points,
        color,
        size,
        round,
        point_scale,
        mode,
    } = stroke;
    if points.is_empty() {
        return Ok(Vec::new());
    }
    // Long gestures are split across operations; each starts on the previous
    // chunk's last point so the path stays connected. A translucent colour
    // blends the joint's stamp twice, once per operation.
    let mut ops = Vec::new();
    let mut start = 0;
    while start < points.len() && ops.len() < OPS_PER_BATCH {
        check()?;
        let end = (start + POINTS_PER_OP).min(points.len());
        let mut values = Vec::new();
        values.try_reserve_exact(end - start).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "cannot prepare stroke point values",
            )
        })?;
        for (index, p) in points[start..end].iter().enumerate() {
            if index % 256 == 0 {
                check()?;
            }
            values.push(point(*p));
        }
        ops.push(Value::object([
            ("op", Value::str("raster.stroke_path")),
            ("layer_ref", layer_ref(*layer)),
            ("points", Value::Array(values)),
            ("color", Value::str(color.to_hex())),
            (
                "mode",
                Value::str(match mode {
                    crate::BlendMode::Replace => "replace",
                    crate::BlendMode::SourceOver => "source-over",
                }),
            ),
            ("size", Value::Int(i64::from(*size))),
            ("point_scale", Value::Int(i64::from(*point_scale))),
            (
                "brush",
                Value::str(if *round { "smooth-round" } else { "square" }),
            ),
        ]));
        if end == points.len() {
            break;
        }
        start = end - 1;
    }
    Ok(ops)
}

/// Resize from the top left, retaining off-canvas pixels and extending editable
/// layer bounds. Unchanged dimensions are a no-op, including cropped layers.
pub fn resize_canvas(
    document: &paintcraft_document::Document,
    width: u32,
    height: u32,
) -> Vec<Value> {
    if document.canvas == (width, height) {
        return Vec::new();
    }
    let mut ops = vec![Value::object([
        ("op", Value::str("canvas.resize")),
        ("width", Value::Int(width.into())),
        ("height", Value::Int(height.into())),
        ("anchor", Value::str("top-left")),
        ("outside_content", Value::str("preserve")),
    ])];
    for layer in document.layers_in_order() {
        let w = i64::from(layer.raster.width()).max(i64::from(width) - layer.origin.0);
        let h = i64::from(layer.raster.height()).max(i64::from(height) - layer.origin.1);
        if w > i64::from(layer.raster.width()) || h > i64::from(layer.raster.height()) {
            ops.push(Value::object([
                ("op", Value::str("layer.expand")),
                ("layer_ref", layer_ref(layer.id)),
                ("width", Value::Int(w)),
                ("height", Value::Int(h)),
            ]));
        }
    }
    ops
}

/// Solid ground for Paint-style frontends. Imported images without an opaque
/// default layer sample use white; layered frontends can choose transparency.
pub fn canvas_color(document: &paintcraft_document::Document) -> Rgba {
    document
        .layers_in_order()
        .map(|layer| layer.raster.default_pixel())
        .find(|color| color.a() == 255)
        .unwrap_or(Rgba([255; 4]))
}
