//! Optional Studio panel preferences in the existing settings schema.

use paintcraft_core::types::json::Value;
use paintcraft_studio::layout::Layout;

const NAMES: [&str; 4] = ["tools", "colors", "layers", "history"];

pub(super) fn parse(value: Option<&Value>) -> Result<Layout, String> {
    let mut layout = Layout::default();
    let Some(value) = value else {
        return Ok(layout);
    };
    let Value::Object(_) = value else {
        return Err("invalid Studio layout".into());
    };
    for (index, name) in NAMES.iter().enumerate() {
        let Some(value) = value.get(name) else {
            continue;
        };
        let visible = value
            .get("visible")
            .and_then(Value::as_bool)
            .ok_or("invalid Studio panel visibility")?;
        let position = match value.get("position") {
            None | Some(Value::Null) => None,
            Some(Value::Array(coordinates)) if coordinates.len() == 2 => {
                let mut position = [0; 2];
                for (output, input) in position.iter_mut().zip(coordinates) {
                    let coordinate = input.as_i64().ok_or("invalid Studio panel position")?;
                    if !(-32768..=32768).contains(&coordinate) {
                        return Err("Studio panel position is out of range".into());
                    }
                    *output = coordinate as i32;
                }
                Some(position)
            }
            _ => return Err("invalid Studio panel position".into()),
        };
        layout.panels[index].visible = visible;
        layout.panels[index].position = position;
    }
    Ok(layout)
}

pub(super) fn value(layout: &Layout) -> Value {
    Value::object(std::array::from_fn::<_, 4, _>(|index| {
        let name = NAMES[index];
        let panel = &layout.panels[index];
        (
            name,
            Value::object([
                ("visible", Value::Bool(panel.visible)),
                (
                    "position",
                    panel.position.map_or(Value::Null, |position| {
                        Value::Array(
                            position
                                .into_iter()
                                .map(|coordinate| Value::Int(i64::from(coordinate)))
                                .collect(),
                        )
                    }),
                ),
            ]),
        )
    }))
}

#[cfg(test)]
mod tests;
