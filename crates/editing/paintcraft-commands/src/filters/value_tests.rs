use super::*;
use crate::{Batch, ErrorCode, Op};
use paintcraft_core::types::filters::{BrightnessContrastV1, Descriptor};
use uuid::Uuid;

fn batch(operation: Value, version: u8) -> Result<Batch> {
    Batch::from_value(&Value::object([
        ("schema", Value::str(format!("paintcraft.batch/v{version}"))),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str("paintcraft.core/default-v1")),
        (
            "base",
            Value::object([
                (
                    "state_id",
                    Value::str("00000000-0000-4000-8000-000000000001"),
                ),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ]))
}
fn roundtrip(op: FilterOp) -> Value {
    let value = op.to_value().unwrap();
    assert_eq!(super::super::parse(&value).unwrap(), Op::Filter(op.clone()));
    assert_eq!(
        batch(value.clone(), 2).unwrap().operations,
        [Op::Filter(op)]
    );
    assert_eq!(
        batch(value.clone(), 1).unwrap_err().code,
        ErrorCode::InputSchema
    );
    value
}

#[test]
fn all_six_operations_roundtrip_both_reference_unions_and_disabled_states() {
    for layer in [
        LayerRef::Alias("ink".into()),
        LayerRef::Id(Uuid::from_u128(1)),
    ] {
        for filter in [
            FilterRef::Alias("effect".into()),
            FilterRef::Id(Uuid::from_u128(2)),
        ] {
            for enabled in [false, true] {
                for index in [None, Some(0), Some(256)] {
                    let value = roundtrip(FilterOp::Add {
                        layer: layer.clone(),
                        alias: "new_filter".into(),
                        descriptor: Descriptor::InvertRgbV1,
                        enabled,
                        to_index: index,
                    });
                    assert_eq!(value.get("enabled"), Some(&Value::Bool(enabled)));
                    assert_eq!(value.get("to_index").is_some(), index.is_some());
                    let value = roundtrip(FilterOp::Duplicate {
                        layer: layer.clone(),
                        filter: filter.clone(),
                        alias: "copy-1".into(),
                        to_index: index,
                    });
                    assert_eq!(value.get("to_index").is_some(), index.is_some());
                }
                roundtrip(FilterOp::SetEnabled {
                    layer: layer.clone(),
                    filter: filter.clone(),
                    enabled,
                });
            }
            for descriptor in [
                Descriptor::InvertRgbV1,
                Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-100, -99).unwrap()),
                Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(100, 99).unwrap()),
            ] {
                roundtrip(FilterOp::Update {
                    layer: layer.clone(),
                    filter: filter.clone(),
                    descriptor,
                });
            }
            for to_index in [0, 256] {
                roundtrip(FilterOp::Reorder {
                    layer: layer.clone(),
                    filter: filter.clone(),
                    to_index,
                });
            }
            roundtrip(FilterOp::Remove {
                layer: layer.clone(),
                filter,
            });
        }
    }
}

#[test]
fn manual_invalid_aliases_and_indices_refuse_instead_of_truncating_or_widening() {
    let layer = LayerRef::Alias("ink".into());
    let filter = FilterRef::Alias("effect".into());
    for index in [257, usize::MAX] {
        for op in [
            FilterOp::Add {
                layer: layer.clone(),
                alias: "new".into(),
                descriptor: Descriptor::InvertRgbV1,
                enabled: false,
                to_index: Some(index),
            },
            FilterOp::Duplicate {
                layer: layer.clone(),
                filter: filter.clone(),
                alias: "copy".into(),
                to_index: Some(index),
            },
            FilterOp::Reorder {
                layer: layer.clone(),
                filter: filter.clone(),
                to_index: index,
            },
        ] {
            assert_eq!(op.to_value().unwrap_err().code, ErrorCode::InputSchema);
        }
    }
    for invalid in [
        "",
        "Upper",
        "has space",
        "0starts_with_number",
        &"a".repeat(65),
    ] {
        for op in [
            FilterOp::Add {
                layer: layer.clone(),
                alias: invalid.into(),
                descriptor: Descriptor::InvertRgbV1,
                enabled: true,
                to_index: None,
            },
            FilterOp::Duplicate {
                layer: layer.clone(),
                filter: filter.clone(),
                alias: invalid.into(),
                to_index: None,
            },
            FilterOp::Remove {
                layer: LayerRef::Alias(invalid.into()),
                filter: filter.clone(),
            },
            FilterOp::Remove {
                layer: layer.clone(),
                filter: FilterRef::Alias(invalid.into()),
            },
        ] {
            assert_eq!(op.to_value().unwrap_err().code, ErrorCode::InputSchema);
        }
    }
    // Descriptors are sealed validated types: invalid numeric domains cannot be
    // constructed for serialization through the public API.
    for (brightness, contrast) in [(101, 0), (-101, 0), (0, 100), (0, -100)] {
        assert_eq!(
            BrightnessContrastV1::new(brightness, contrast)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
