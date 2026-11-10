use super::*;

#[test]
fn legacy_defaults_and_all_three_explicit_interfaces_round_trip() {
    for frontend in [Frontend::Classic, Frontend::Studio, Frontend::Retro] {
        let prefs = Preferences {
            frontend,
            ..Default::default()
        };
        assert_eq!(Preferences::parse(&prefs.bytes().unwrap()).unwrap(), prefs);
    }
    let mut value = json::parse(&Preferences::default().bytes().unwrap()).unwrap();
    let Value::Object(ref mut fields) = value else {
        unreachable!()
    };
    fields.remove("frontend");
    fields.remove("retro_skin");
    let legacy = Preferences::parse(&json::canonical(&value)).unwrap();
    assert_eq!(legacy.frontend, Frontend::Classic);
    assert_eq!(legacy.retro_skin, paintcraft_retro::Skin::Win95);
    for (field, bad) in [
        ("frontend", Value::str("other")),
        ("frontend", Value::Null),
        ("retro_skin", Value::str("other")),
        ("retro_skin", Value::Null),
    ] {
        let mut bad_value = value.clone();
        let Value::Object(ref mut fields) = bad_value else {
            unreachable!()
        };
        fields.insert(field.into(), bad);
        assert!(Preferences::parse(&json::canonical(&bad_value)).is_err());
    }
}
