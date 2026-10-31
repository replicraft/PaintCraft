//! Spatial descriptors extend the packet grammar without widening older profiles.
use super::*;
use paintcraft_core::types::filters::BoxBlurV1;
fn packet(version: u8, operation: Value) -> Result<Batch> {
    Batch::from_value(&Value::object([
        ("schema", Value::str(format!("paintcraft.batch/v{version}"))),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                ("state_id", Value::str(Uuid::from_u128(1).to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ]))
}
fn add(descriptor: Value) -> Value {
    Value::object([
        ("op", Value::str("filter.add")),
        (
            "layer_ref",
            Value::object([("id", Value::str(Uuid::from_u128(2).to_string()))]),
        ),
        ("ref", Value::str("blur")),
        ("enabled", Value::Bool(false)),
        ("descriptor", descriptor),
    ])
}
#[test]
fn box_add_update_alone_select_v5_and_old_selectors_stay_exact() {
    let descriptor = Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap());
    for op in [
        FilterOp::Add {
            layer: LayerRef::Id(Uuid::from_u128(2)),
            alias: "blur".into(),
            descriptor,
            enabled: false,
            to_index: None,
        },
        FilterOp::Update {
            layer: LayerRef::Id(Uuid::from_u128(2)),
            filter: FilterRef::Id(Uuid::from_u128(3)),
            descriptor,
        },
    ] {
        assert_eq!(op.required_batch_version(), 5);
        assert_eq!(
            Batch::schema_for_operations(&[op.to_value().unwrap()]),
            "paintcraft.batch/v5"
        );
    }
    for (name, kind, expected) in [
        ("filter.add", Some("invert-rgb-v1"), "paintcraft.batch/v2"),
        (
            "filter.update",
            Some("brightness-contrast-v1"),
            "paintcraft.batch/v2",
        ),
        (
            "filter.add",
            Some("black-white-rgb-v1"),
            "paintcraft.batch/v4",
        ),
        (
            "filter.update",
            Some("threshold-luma-v1"),
            "paintcraft.batch/v4",
        ),
        (
            "filter.add",
            Some("posterize-rgb-v1"),
            "paintcraft.batch/v4",
        ),
        (
            "filter.update",
            Some("solarize-rgb-v1"),
            "paintcraft.batch/v4",
        ),
        ("filter.mask.from_selection", None, "paintcraft.batch/v3"),
        (
            "filter.mask.remove",
            Some("box-blur-v1"),
            "paintcraft.batch/v3",
        ),
        ("filter.set_enabled", None, "paintcraft.batch/v2"),
        ("layer.rename", None, "paintcraft.batch/v2"),
    ] {
        assert_eq!(Batch::schema_for_operation_fields([(name, kind)]), expected);
    }
    assert_eq!(
        Batch::schema_for_operation_fields([
            ("filter.mask.remove", None),
            ("filter.add", Some("box-blur-v1"))
        ]),
        "paintcraft.batch/v5"
    );
}
#[test]
fn strict_box_packets_roundtrip_radius_zero_disabled_and_old_profiles_refuse() {
    for radius in [0, 1, 2, 32] {
        let descriptor = Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap());
        for op in [
            FilterOp::Add {
                layer: LayerRef::Alias("ink".into()),
                alias: "blur".into(),
                descriptor,
                enabled: false,
                to_index: Some(0),
            },
            FilterOp::Update {
                layer: LayerRef::Id(Uuid::from_u128(2)),
                filter: FilterRef::Alias("blur".into()),
                descriptor,
            },
        ] {
            let value = op.to_value().unwrap();
            assert_eq!(
                packet(5, value.clone()).unwrap().operations,
                [Op::Filter(op)]
            );
            for old in 1..=4 {
                assert_eq!(
                    packet(old, value.clone()).unwrap_err().code,
                    ErrorCode::InputSchema
                );
            }
        }
    }
    let legacy = add(Descriptor::InvertRgbV1.to_value());
    assert!(packet(5, legacy.clone()).is_ok());
    assert_eq!(
        Batch::schema_for_operations(&[legacy]),
        "paintcraft.batch/v2"
    );
}
#[test]
fn box_v5_does_not_authorize_invalid_or_extra_parameters() {
    for descriptor in [
        Value::object([("kind", Value::str("box-blur-v1"))]),
        Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::Int(-1)),
        ]),
        Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::Int(33)),
        ]),
        Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::Bool(true)),
        ]),
        Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::str("2")),
        ]),
        Value::object([
            ("kind", Value::str("box-blur-v1")),
            ("radius", Value::Int(2)),
            ("clamp", Value::Bool(true)),
        ]),
    ] {
        assert_eq!(
            packet(5, add(descriptor)).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
}
