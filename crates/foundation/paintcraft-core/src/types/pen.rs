//! Captured PaintCraft pen policies, independent of device capability claims.
use super::{Error, ErrorCode, Result, json::Value};

/// Finite, deterministic pressure response. These are PaintCraft policies,
/// rather than claims of matching a particular tablet driver or original app.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PressureCurve {
    #[default]
    Linear,
    Soft,
    Firm,
}
impl PressureCurve {
    pub const ALL: [Self; 3] = [Self::Linear, Self::Soft, Self::Firm];
    pub const fn id(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Soft => "soft",
            Self::Firm => "firm",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|curve| curve.id() == id)
    }
    /// Map normalized Q16, rounding to nearest with ties upward. No platform
    /// floating-point power function participates in the captured response.
    pub fn map(self, pressure: u16) -> u16 {
        let p = u64::from(pressure);
        match self {
            Self::Linear => pressure,
            Self::Firm => ((p * p + 32767) / 65535) as u16,
            Self::Soft => {
                let square = p * 65535;
                let floor = square.isqrt();
                let upper = floor + 1;
                if upper * upper - square <= square - floor * floor {
                    upper as u16
                } else {
                    floor as u16
                }
            }
        }
    }
}

/// Preferences are copied into the stroke at Down. Changing these settings
/// later must not modify a captured preview, replay, or history item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub size_pressure: bool,
    pub curve: PressureCurve,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            size_pressure: true,
            curve: PressureCurve::Linear,
        }
    }
}
impl Settings {
    /// Missing pressure remains missing. It is not converted into reported zero
    /// or evidence that the physical device supports a pressure axis.
    pub fn width(self, pressure: Option<u16>) -> Option<u16> {
        if self.size_pressure {
            pressure.map(|p| self.curve.map(p))
        } else {
            None
        }
    }
    pub fn to_value(self) -> Value {
        Value::object([
            ("size_pressure", Value::Bool(self.size_pressure)),
            ("curve", Value::str(self.curve.id())),
        ])
    }
    /// Hosts may default an absent preferences field, but an explicit malformed
    /// policy is rejected rather than silently assigned another response.
    pub fn from_value(value: &Value) -> Result<Self> {
        let invalid = || Error::new(ErrorCode::InputSchema, "invalid pen settings");
        let object = value.as_object().ok_or_else(invalid)?;
        if object.len() != 2 {
            return Err(invalid());
        }
        Ok(Self {
            size_pressure: object
                .get("size_pressure")
                .and_then(Value::as_bool)
                .ok_or_else(invalid)?,
            curve: object
                .get("curve")
                .and_then(Value::as_str)
                .and_then(PressureCurve::from_id)
                .ok_or_else(invalid)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_q16_response_matches_an_independent_nearest_formula() {
        let mut previous = [0; 3];
        for p in 0..=u16::MAX {
            for (index, curve) in PressureCurve::ALL.into_iter().enumerate() {
                let expected = match curve {
                    PressureCurve::Linear => f64::from(p),
                    PressureCurve::Firm => f64::from(p).powi(2) / 65535.0,
                    PressureCurve::Soft => (f64::from(p) * 65535.0).sqrt(),
                }
                .round() as u16;
                let actual = curve.map(p);
                assert_eq!(actual, expected, "{} at {p}", curve.id());
                assert!(actual >= previous[index]);
                previous[index] = actual;
            }
            assert!(PressureCurve::Firm.map(p) <= p);
            assert!(PressureCurve::Soft.map(p) >= p);
        }
        assert_eq!(previous, [65535; 3]);
    }

    #[test]
    fn settings_roundtrip_strictly_and_missing_is_not_reported_zero() {
        for curve in PressureCurve::ALL {
            for size_pressure in [true, false] {
                let settings = Settings {
                    size_pressure,
                    curve,
                };
                assert_eq!(
                    Settings::from_value(&settings.to_value()).unwrap(),
                    settings
                );
                assert_eq!(settings.width(None), None);
                assert_eq!(settings.width(Some(0)), size_pressure.then_some(0));
            }
        }
        assert!(Settings::from_value(&Value::object([])).is_err());
        for curve in [Value::str("driver"), Value::Int(0), Value::Null] {
            assert!(
                Settings::from_value(&Value::object([
                    ("size_pressure", Value::Bool(true)),
                    ("curve", curve)
                ]))
                .is_err()
            );
        }
    }
}
