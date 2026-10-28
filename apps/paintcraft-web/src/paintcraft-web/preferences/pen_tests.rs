use super::*;
use paintcraft_core::types::pen::{PressureCurve, Settings as PenSettings};

#[test]
fn pen_preferences_roundtrip_migrate_and_reject_invalid_explicit_policies() {
    for curve in PressureCurve::ALL {
        for size_pressure in [true, false] {
            let mut prefs = Preferences::default();
            prefs.pen.curve = curve;
            prefs.pen.size_pressure = size_pressure;
            assert_eq!(Preferences::parse(&prefs.bytes().unwrap()).unwrap(), prefs);
        }
    }
    let mut legacy = json::parse(&Preferences::default().bytes().unwrap()).unwrap();
    let Value::Object(object) = &mut legacy else {
        unreachable!()
    };
    object.remove("pen");
    assert_eq!(
        Preferences::parse(&json::canonical(&legacy)).unwrap().pen,
        PenSettings::default()
    );
    for invalid in [
        Value::Null,
        Value::Bool(false),
        Value::object([
            ("size_pressure", Value::str("true")),
            ("curve", Value::str("linear")),
        ]),
    ] {
        let Value::Object(object) = &mut legacy else {
            unreachable!()
        };
        object.insert("pen".into(), invalid);
        assert!(Preferences::parse(&json::canonical(&legacy)).is_err());
    }
}
