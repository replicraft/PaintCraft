use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, NoiseDistribution};
use paintcraft_document::{FilterInstance, FilterStack};
fn document(stored: bool, enabled: bool) -> Document {
    let mut doc = new_document(2, 1, Some(Rgba([30, 80, 160, 255])), 1, POLICY).unwrap();
    if stored {
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(19),
                enabled,
                Descriptor::AddNoiseRgbV1(
                    AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
                ),
            )])
            .unwrap();
    }
    doc
}
#[test]
fn source_and_same_strict_batch_select_root_without_final_state_guessing() {
    let doc = document(false, false);
    assert_eq!(Mode::capture(&doc, None), Mode::Legacy);
    assert_eq!(Mode::capture(&document(true, false), None), Mode::Noise);
    let operation = FilterOp::Add {
        layer: paintcraft_commands::LayerRef::Id(doc.roots[0]),
        alias: "new-noise".into(),
        descriptor: Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
        ),
        enabled: false,
        to_index: None,
    }
    .to_value()
    .unwrap();
    let value = Value::object([
        (
            "schema",
            Value::str(paintcraft_commands::CATALOG_BATCH_SCHEMA),
        ),
        ("policy", Value::str(POLICY)),
        ("atomic", Value::Bool(true)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(doc.state_id.to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ]);
    let batch = Batch::from_generated_value(value).unwrap();
    assert_eq!(Mode::capture(&doc, Some(&batch)), Mode::Noise);
    assert!(matches!(
        batch.operations[0],
        Op::Filter(FilterOp::Add { enabled: false, .. })
    ));
}
#[test]
fn malformed_distribution_is_rejected_before_any_grant_or_execution() {
    let doc = document(false, false);
    let mut operation = FilterOp::Add {
        layer: paintcraft_commands::LayerRef::Id(doc.roots[0]),
        alias: "bad".into(),
        descriptor: Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        enabled: true,
        to_index: None,
    }
    .to_value()
    .unwrap();
    let Value::Object(ref mut fields) = operation else {
        panic!("operation object");
    };
    let Some(Value::Object(descriptor)) = fields.get_mut("descriptor") else {
        panic!("descriptor");
    };
    descriptor.insert("distribution".into(), Value::str("Gaussian"));
    let value = Value::object([
        (
            "schema",
            Value::str(paintcraft_commands::CATALOG_BATCH_SCHEMA),
        ),
        ("policy", Value::str(POLICY)),
        ("atomic", Value::Bool(true)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(doc.state_id.to_string())),
                ("publication_epoch", Value::str("0")),
            ]),
        ),
        ("operations", Value::Array(vec![operation])),
    ]);
    let budget = SharedBudget::new(BYTES);
    let activity = budget.activity();
    let entered = std::cell::Cell::new(false);
    let result = (|| {
        let batch = Batch::from_generated_value(value)?;
        in_budget(
            Mode::capture(&doc, Some(&batch)),
            &budget,
            NoiseLimits {
                bytes: BYTES,
                base_work: WORK,
                noise_work: NOISE_WORK,
            },
            || {
                entered.set(true);
                Ok(())
            },
        )
    })();
    assert_eq!(result.unwrap_err().code, ErrorCode::InputSchema);
    assert!(!entered.get());
    assert_eq!(budget.activity(), activity);
}
#[test]
fn low_base_and_low_noise_refuse_without_increasing_the_other_component() {
    // A Noise-only direct route intentionally has no additional legacy base
    // work. Box supplies independently priced base work for this paired denial.
    let mut doc = document(true, true);
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    let mut filters = layer.filters.as_slice().to_vec();
    filters.push(FilterInstance::new(
        uuid::Uuid::from_u128(21),
        true,
        Descriptor::BoxBlurV1(paintcraft_core::types::filters::BoxBlurV1::new(1).unwrap()),
    ));
    layer.filters = FilterStack::new(filters).unwrap();
    let budget = SharedBudget::new(BYTES);
    let full = NoiseLimits {
        bytes: BYTES,
        base_work: WORK,
        noise_work: NOISE_WORK,
    };
    for limits in [
        NoiseLimits {
            base_work: 0,
            ..full
        },
        NoiseLimits {
            noise_work: 0,
            ..full
        },
    ] {
        let error = in_budget(Mode::Noise, &budget, limits, || {
            paintcraft_render::render_flat(&doc)
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(budget.usage().reserved, 0);
    }
    in_budget(Mode::Noise, &budget, full, || {
        paintcraft_render::render_flat(&doc)
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn legacy_parent_cannot_be_upgraded_and_noise_counter_is_cumulative() {
    let doc = document(true, true);
    let budget = SharedBudget::new(BYTES);
    let limits = NoiseLimits {
        bytes: BYTES,
        base_work: WORK,
        noise_work: NOISE_WORK,
    };
    in_realm(&budget, WORK, || {
        let error = in_budget(Mode::Noise, &budget, limits, || {
            paintcraft_render::render_flat(&doc)
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        Ok(())
    })
    .unwrap();
    let admission = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::NativeV1,
        limits,
    )
    .unwrap();
    let observer = admission.clone();
    with_noise_workspace(admission, || {
        paintcraft_render::render_flat(&doc)?;
        let once = observer.used()?;
        assert!(once.noise > 0);
        in_budget(Mode::Noise, &budget, limits, || {
            paintcraft_render::render_flat(&doc)
        })?;
        let twice = observer.used()?;
        assert!(twice.noise >= once.noise * 2);
        assert!(twice.base >= once.base);
        Ok(())
    })
    .unwrap();
    drop(observer);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn legacy_capture_keeps_depleted_parent_even_when_a_fresh_budget_is_available() {
    use paintcraft_core::types::{Rect, filters::BoxBlurV1};
    let mut doc = document(false, false);
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(20),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        )])
        .unwrap();
    let cost = paintcraft_render::spatial::quote_layer(
        &doc.layers[&doc.roots[0]],
        Rect::new(0, 0, 2, 1).unwrap(),
        &|| Ok(()),
    )
    .unwrap()
    .work;
    let parent_budget = SharedBudget::new(BYTES);
    let spare_budget = SharedBudget::new(BYTES);
    let parent = WorkspaceAdmission::new(
        parent_budget.scope(AllocationClass::SharedBudget),
        BYTES,
        cost,
    );
    let observer = parent.clone();
    let limits = NoiseLimits {
        bytes: BYTES,
        base_work: WORK,
        noise_work: NOISE_WORK,
    };
    with_workspace(parent, || {
        in_budget(Mode::Legacy, &spare_budget, limits, || {
            paintcraft_render::render_flat(&doc)
        })?;
        assert!(observer.work_used() > 0);
        assert_eq!(
            in_budget(Mode::Legacy, &spare_budget, limits, || {
                paintcraft_render::render_flat(&doc)
            })
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(spare_budget.activity(), 0);
    assert_eq!(parent_budget.usage().reserved, 0);
}

#[test]
fn zero_amount_noise_only_can_use_zero_base_but_still_spends_noise() {
    let doc = document(true, true);
    let budget = SharedBudget::new(BYTES);
    let limits = NoiseLimits {
        bytes: BYTES,
        base_work: 0,
        noise_work: NOISE_WORK,
    };
    in_budget(Mode::Noise, &budget, limits, || {
        let observer =
            capture_noise_or_current(|| panic!("must retain the installed pair"), limits)?;
        assert_eq!(observer.used()?.base, 0);
        assert_eq!(observer.used()?.noise, 0);
        assert_eq!(
            paintcraft_render::render_flat(&doc)?,
            [30, 80, 160, 255].repeat(2)
        );
        assert_eq!(observer.used()?.base, 0);
        assert!(observer.used()?.noise > 0);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
    let denied = NoiseLimits {
        noise_work: 0,
        ..limits
    };
    assert_eq!(
        in_budget(Mode::Noise, &budget, denied, || {
            paintcraft_render::render_flat(&doc)
        })
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
}
