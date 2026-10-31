//! Independent literals/direct-square expectations, not product blur as an oracle.
use super::*;
use paintcraft_core::types::{
    filters::{BoxBlurV1, Descriptor, ThresholdLumaV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack, POLICY};
use paintcraft_raster::{
    Admission, BlendMode,
    coverage_mask::{A8Mask, Limits},
};
use std::cell::Cell;

fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn blur(radius: i16) -> Descriptor {
    Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap())
}
fn filter(name: &str, descriptor: Descriptor) -> FilterInstance {
    FilterInstance::new(deterministic_id(&[name]), true, descriptor)
}
fn layer(width: u32, height: u32, pixels: &[Rgba], filters: Vec<FilterInstance>) -> Layer {
    let bytes: Vec<_> = pixels.iter().flat_map(|p| p.0).collect();
    let doc = paintcraft_document::import_image(width, height, &bytes, &"ab".repeat(32)).unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.filters = FilterStack::new(filters).unwrap();
    layer.origin = (-77, -31);
    layer
}
fn mask(area: Rect, coverage: u8) -> FilterMask {
    FilterMask::new(
        true,
        A8Mask::uniform(
            area,
            coverage,
            &SharedBudget::new(8192),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    )
}
fn evaluate(layer: &Layer, output: Rect) -> Vec<Rgba> {
    let budget = SharedBudget::new(32 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), 32 << 20, u64::MAX);
    let plan = LayerAppearancePlan::new(layer, output, &|| Ok(())).unwrap();
    let mut result = vec![Rgba([249; 4]); output.area() as usize];
    plan.evaluate_into(&admission, &mut result, &|| Ok(()))
        .unwrap();
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(scope.footprint().live, 0);
    assert!(budget.usage().peak >= plan.quote().bytes);
    result
}
fn extract(pixels: &[Rgba], width: usize, output: Rect) -> Vec<Rgba> {
    let mut result = Vec::new();
    for y in output.y0..output.y1 {
        let start = y as usize * width + output.x0 as usize;
        result.extend_from_slice(&pixels[start..start + output.width() as usize]);
    }
    result
}
// Direct full-square reference, no sliding sums, BoxPlan, Effects or render calls.
// Positive integer ratios round through doubled numerators (ties upward).
fn square_reference(input: &[Rgba], width: usize, height: usize, radius: i64) -> Vec<Rgba> {
    if radius == 0 {
        return input.to_vec();
    }
    let rounded = |n: u128, d: u128| ((2 * n + d) / (2 * d)) as u8;
    let mut result = Vec::new();
    for y in 0..height as i64 {
        for x in 0..width as i64 {
            let mut sums = [0u128; 4];
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let p = input[(y + dy).clamp(0, height as i64 - 1) as usize * width
                        + (x + dx).clamp(0, width as i64 - 1) as usize];
                    let alpha = u128::from(p.a());
                    for (sum, channel) in sums[..3].iter_mut().zip(p.0) {
                        *sum += u128::from(channel) * alpha;
                    }
                    sums[3] += alpha;
                }
            }
            result.push(if sums[3] == 0 {
                Rgba([0; 4])
            } else {
                Rgba([
                    rounded(sums[0], sums[3]),
                    rounded(sums[1], sums[3]),
                    rounded(sums[2], sums[3]),
                    rounded(sums[3], (2 * radius + 1).pow(2) as u128),
                ])
            });
        }
    }
    result
}

#[test]
fn threshold_box_order_is_noncommuting_and_uses_the_incoming_prefix() {
    let input = [
        Rgba([0, 0, 0, 255]),
        Rgba([90, 60, 30, 255]),
        Rgba([240, 120, 0, 255]),
    ];
    let threshold = Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(64).unwrap());
    let first = layer(
        3,
        1,
        &input,
        vec![filter("threshold", threshold), filter("blur", blur(1))],
    );
    let reverse = layer(
        3,
        1,
        &input,
        vec![filter("blur", blur(1)), filter("threshold", threshold)],
    );
    // Independent scaled-luma decisions followed by three-tap clamped averages.
    let a = [
        Rgba([85, 85, 85, 255]),
        Rgba([170, 170, 170, 255]),
        Rgba([255; 4]),
    ];
    let b = [Rgba([0, 0, 0, 255]), Rgba([255; 4]), Rgba([255; 4])];
    assert_eq!(evaluate(&first, rect(0, 0, 3, 1)), a);
    assert_eq!(evaluate(&reverse, rect(0, 0, 3, 1)), b);
    assert_ne!(a, b);
    assert_eq!(evaluate(&first, rect(1, 0, 1, 1)), [a[1]]);
    assert_eq!(first.raster.get(1, 0), input[1]);
}

