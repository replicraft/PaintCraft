use super::{tests::packet, *};
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
fn descriptor(amount: i16, seed: u16, mono: bool, d: NoiseDistribution) -> Descriptor {
    Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, seed, mono, d).unwrap())
}
#[test]
fn noise_generated_add_update_all_controls_keep_complete_catalog_intent() {
    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        for (a, s, m) in [(0, 0, true), (10, 1, true), (100, 65535, false)] {
            let desc = descriptor(a, s, m, d);
            for op in [
                FilterOp::Add {
                    layer: LayerRef::Id(Uuid::from_u128(2)),
                    alias: "noise".into(),
                    descriptor: desc,
                    enabled: false,
                    to_index: Some(0),
                },
                FilterOp::Update {
                    layer: LayerRef::Id(Uuid::from_u128(2)),
                    filter: FilterRef::Id(Uuid::from_u128(3)),
                    descriptor: desc,
                },
            ] {
                assert_eq!(op.required_batch_version(), 6);
                let operation = op.to_value().unwrap();
                assert_eq!(
                    Batch::schema_for_operations(std::slice::from_ref(&operation)),
                    CATALOG_BATCH_SCHEMA
                );
                let mut raw = packet(CATALOG_BATCH_SCHEMA, None, desc.to_value());
                let Value::Object(ref mut fields) = raw else {
                    unreachable!()
                };
                fields.insert("operations".into(), Value::Array(vec![operation]));
                assert_eq!(
                    Batch::from_value(&raw).unwrap_err().code,
                    ErrorCode::InputSchema
                );
                let parsed = Batch::from_generated_value(raw).unwrap();
                assert_eq!(parsed.version, 6);
                let Op::Filter(actual) = &parsed.operations[0] else {
                    panic!("filter operation")
                };
                assert_eq!(actual, &op);
            }
        }
    }
}
#[test]
fn noise_required_inventory_is_exact_and_every_numbered_legacy_batch_stays_closed() {
    let desc = descriptor(0, 65535, false, NoiseDistribution::Gaussian).to_value();
    assert!(
        Batch::from_value(&packet(
            CATALOG_BATCH_SCHEMA,
            Some(&["add-noise-rgb-v1"]),
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
        vec!["add-noise-rgb-v1", "add-noise-rgb-v1"],
        vec!["add-noise-rgb-v1", "vibrance-rgb-v1"],
        vec!["future-noise-v1"],
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
            ("filter.update", Some("add-noise-rgb-v1")),
            ("filter.add", Some("add-noise-rgb-v1")),
            ("filter.add", Some("hue-saturation-rgb-v1"))
        ])
        .unwrap(),
        ["add-noise-rgb-v1", "hue-saturation-rgb-v1"]
    );
    for name in [
        "filter.set_enabled",
        "filter.duplicate",
        "filter.remove",
        "filter.reorder",
    ] {
        assert_eq!(
            Batch::schema_for_operation_fields([(name, Some("add-noise-rgb-v1"))]),
            "paintcraft.batch/v2"
        );
    }
}
#[test]
fn noise_malformed_seed_flags_and_distribution_refuse_at_public_batch_boundary() {
    for (field, bad) in [
        ("amount_hundredths", Value::Int(101)),
        ("seed", Value::Int(65536)),
        ("seed", Value::Bool(false)),
        ("monochrome", Value::Int(0)),
        ("distribution", Value::str("normal")),
    ] {
        let Value::Object(mut fields) =
            descriptor(10, 65535, true, NoiseDistribution::Uniform).to_value()
        else {
            unreachable!()
        };
        fields.insert(field.into(), bad);
        assert_eq!(
            Batch::from_value(&packet(
                CATALOG_BATCH_SCHEMA,
                Some(&["add-noise-rgb-v1"]),
                Value::Object(fields)
            ))
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
}
