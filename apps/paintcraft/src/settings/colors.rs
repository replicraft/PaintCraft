use paintcraft_classic::{CUSTOM_COLOR_COUNT, CustomColors};
use paintcraft_core::types::{Rgba, json::Value};

pub(super) fn parse(value: Option<&Value>) -> Result<CustomColors, String> {
    let mut colors = [None; CUSTOM_COLOR_COUNT];
    let Some(value) = value else {
        return Ok(colors);
    };
    let values = value
        .as_array()
        .filter(|v| v.len() <= CUSTOM_COLOR_COUNT)
        .ok_or("invalid custom color palette")?;
    for (i, value) in values.iter().enumerate() {
        colors[i] = match value {
            Value::Null => None,
            Value::Str(text) => Some(
                Rgba::parse(text)
                    .filter(|c| c.a() == 255)
                    .ok_or("invalid custom RGB color")?,
            ),
            _ => return Err("invalid custom color slot".into()),
        };
    }
    Ok(colors)
}

pub(super) fn value(colors: &CustomColors) -> Value {
    Value::Array(
        colors
            .iter()
            .map(|color| color.map_or(Value::Null, |c| Value::str(c.to_hex())))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palettes_retain_slot_positions_and_older_settings_default_to_empty() {
        let mut colors = [None; CUSTOM_COLOR_COUNT];
        colors[0] = Some(Rgba([12, 34, 56, 255]));
        colors[27] = Some(Rgba([255, 128, 0, 255]));
        assert_eq!(parse(Some(&value(&colors))).unwrap(), colors);
        assert_eq!(parse(None).unwrap(), [None; CUSTOM_COLOR_COUNT]);
        for bad in [
            Value::Array(vec![Value::Null; 29]),
            Value::Array(vec![Value::str("#12345600")]),
            Value::Bool(true),
        ] {
            assert!(parse(Some(&bad)).is_err());
        }
    }
}
