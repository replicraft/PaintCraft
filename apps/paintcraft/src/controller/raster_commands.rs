//! One command description for speculative raster pixels and final publication.
use super::*;

pub(super) fn shape(
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

pub(super) fn bitmap_text(
    layer: Uuid,
    at: (i64, i64),
    text: &str,
    scale: u32,
    color: Rgba,
) -> Value {
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
