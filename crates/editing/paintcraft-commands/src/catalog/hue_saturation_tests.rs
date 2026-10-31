use super::{tests::packet, *};
use paintcraft_core::types::filters::{Descriptor, HueSaturationRgbV1};
fn descriptor(h: i16, s: i16, l: i16) -> Descriptor {
    Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::new(h, s, l).unwrap())
}
#[test]
fn hsl_generated_add_update_disabled_roundtrip_and_minimal_schema() {
    for [h, s, l] in [
        [0, 0, 0],
        [-180, -100, 100],
        [180, 100, -100],
        [137, 73, -19],
    ] {
        for op in [
            FilterOp::Add {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                alias: "hsl".into(),
                descriptor: descriptor(h, s, l),
                enabled: false,
                to_index: Some(0),
            },
            FilterOp::Update {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                filter: FilterRef::Id(Uuid::from_u128(3)),
                descriptor: descriptor(h, s, l),
            },
        ] {
            assert_eq!(op.required_batch_version(), 6);
            let operation = op.to_value().unwrap();
            assert_eq!(
                Batch::schema_for_operations(std::slice::from_ref(&operation)),
                CATALOG_BATCH_SCHEMA
            );
            let mut raw = packet(CATALOG_BATCH_SCHEMA, None, descriptor(h, s, l).to_value());
            let Value::Object(ref mut fields) = raw else {
                unreachable!()
            };
            fields.insert("operations".into(), Value::Array(vec![operation]));
            let generated = Batch::from_generated_value(raw.clone()).unwrap();
            assert_eq!(generated.version, 6);
            let Op::Filter(actual) = &generated.operations[0] else {
                panic!("filter operation")
            };
            assert_eq!(actual, &op);
            assert_eq!(
                Batch::from_value(&raw).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
    for name in [
        "filter.set_enabled",
        "filter.duplicate",
        "filter.remove",
        "filter.reorder",
    ] {
        assert_eq!(
            Batch::schema_for_operation_fields([(name, Some("hue-saturation-rgb-v1"))]),
            "paintcraft.batch/v2"
        );
    }
}
#[test]
fn hsl_untrusted_requirements_and_old_closed_batches_refuse() {
    let desc = descriptor(0, 0, 0).to_value();
    assert!(
        Batch::from_value(&packet(
            CATALOG_BATCH_SCHEMA,
            Some(&["hue-saturation-rgb-v1"]),
            desc.clone()
        ))
        .is_ok()
    );
    for version in 1..=5 {
        assert_eq!(
            Batch::from_value(&packet(
                &format!("paintcraft.batch/v{version}"),
                None,
                desc.clone()
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
    for required in [
        vec![],
        vec!["invert-rgb-v1"],
        vec!["hue-saturation-rgb-v1", "hue-saturation-rgb-v1"],
        vec!["hue-saturation-rgb-v1", "invert-rgb-v1"],
        vec!["future-hsl-v1"],
    ] {
        assert_eq!(
            Batch::from_value(&packet(CATALOG_BATCH_SCHEMA, Some(&required), desc.clone()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        Batch::catalog_requirements_for_fields([
            ("filter.add", Some("vibrance-rgb-v1")),
            ("filter.update", Some("hue-saturation-rgb-v1")),
            ("filter.add", Some("hue-saturation-rgb-v1"))
        ])
        .unwrap(),
        ["hue-saturation-rgb-v1", "vibrance-rgb-v1"]
    );
}
#[test]
fn hsl_batch_requires_all_three_strict_fields_and_bounds() {
    for field in [
        "hue_degrees",
        "saturation_hundredths",
        "lightness_hundredths",
    ] {
        for bad in [
            Value::Null,
            Value::Bool(true),
            Value::str("0"),
            Value::Int(if field == "hue_degrees" { 181 } else { 101 }),
        ] {
            let Value::Object(mut fields) = descriptor(0, 0, 0).to_value() else {
                unreachable!()
            };
            fields.insert(field.into(), bad);
            assert_eq!(
                Batch::from_value(&packet(
                    CATALOG_BATCH_SCHEMA,
                    Some(&["hue-saturation-rgb-v1"]),
                    Value::Object(fields)
                ))
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
}