#[test]
fn two_blurs_mix_masks_once_against_each_incoming_prefix_not_the_raw_source() {
    let input = [
        Rgba([255, 0, 0, 255]),
        Rgba([0, 255, 0, 0]),
        Rgba([0, 0, 255, 128]),
        Rgba([255, 255, 0, 64]),
        Rgba([1, 2, 3, 0]),
    ];
    let first = filter("first", blur(1)).with_mask(Some(mask(rect(-1, 0, 4, 1), 128)));
    let second = filter("second", blur(2)).with_mask(Some(mask(rect(1, 0, 4, 1), 127)));
    let prefix = filter("invert", Descriptor::InvertRgbV1);
    let one = layer(5, 1, &input, vec![prefix.clone(), first.clone()]);
    let two = layer(5, 1, &input, vec![prefix, first, second]);
    // Independent stdlib Python Fraction: direct clamped square, round complete
    // candidate, then one premultiplied coverage mix against that stage's prefix.
    let after_one = [
        Rgba([0, 255, 255, 212]),
        Rgba([85, 255, 170, 64]),
        Rgba([227, 227, 28, 96]),
        Rgba([0, 0, 255, 64]),
        Rgba([254, 253, 252, 0]),
    ];
    let expected = [
        Rgba([0, 255, 255, 212]),
        Rgba([56, 236, 199, 97]),
        Rgba([149, 219, 106, 92]),
        Rgba([50, 70, 205, 55]),
        Rgba([136, 136, 119, 16]),
    ];
    assert_eq!(evaluate(&one, rect(0, 0, 5, 1)), after_one);
    assert_eq!(evaluate(&two, rect(0, 0, 5, 1)), expected);
    let plan = LayerAppearancePlan::new(&two, rect(2, 0, 1, 1), &|| Ok(())).unwrap();
    assert_eq!(plan.required_input(), rect(0, 0, 5, 1));
    assert_eq!(plan.quote().stages, 3);
    assert_eq!(evaluate(&two, rect(2, 0, 1, 1)), [expected[2]]);
    assert_ne!(
        expected[1],
        Rgba([42, 226, 213, 65]),
        "Do not mix the last candidate against raw pixels"
    );
}

#[test]
fn hidden_alpha_carry_and_disabled_or_zero_radius_spatial_meaning_are_explicit() {
    let input = [
        Rgba([100, 20, 220, 0]),
        Rgba([255, 0, 0, 1]),
        Rgba([9, 9, 9, 0]),
    ];
    let blurred = layer(3, 1, &input, vec![filter("blur", blur(32))]);
    assert_eq!(
        evaluate(&blurred, rect(0, 0, 3, 1)),
        [Rgba([255, 0, 0, 0]); 3]
    );
    let mut point_after = blurred.clone();
    point_after.filters = FilterStack::new(vec![
        filter("blur", blur(32)),
        filter("invert", Descriptor::InvertRgbV1),
    ])
    .unwrap();
    assert_eq!(
        evaluate(&point_after, rect(0, 0, 3, 1)),
        [Rgba([0, 255, 255, 0]); 3]
    );
    let zero = layer(
        3,
        1,
        &input,
        vec![filter("zero", blur(0)).with_mask(Some(mask(rect(0, 0, 3, 1), 128)))],
    );
    assert!(active(&zero));
    let request = LayerAppearancePlan::new(&zero, rect(0, 0, 3, 1), &|| Ok(())).unwrap();
    assert_eq!(request.quote().scratch_elements, 0);
    assert_eq!(request.quote().stages, 1);
    assert_eq!(evaluate(&zero, rect(0, 0, 3, 1)), input);
    let disabled = layer(
        3,
        1,
        &input,
        vec![filter("disabled", blur(32)).with_enabled(false)],
    );
    assert!(!active(&disabled));
    assert_eq!(
        LayerAppearancePlan::new(&disabled, rect(1, 0, 1, 1), &|| Ok(()))
            .unwrap()
            .quote()
            .stages,
        0
    );
    assert_eq!(evaluate(&disabled, rect(0, 0, 3, 1)), input);
}

