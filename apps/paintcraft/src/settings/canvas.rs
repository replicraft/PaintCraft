//! Classic New-canvas preferences; dimensions are document pixels.
use paintcraft_core::types::json::Value;

pub const DEFAULT_SIZE: (u32, u32) = (1000, 486);
pub const SIZE_MAX: u32 = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasDefaults {
    pub configured: (u32, u32),
    pub remember_last: bool,
    pub last: Option<(u32, u32)>,
}

impl Default for CanvasDefaults {
    fn default() -> Self {
        Self {
            configured: DEFAULT_SIZE,
            remember_last: true,
            last: None,
        }
    }
}

impl CanvasDefaults {
    pub fn new_size(&self) -> (u32, u32) {
        if self.remember_last {
            self.last.unwrap_or(self.configured)
        } else {
            self.configured
        }
    }

    pub(super) fn parse(value: &Value) -> Result<Self, String> {
        let configured = dimensions(
            value
                .get("configured")
                .ok_or("missing configured canvas size")?,
        )?;
        let remember_last = value
            .get("remember_last")
            .and_then(Value::as_bool)
            .ok_or("invalid remember_last")?;
        let last = match value.get("last") {
            Some(Value::Null) => None,
            Some(v) => Some(dimensions(v)?),
            None => return Err("missing last canvas size".into()),
        };
        Ok(Self {
            configured,
            remember_last,
            last,
        })
    }

    pub(super) fn value(&self) -> Value {
        Value::object([
            ("configured", size_value(self.configured)),
            ("remember_last", Value::Bool(self.remember_last)),
            ("last", self.last.map_or(Value::Null, size_value)),
        ])
    }
}

fn dimensions(value: &Value) -> Result<(u32, u32), String> {
    let dimension = |key| {
        value
            .get(key)
            .and_then(Value::as_i64)
            .filter(|&n| (1..=i64::from(SIZE_MAX)).contains(&n))
            .map(|n| n as u32)
            .ok_or_else(|| format!("invalid canvas {key}"))
    };
    Ok((dimension("width")?, dimension("height")?))
}

fn size_value((width, height): (u32, u32)) -> Value {
    Value::object([
        ("width", Value::Int(width.into())),
        ("height", Value::Int(height.into())),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_remembered_sizes_remain_independent() {
        let mut prefs = CanvasDefaults::default();
        assert_eq!(prefs.new_size(), (1000, 486));
        prefs.last = Some((1234, 567));
        assert_eq!(prefs.new_size(), (1234, 567));
        prefs.remember_last = false;
        assert_eq!(prefs.new_size(), (1000, 486));
        assert_eq!(CanvasDefaults::parse(&prefs.value()).unwrap(), prefs);
        for invalid in [0, -1, i64::MAX] {
            let mut v = prefs.value();
            let Value::Object(map) = &mut v else {
                unreachable!()
            };
            map.insert(
                "configured".into(),
                Value::object([("width", Value::Int(invalid)), ("height", Value::Int(486))]),
            );
            assert!(CanvasDefaults::parse(&v).is_err());
        }
    }
}
