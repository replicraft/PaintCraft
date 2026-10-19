use paintcraft_classic::{TextAlignment, TextOptions};
use paintcraft_core::types::json::Value;

pub(super) fn parse(value: Option<&Value>) -> Result<TextOptions, String> {
    let Some(v) = value else {
        return Ok(TextOptions::default());
    };
    let family = v
        .get("family")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or("invalid text font family")?
        .to_owned();
    let points = v
        .get("points")
        .and_then(Value::as_i64)
        .filter(|n| (1..=512).contains(n))
        .ok_or("invalid text font size")? as u16;
    let boolean = |key| {
        v.get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("invalid text {key}"))
    };
    let alignment = match v.get("alignment").and_then(Value::as_str) {
        Some("left") => TextAlignment::Left,
        Some("center") => TextAlignment::Center,
        Some("right") => TextAlignment::Right,
        _ => return Err("invalid text alignment".into()),
    };
    Ok(TextOptions {
        family,
        points,
        alignment,
        bold: boolean("bold")?,
        italic: boolean("italic")?,
        underline: boolean("underline")?,
        strikeout: boolean("strikeout")?,
        background_fill: boolean("background_fill")?,
        editable_until_save: boolean("editable_until_save")?,
    })
}

pub(super) fn value(text: &TextOptions) -> Value {
    Value::object([
        ("family", Value::str(&text.family)),
        ("points", Value::Int(text.points.into())),
        ("bold", Value::Bool(text.bold)),
        ("italic", Value::Bool(text.italic)),
        ("underline", Value::Bool(text.underline)),
        ("strikeout", Value::Bool(text.strikeout)),
        (
            "alignment",
            Value::str(match text.alignment {
                TextAlignment::Left => "left",
                TextAlignment::Center => "center",
                TextAlignment::Right => "right",
            }),
        ),
        ("background_fill", Value::Bool(text.background_fill)),
        ("editable_until_save", Value::Bool(text.editable_until_save)),
    ])
}
