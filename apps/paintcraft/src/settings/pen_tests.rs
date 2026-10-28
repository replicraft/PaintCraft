use super::*;
use paintcraft_core::types::pen::{PressureCurve, Settings as PenSettings};

#[test]
fn pen_preferences_migrate_roundtrip_and_reject_malformed_explicit_values() {
    let dir =
        std::env::temp_dir().join(format!("paintcraft-pen-settings-{}", uuid::Uuid::new_v4()));
    for curve in PressureCurve::ALL {
        for size_pressure in [true, false] {
            let mut settings = Settings::default();
            settings.pen.curve = curve;
            settings.pen.size_pressure = size_pressure;
            settings.save(&dir).unwrap();
            assert_eq!(Settings::load(&dir), (settings, Loaded::Ok));
        }
    }
    let mut legacy = json::parse(&fs::read(file(&dir)).unwrap()).unwrap();
    let Value::Object(object) = &mut legacy else {
        unreachable!()
    };
    object.remove("pen");
    assert_eq!(
        parse(&json::canonical(&legacy)).unwrap().pen,
        PenSettings::default()
    );
    for schema in ["v1", "v2"] {
        let source =
            format!(r#"{{"schema":"paintcraft.settings/{schema}","experience":"classic"}}"#);
        assert_eq!(
            parse(source.as_bytes()).unwrap().pen,
            PenSettings::default()
        );
    }
    for invalid in [
        Value::Null,
        Value::Bool(false),
        Value::object([
            ("size_pressure", Value::Bool(true)),
            ("curve", Value::str("driver")),
        ]),
    ] {
        let Value::Object(object) = &mut legacy else {
            unreachable!()
        };
        object.insert("pen".into(), invalid);
        assert!(parse(&json::canonical(&legacy)).is_err());
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn desktop_startup_adopts_saved_pen_policy_without_a_native_window() {
    let dir = std::env::temp_dir().join(format!("paintcraft-pen-startup-{}", uuid::Uuid::new_v4()));
    let mut settings = Settings::default();
    settings.pen.size_pressure = false;
    settings.pen.curve = PressureCurve::Firm;
    settings.save(&dir).unwrap();
    let app = crate::App::new(None).with_settings(dir.clone());
    assert_eq!(app.editor.options.pen, settings.pen);
    assert!(!app.editor.is_dirty());
    fs::remove_dir_all(dir).unwrap();
}