#[test]
fn source_edges_and_tiled_regions_include_sparse_implicit_defaults() {
    let width = 300;
    let height = 2;
    let default = Rgba([40, 80, 120, 64]);
    let doc = paintcraft_document::new_document(width, height, Some(default), 73, POLICY).unwrap();
    let mut source = doc.layers[&doc.roots[0]].clone();
    assert_eq!(source.raster.resident_tiles(), 0);
    source.origin = (-1000, -20);
    source.filters = FilterStack::new(vec![filter("blur", blur(1))]).unwrap();
    assert_eq!(evaluate(&source, rect(31, 0, 33, 2)), vec![default; 66]);
    source
        .raster
        .blend_pixel(
            255,
            0,
            Rgba([255; 4]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    assert_eq!(source.raster.resident_tiles(), 1);
    let before = source.clone();
    let mut raw = vec![default; width as usize * height as usize];
    raw[255] = Rgba([255; 4]);
    let expected = square_reference(&raw, width as usize, height as usize, 1);
    assert_eq!(
        evaluate(&source, rect(0, 0, width.into(), height.into())),
        expected
    );
    for edge in [1, 3, 32, 64, 128] {
        let mut assembled = vec![Rgba([249; 4]); expected.len()];
        for y in 0..height {
            for x in (0..width).step_by(edge) {
                let output = rect(
                    x.into(),
                    y.into(),
                    ((width - x) as usize).min(edge) as i64,
                    1,
                );
                let part = evaluate(&source, output);
                let start = y as usize * width as usize + x as usize;
                assembled[start..start + part.len()].copy_from_slice(&part);
            }
        }
        assert_eq!(assembled, expected, "partition edge {edge}");
    }
    let interior = rect(254, 0, 4, 2);
    assert_eq!(
        evaluate(&source, interior),
        extract(&expected, width as usize, interior)
    );
    assert_eq!(source, before);
}

#[test]
fn quote_limits_and_shared_realm_refusal_precede_extraction_and_output_writes() {
    let source = layer(
        3,
        1,
        &[Rgba([40, 80, 120, 64]); 3],
        vec![filter("blur", blur(1))],
    );
    let original = source.clone();
    let plan = LayerAppearancePlan::new(&source, rect(1, 0, 1, 1), &|| Ok(())).unwrap();
    let quote = plan.quote();
    assert_eq!(plan.required_input(), rect(0, 0, 3, 1));
    assert_eq!(quote.pixels_per_buffer, 3);
    assert_eq!(quote.scratch_elements, 4);
    assert!(quote.bytes >= 3 * 8 + 4 * 8);
    assert!(quote.work > 0);
    let budget = SharedBudget::new(quote.bytes);
    let scope = budget.scope(AllocationClass::SharedBudget);
    for (bytes, work) in [(quote.bytes - 1, quote.work), (quote.bytes, quote.work - 1)] {
        let mut output = [Rgba([249; 4])];
        let error = plan
            .evaluate_into(
                &WorkspaceAdmission::new(scope.clone(), bytes, work),
                &mut output,
                &|| panic!("Limits precede callback and extraction"),
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(output, [Rgba([249; 4])]);
        assert_eq!(budget.usage().reserved, 0);
    }
    let blocker = budget.reserve(1).unwrap();
    let calls = Cell::new(0);
    let mut output = [Rgba([249; 4])];
    let error = plan
        .evaluate_into(
            &WorkspaceAdmission::new(scope.clone(), quote.bytes, quote.work),
            &mut output,
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        calls.get(),
        1,
        "Shared denial happens before workspace/extraction callbacks"
    );
    assert_eq!(output, [Rgba([249; 4])]);
    assert_eq!(budget.usage().reserved, 1);
    drop(blocker);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(source, original);
    assert_eq!(
        plan.evaluate_into(
            &WorkspaceAdmission::new(scope, quote.bytes, quote.work),
            &mut [],
            &|| panic!("Length validation precedes work")
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}

#[test]
fn simultaneous_evaluations_share_the_same_admission_instead_of_minting_a_realm() {
    let source = layer(
        3,
        1,
        &[Rgba([7, 19, 31, 71]); 3],
        vec![filter("blur", blur(1))],
    );
    let plan = LayerAppearancePlan::new(&source, rect(0, 0, 3, 1), &|| Ok(())).unwrap();
    let budget = SharedBudget::new(plan.quote().bytes);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), plan.quote().bytes, u64::MAX);
    let nested = Cell::new(false);
    let mut output = [Rgba([249; 4]); 3];
    plan.evaluate_into(&admission, &mut output, &|| {
        if budget.usage().reserved > 0 && !nested.replace(true) {
            let mut other = [Rgba([249; 4]); 3];
            let denial = plan
                .evaluate_into(&admission, &mut other, &|| Ok(()))
                .unwrap_err();
            assert_eq!(denial.code, ErrorCode::ResourceLimit);
            assert!(
                denial.allocation_denial().is_some(),
                "Prove shared bytes, not cumulative work, refused the nested job"
            );
            assert_eq!(other, [Rgba([249; 4]); 3]);
        }
        Ok(())
    })
    .unwrap();
    assert!(nested.get());
    assert_eq!(output, [Rgba([7, 19, 31, 71]); 3]);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn cloned_admissions_share_aggregate_work_and_a_new_job_reuses_the_same_byte_realm() {
    let source = layer(
        3,
        1,
        &[Rgba([7, 19, 31, 71]); 3],
        vec![filter("blur", blur(1))],
    );
    let plan = LayerAppearancePlan::new(&source, rect(0, 0, 3, 1), &|| Ok(())).unwrap();
    let budget = SharedBudget::new(plan.quote().bytes);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), plan.quote().bytes, plan.quote().work);
    let cloned = admission.clone();
    let mut output = [Rgba([249; 4]); 3];
    plan.evaluate_into(&admission, &mut output, &|| Ok(()))
        .unwrap();
    assert_eq!(output, [Rgba([7, 19, 31, 71]); 3]);
    assert_eq!(admission.work_used(), plan.quote().work);
    assert_eq!(cloned.work_used(), plan.quote().work);
    assert_eq!(budget.usage().reserved, 0);
    let callbacks = Cell::new(0);
    let mut blocked = [Rgba([249; 4]); 3];
    let error = plan
        .evaluate_into(&cloned, &mut blocked, &|| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(
        error.allocation_denial().is_none(),
        "This refusal is aggregate work, not shared memory"
    );
    assert_eq!(
        callbacks.get(),
        1,
        "Work cap rejects before allocation/extraction callbacks"
    );
    assert_eq!(blocked, [Rgba([249; 4]); 3]);
    assert_eq!(cloned.work_used(), plan.quote().work);
    assert_eq!(budget.usage().reserved, 0);
    let next_job = WorkspaceAdmission::new(scope, plan.quote().bytes, plan.quote().work);
    plan.evaluate_into(&next_job, &mut blocked, &|| Ok(()))
        .unwrap();
    assert_eq!(blocked, output);
    assert_eq!(next_job.work_used(), plan.quote().work);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn every_execution_checkpoint_cancels_without_retained_charge_or_source_changes() {
    let raw: Vec<_> = (0..240)
        .map(|i| {
            Rgba([
                (i * 13 % 256) as u8,
                (i * 29 % 256) as u8,
                (i * 37 % 256) as u8,
                (i * 17 % 256) as u8,
            ])
        })
        .collect();
    let source = layer(
        20,
        12,
        &raw,
        vec![
            filter("one", blur(1)),
            filter("invert", Descriptor::InvertRgbV1),
            filter("two", blur(1)).with_mask(Some(mask(rect(0, 0, 20, 12), 127))),
        ],
    );
    let original = source.clone();
    let plan = LayerAppearancePlan::new(&source, rect(2, 2, 16, 8), &|| Ok(())).unwrap();
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let admission = WorkspaceAdmission::new(scope.clone(), 1 << 20, u64::MAX);
    let sentinel = Rgba([249, 250, 251, 252]);
    let calls = Cell::new(0);
    let mut baseline = vec![sentinel; 128];
    plan.evaluate_into(&admission, &mut baseline, &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert!(calls.get() > 4);
    let mut untouched = false;
    let mut partial = false;
    let mut completed_but_rejected = false;
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let mut output = vec![sentinel; 128];
        let error = plan
            .evaluate_into(&admission, &mut output, &|| {
                current.set(current.get() + 1);
                if current.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "deterministic phase cancellation",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(current.get(), stop);
        assert_eq!(source, original);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(scope.footprint().live, 0);
        let written = output.iter().filter(|&&p| p != sentinel).count();
        untouched |= written == 0;
        partial |= written > 0 && written < output.len();
        // Baseline is used only to classify final cancellation, not as a pixel oracle.
        completed_but_rejected |= output == baseline;
    }
    assert!(untouched && partial && completed_but_rejected);
    let planning_calls = Cell::new(0);
    assert_eq!(
        LayerAppearancePlan::new(&source, rect(-1, 0, 1, 1), &|| panic!(
            "Invalid geometry precedes work"
        ))
        .err()
        .unwrap()
        .code,
        ErrorCode::InputSchema
    );
    let error = LayerAppearancePlan::new(&source, rect(0, 0, 1, 1), &|| {
        planning_calls.set(planning_calls.get() + 1);
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "planning cancellation",
        ))
    })
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(planning_calls.get(), 1);
}
