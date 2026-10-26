use super::*;
use std::cell::Cell;

const NATIVE: &[u8] = include_bytes!("../../../tests/abr-fixtures/native-presets.abr");
fn source() -> Arc<Library> {
    Arc::new(decode(NATIVE, &|| Ok(())).unwrap())
}
fn adopt(source: Arc<Library>, preset: usize, budget: &SharedBudget) -> Result<AdoptedPreset> {
    adopt_native(
        source,
        preset,
        StaticTipPolicy::NativeGridV1,
        &[],
        budget,
        &|| Ok(()),
    )
}
#[test]
fn native_resource_adopts_exact_noninverted_pixels_and_retains_original_source() {
    let source = source();
    let sections = source.sections().to_vec();
    let assessment = assess(&source, 0, &|| Ok(())).unwrap();
    assert!(assessment.qualified());
    assert_eq!(assessment.dimensions(), Some((3, 2)));
    assert_eq!(assessment.spacing_q8(), Some(768));
    assert!(assessment.required_conversions().is_empty());
    let budget = SharedBudget::new(1024);
    let adopted = adopt(source.clone(), 0, &budget).unwrap();
    assert!(Arc::ptr_eq(adopted.source(), &source));
    assert_eq!(adopted.tip().alpha(), [0, 1, 127, 128, 254, 255]);
    assert_eq!(adopted.tip().anchor(), (1, 1));
    assert_eq!(adopted.preset_index(), 0);
    assert_eq!(adopted.policy(), StaticTipPolicy::NativeGridV1);
    assert_eq!(source.sections(), sections);
    let clone = adopted.clone();
    assert!(clone.tip().shares_storage(adopted.tip()));
    drop(adopted);
    assert!(budget.usage().reserved > 0);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn spacing_conversion_requires_explicit_consent_and_precision_is_never_narrowed() {
    let source = source();
    let budget = SharedBudget::new(1024);
    let assessment = assess(&source, 1, &|| Ok(())).unwrap();
    assert!(assessment.qualified());
    assert_eq!(assessment.spacing_q8(), Some(77));
    assert_eq!(
        assessment.required_conversions(),
        [RequiredConversion::QuantizedSpacing]
    );
    assert_eq!(
        adopt(source.clone(), 1, &budget).unwrap_err().code,
        ErrorCode::InputSchema
    );
    assert_eq!(budget.usage().reserved, 0);
    let tip = adopt_native(
        source.clone(),
        1,
        StaticTipPolicy::NativeGridV1,
        &[RequiredConversion::QuantizedSpacing],
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(tip.spacing_q8(), 77);
    assert!(
        assess(&source, 2, &|| Ok(()))
            .unwrap()
            .unsupported()
            .contains(&UnsupportedFeature::HighDepthMask)
    );
    assert_eq!(
        source.samples()[1].alpha,
        Alpha::U16(vec![0, 1, 255, 256, 32769, 65535])
    );
}
#[test]
fn transforms_active_dynamics_missing_masks_and_unknown_fields_refuse_before_allocation() {
    let source = source();
    let budget = SharedBudget::new(1024);
    for (index, feature) in [
        (2, UnsupportedFeature::HighDepthMask),
        (3, UnsupportedFeature::MissingSample),
        (4, UnsupportedFeature::SizeOverride),
        (5, UnsupportedFeature::Rotation),
        (6, UnsupportedFeature::Roundness),
        (7, UnsupportedFeature::Flip),
        (8, UnsupportedFeature::DisabledSpacing),
        (9, UnsupportedFeature::WetEdges),
        (10, UnsupportedFeature::Noise),
        (11, UnsupportedFeature::Texture),
        (12, UnsupportedFeature::UnrecognizedDescriptor),
        (13, UnsupportedFeature::PressureDynamics),
    ] {
        let assessment = assess(&source, index, &|| Ok(())).unwrap();
        assert!(
            assessment.unsupported().contains(&feature),
            "{index}: {assessment:?}"
        );
        assert_eq!(
            adopt(source.clone(), index, &budget).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    // The original independently packed fixture has unsupported transform,
    // size and opaque fields. It never silently becomes a round brush.
    let old = Arc::new(
        decode(
            include_bytes!("../../../tests/abr-fixtures/samples-v6.abr"),
            &|| Ok(()),
        )
        .unwrap(),
    );
    assert!(!assess(&old, 1, &|| Ok(())).unwrap().qualified());
    let mut duplicate = (*source).clone();
    duplicate.samples.push(duplicate.samples[0].clone());
    assert!(
        assess(&duplicate, 0, &|| Ok(()))
            .unwrap()
            .unsupported()
            .contains(&UnsupportedFeature::AmbiguousSampleId)
    );
}
#[test]
fn cancellation_budget_denial_and_empty_masks_preserve_the_resource() {
    let source = source();
    let calls = Cell::new(0);
    let budget = SharedBudget::new(1024);
    drop(
        adopt_native(
            source.clone(),
            0,
            StaticTipPolicy::NativeGridV1,
            &[],
            &budget,
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap(),
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let result = adopt_native(
            source.clone(),
            0,
            StaticTipPolicy::NativeGridV1,
            &[],
            &budget,
            &|| {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
    }
    assert_eq!(
        adopt(source.clone(), 0, &SharedBudget::new(0))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let empty = adopt(source.clone(), 14, &budget).unwrap();
    assert!(empty.tip().is_empty());
    assert_eq!(
        source.samples()[0].alpha,
        Alpha::U8(vec![0, 1, 127, 128, 254, 255])
    );
    assert_eq!(
        assess(&source, usize::MAX, &|| Ok(())).unwrap_err().code,
        ErrorCode::InputSchema
    );
}

#[test]
fn adopted_wire_fixture_paints_independent_pixels_through_the_existing_atomic_executor() {
    use paintcraft_commands::{Batch, LayerRef, Op};
    use paintcraft_core::types::Rgba;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_editor::{execute, session::Session};
    let budget = SharedBudget::new(1 << 20);
    let adopted = adopt(source(), 0, &budget).unwrap();
    let mut session =
        Session::new(new_document(8, 8, Some(Rgba([9, 8, 7, 0])), 0, POLICY).unwrap());
    let before = session.snapshot();
    let layer = before.roots[0];
    let coverage = sampled_brush::coverage(
        adopted.tip(),
        &[(3, 3)],
        1,
        adopted.spacing_q8(),
        before.canvas_rect(),
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let batch = Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: before.state_id,
        base_epoch: session.ticket().publication_epoch,
        seed: None,
        digest: "0".repeat(64),
        operations: vec![Op::MaskSpans {
            layer: LayerRef::Id(layer),
            color: Rgba([100, 50, 25, 255]),
            spans: coverage.spans().to_vec(),
        }],
    };
    let preview = execute::execute(&before, session.selection(), &batch).unwrap();
    let prepared = session
        .preparation()
        .unwrap()
        .apply_mask_stroke(&batch, || Ok(()))
        .unwrap();
    assert!(session.document().same_semantics(&before));
    session.publish_prepared(prepared, || Ok(())).unwrap();
    for y in 0..8 {
        for x in 0..8 {
            // Independent owned wire values, explicitly no inversion. Source
            // alpha zero leaves transparent hidden RGB untouched.
            let alpha = match (x, y) {
                (3, 2) => 1,
                (4, 2) => 127,
                (2, 3) => 128,
                (3, 3) => 254,
                (4, 3) => 255,
                _ => 0,
            };
            let expected = if alpha == 0 {
                Rgba([9, 8, 7, 0])
            } else {
                Rgba([100, 50, 25, alpha])
            };
            assert_eq!(session.document().layers[&layer].raster.get(x, y), expected);
            assert_eq!(preview.doc.layers[&layer].raster.get(x, y), expected);
        }
    }
    let after = session.snapshot();
    session.undo().unwrap();
    assert!(session.document().same_semantics(&before));
    session.redo().unwrap();
    assert!(session.document().same_semantics(&after));
}
