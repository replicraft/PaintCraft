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

pub fn contour(shape: &crate::Contour) -> Value {
    let mut pairs = vec![
        ("op", Value::str("raster.contour")),
        ("layer_ref", layer_ref(shape.layer)),
        (
            "points",
            Value::Array(shape.points.iter().map(|&p| point(p)).collect()),
        ),
        ("closed", Value::Bool(shape.closed)),
        ("mode", Value::str("source-over")),
    ];
    if let Some((color, size)) = shape.stroke {
        pairs.push((
            "stroke",
            Value::object([
                ("color", Value::str(color.to_hex())),
                ("size", Value::Int(size.into())),
            ]),
        ));
    }
    if let Some(color) = shape.fill {
        pairs.push(("fill", Value::str(color.to_hex())));
    }
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
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
                Tool::Preset(preset) => preset.command_name(),
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
        media,
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
                Value::str(if *round { media.id() } else { "square" }),
            ),
        ]));
        if end == points.len() {
            break;
        }
        start = end - 1;
    }
    Ok(ops)
}

/// Core canvas placement: move origins and selection without scaling or changing
/// layer-local bounds. Additional canvas area remains uncovered by those layers.
pub fn resize_canvas_anchored(
    document: &paintcraft_document::Document,
    width: u32,
    height: u32,
    anchor: paintcraft_core::types::canvas::Anchor,
) -> Vec<Value> {
    if document.canvas == (width, height) {
        return Vec::new();
    }
    vec![Value::object([
        ("op", Value::str("canvas.resize")),
        ("width", Value::Int(width.into())),
        ("height", Value::Int(height.into())),
        ("anchor", Value::str(anchor.id())),
        ("outside_content", Value::str("preserve")),
    ])]
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

pub fn font_text(
    mask: &paintcraft_core::types::text::TextMask,
    canvas: paintcraft_core::types::Rect,
    layer: Uuid,
    style: &paintcraft_core::types::text::TextOptions,
    color: Rgba,
    background: Rgba,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<Value>> {
    let mut ops = Vec::new();
    if style.background_fill
        && let Some(rect) = mask.bounds.and_then(|rect| rect.intersect(&canvas))
    {
        ops.push(Value::object([
            ("op", Value::str("raster.fill_rect")),
            ("layer_ref", layer_ref(layer)),
            ("rect", rect_value(rect)),
            ("color", Value::str(background.to_hex())),
            ("mode", Value::str("source-over")),
        ]));
    }
    check()?;
    if !mask.spans.is_empty() {
        ops.push(Value::object([
            ("op", Value::str("raster.mask_spans")),
            ("layer_ref", layer_ref(layer)),
            ("color", Value::str(color.to_hex())),
            (
                "spans",
                Value::Array(
                    mask.spans
                        .iter()
                        .enumerate()
                        .map(|(index, s)| {
                            if index % 256 == 0 {
                                check()?;
                            }
                            Ok(Value::Array(vec![
                                Value::Int(s.x0),
                                Value::Int(s.y),
                                Value::Int(s.x1),
                                Value::Int(i64::from(s.coverage)),
                            ]))
                        })
                        .collect::<Result<Vec<_>>>()?,
                ),
            ),
        ]));
    }
    Ok(ops)
}

fn rect_value(r: paintcraft_core::types::Rect) -> Value {
    Value::object([
        ("x", Value::Int(r.x0)),
        ("y", Value::Int(r.y0)),
        ("width", Value::Int(r.x1 - r.x0)),
        ("height", Value::Int(r.y1 - r.y0)),
    ])
}

pub fn crop_to_selection(
    document: &paintcraft_document::Document,
    selection: paintcraft_raster::Selection,
) -> Result<Value> {
    let area = match selection {
        paintcraft_raster::Selection::Rect(rect) => rect.intersect(&document.canvas_rect()),
        paintcraft_raster::Selection::Mask(_) => return Err(mask_selection_unsupported()),
        _ => None,
    }
    .ok_or_else(|| {
        paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::InputSchema,
            "Select an area of the canvas before cropping",
        )
    })?;
    Ok(Value::object([
        ("op", Value::str("canvas.crop")),
        ("rect", rect_value(area)),
    ]))
}

/// Whole canvas when unrestricted; otherwise the active layer's selected pixels.
pub fn transform_image(
    selection: paintcraft_raster::Selection,
    target: Option<Uuid>,
    transform: paintcraft_core::types::orthogonal::OrthogonalTransform,
    clear: Rgba,
) -> Result<Value> {
    if matches!(selection, paintcraft_raster::Selection::Mask(_)) {
        return Err(mask_selection_unsupported());
    }
    if selection == paintcraft_raster::Selection::Inactive {
        return Ok(Value::object([
            ("op", Value::str("canvas.transform")),
            ("transform", Value::str(transform.name())),
        ]));
    }
    let layer = target.ok_or_else(|| {
        paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::InputSchema,
            "Choose a layer to transform selected pixels",
        )
    })?;
    Ok(Value::object([
        ("op", Value::str("raster.transform_selection")),
        ("layer_ref", layer_ref(layer)),
        ("transform", Value::str(transform.name())),
        ("clear", Value::str(clear.to_hex())),
    ]))
}

/// Erase only an active nonempty selection, using the frontend's captured ground.
/// The fill command retains selection clipping and publication/undo semantics.
pub fn erase_selection(
    selection: paintcraft_raster::Selection,
    target: Option<Uuid>,
    color: Rgba,
) -> Result<Vec<Value>> {
    if matches!(selection, paintcraft_raster::Selection::Mask(_)) {
        return Err(mask_selection_unsupported());
    }
    let paintcraft_raster::Selection::Rect(rect) = selection else {
        return Ok(Vec::new());
    };
    let layer = target.ok_or_else(|| {
        paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::ReferenceUnknownId,
            "Select a layer to erase selected pixels",
        )
    })?;
    Ok(vec![Value::object([
        ("op", Value::str("raster.fill_rect")),
        ("layer_ref", layer_ref(layer)),
        ("rect", rect_value(rect)),
        ("color", Value::str(color.to_hex())),
        ("mode", Value::str("replace")),
    ])])
}

fn mask_selection_unsupported() -> paintcraft_core::types::Error {
    paintcraft_core::types::Error::new(
        paintcraft_core::types::ErrorCode::InputSchema,
        "This operation does not yet support selection masks",
    )
}
