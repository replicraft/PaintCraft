//! Authored controls; Root executes only after exact-source promotion.
use super::*;
use paintcraft_core::types::{
    LayerBlendMode, Rgba,
    filters::{AddNoiseRgbV1, Descriptor, LevelsRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::cell::Cell;
fn scene() -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.id = deterministic_id(&["source8-service-front"]);
    layer.blend_mode = LayerBlendMode::Multiply;
    layer.opacity = 64;
    layer.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([204, 77, 13, 128]));
    doc.roots.push(layer.id);
    doc.layers.insert(layer.id, layer);
    doc
}
fn owner(scope: paintcraft_core::types::resources::BudgetScope, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(scope, 1 << 20, work)
}
fn level(doc: &mut Document, policy: LevelsRgbV1) {
    doc.layers.get_mut(&doc.roots[1]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["source8-service-level"]),
            true,
            Descriptor::LevelsRgbV1(policy),
        )])
        .unwrap();
}
#[test]
fn exact_and_one_short_retain_the_same_prefix_and_zero_key_owner() {
    for (total, expected) in [(17696, Some([249, 233, 225, 255])), (17695, None)] {
        let doc = scene();
        let before = doc.clone();
        let budget = SharedBudget::new(2 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let a = owner(scope.clone(), total);
        let spend = |n| a.reserve_work(n);
        let pq = program_quote_checked(&doc, Caller::Base(&a), &|| Ok(()), &spend).unwrap();
        assert_eq!(a.work_used(), 4160);
        assert_eq!(
            (
                pq.compile_work,
                pq.new_retained_bytes,
                pq.required_retained_bytes
            ),
            (0, 0, 0)
        );
        assert_eq!(pq.prepare_metadata_work, 4160);
        let call = call_quote_checked(
            &doc,
            doc.canvas_rect(),
            Caller::Base(&a),
            &|| Ok(()),
            &spend,
        )
        .unwrap();
        assert_eq!(a.work_used(), 8832);
        assert_eq!(
            (
                call.call_metadata_work,
                call.setup_work,
                call.opaque_work,
                call.general_work
            ),
            (4160, 512, 32, 0)
        );
        let programs =
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &spend).unwrap();
        assert_eq!(a.work_used(), 12992);
        assert!(programs.same_job(Caller::Base(&a)));
        assert!(!programs.workspace().has_prepared_programs());
        assert_eq!(programs.retained_bytes(), 0);
        let observed = Cell::new(false);
        let result = render_call_checked(&doc, doc.canvas_rect(), &programs, &|| {
            if super::super::allowed(&doc) && !observed.replace(true) {
                // Fresh gateway has charged setup512 after actual validation4160.
                assert_eq!(a.work_used(), 17664);
            }
            Ok(())
        });
        if let Some(pixel) = expected {
            assert_eq!(result.unwrap(), pixel);
            assert_eq!(a.work_used(), 17696);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(a.work_used(), 17664);
        }
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
        assert_eq!(scope.footprint().live, 0);
        assert!(crate::spatial::current().is_none());
        assert!(!super::super::allowed(&doc));
    }
}
#[test]
fn quote_only_does_not_compile_and_metadata_denial_precedes_filter_helpers() {
    let mut doc = scene();
    level(&mut doc, LevelsRgbV1::new(0, 255, 999, 0, 255).unwrap());
    let budget = SharedBudget::new(2 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let a = owner(scope.clone(), 4160);
    let q = program_quote_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n));
    assert_eq!(q.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(a.work_used(), 4160);
    assert!(!a.has_prepared_programs());
    assert_eq!(scope.footprint().live, 0);
    let large = owner(scope.clone(), u64::MAX);
    let q = program_quote_checked(&doc, Caller::Base(&large), &|| Ok(()), &|n| {
        large.reserve_work(n)
    })
    .unwrap();
    assert!(q.compile_work > 0);
    assert!(!large.has_prepared_programs());
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn ambient_frozen_missing_key_cannot_be_replaced_by_same_counter_unbound_or_wider_owner() {
    let budget = SharedBudget::new(4 << 20);
    let a = owner(budget.scope(AllocationClass::SharedBudget), u64::MAX);
    let mut old = scene();
    level(&mut old, LevelsRgbV1::default());
    let narrow =
        prepare_programs_checked(&old, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    let mut candidate = old.clone();
    level(
        &mut candidate,
        LevelsRgbV1::new(0, 255, 100, 17, 17).unwrap(),
    );
    let wider = prepare_programs_checked(&candidate, Caller::Base(&a), &|| Ok(()), &|n| {
        a.reserve_work(n)
    })
    .unwrap();
    crate::spatial::with_workspace(narrow.workspace().clone(), || {
        for caller in [Caller::Base(&a), Caller::Base(wider.workspace())] {
            assert_eq!(
                program_quote_checked(&candidate, caller, &|| Ok(()), &|n| a.reserve_work(n))
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            assert_eq!(
                prepare_programs_checked(&candidate, caller, &|| Ok(()), &|n| a.reserve_work(n))
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
        }
        assert_eq!(
            render_call_checked(&candidate, candidate.canvas_rect(), &wider, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    assert!(!a.has_prepared_programs());
}
#[test]
fn captured_minima_and_reserved_physical_scope_are_not_replaced() {
    let doc = scene();
    let budget = SharedBudget::new(8 << 20);
    let capacity = blend_programs::CAPTURE_BYTES - 1;
    let reserved = budget.reserve_capacity(capacity).unwrap();
    let scope = reserved.clone().scope(AllocationClass::SharedBudget);
    assert_eq!(
        (
            scope.capacity_snapshot().ceiling,
            scope.capacity_snapshot().available
        ),
        (capacity, capacity)
    );
    let a = owner(scope.clone(), u64::MAX);
    assert_eq!(
        prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .err()
            .unwrap()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(reserved.available(), capacity);
    assert_eq!(a.work_used(), 0);
    assert_eq!(budget.usage().reserved, capacity);
    let current = a.tightened(1, 1).unwrap();
    crate::spatial::with_workspace(current, || {
        assert!(
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
                .is_err()
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(a.work_used(), 0);
}
#[test]
fn disabled_noise_still_requires_the_actual_pair_and_base_cannot_drop_ambient_pair() {
    use crate::noise_work::{NoiseJobPolicy, NoiseLimits, WorkComponents};
    let mut doc = scene();
    doc.layers.get_mut(&doc.roots[1]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["disabled-noise"]),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let base = owner(scope.clone(), u64::MAX);
    assert_eq!(
        program_quote_checked(&doc, Caller::Base(&base), &|| Ok(()), &|n| base
            .reserve_work(n))
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    let pair = NoiseAdmission::new_root(
        scope.clone(),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 1 << 20,
            base_work: 1 << 20,
            noise_work: 1 << 20,
        },
    )
    .unwrap();
    assert_eq!(
        pair.retained_control_bytes(),
        NoiseAdmission::control_quote().unwrap().counter_bytes
    );
    let programs = prepare_programs_checked(&doc, Caller::Noise(&pair), &|| Ok(()), &|n| {
        pair.reserve_base_work(n)
    })
    .unwrap();
    assert!(programs.same_job(Caller::Noise(&pair)));
    assert!(!programs.same_job(Caller::Base(pair.workspace())));
    assert_eq!(
        render_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(())).unwrap(),
        [249, 233, 225, 255]
    );
    assert_eq!(pair.used().unwrap().noise, 0);
    assert!(pair.used().unwrap().base > 0);
    crate::noise_work::with_noise_workspace(pair.clone(), || {
        assert_eq!(
            program_quote_checked(&doc, Caller::Base(pair.workspace()), &|| Ok(()), &|n| pair
                .reserve_base_work(n))
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        pair.used().unwrap().checked_total().unwrap(),
        pair.used().unwrap().base
    );
    pair.check_available(WorkComponents::default()).unwrap();
}
#[test]
fn cancellation_keeps_prefix_and_no_output_or_cached_context_escapes() {
    let doc = scene();
    let budget = SharedBudget::new(2 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let a = owner(scope.clone(), u64::MAX);
    let programs =
        prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    let before = a.work_used();
    let error = render_call_checked(&doc, doc.canvas_rect(), &programs, &|| {
        if super::super::allowed(&doc) {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel actual body",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(a.work_used(), before + 4160 + 512);
    assert_eq!(scope.footprint().live, 0);
    assert!(!super::super::allowed(&doc));
}
#[test]
fn public_callback_reentry_cannot_inherit_the_private_prepared_gate() {
    let mut doc = scene();
    doc.canvas = (2, 1);
    for id in &doc.roots {
        let layer = doc.layers.get_mut(id).unwrap();
        layer.raster = paintcraft_raster::RasterLayer::new(2, 1, layer.raster.default_pixel());
    }
    let budget = SharedBudget::new(2 << 20);
    let a = owner(budget.scope(AllocationClass::SharedBudget), u64::MAX);
    let programs =
        prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    let once = Cell::new(false);
    let out = render_call_checked(&doc, Rect::new(0, 0, 1, 1).unwrap(), &programs, &|| {
        if super::super::allowed(&doc) && !once.replace(true) {
            let current = crate::spatial::current().unwrap();
            let before = a.work_used();
            let child = current.tightened(current.bytes_limit(), before + 63)?;
            let error = crate::spatial::with_workspace(child, || {
                crate::render_region(&doc, doc.canvas_rect())
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::ResourceLimit);
            assert_eq!(a.work_used(), before);
        }
        Ok(())
    })
    .unwrap();
    assert!(once.get());
    assert_eq!(out, [249, 233, 225, 255]);
}
#[test]
fn borrowed_programs_keep_required_bytes_and_exact_current_context_peak() {
    let mut doc = scene();
    level(&mut doc, LevelsRgbV1::default());
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let a = owner(scope.clone(), u64::MAX);
    let programs =
        prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
    let retained = programs.retained_bytes();
    assert!(retained > 0);
    let q = program_quote_checked(&doc, Caller::Base(programs.workspace()), &|| Ok(()), &|n| {
        a.reserve_work(n)
    })
    .unwrap();
    assert_eq!(q.required_retained_bytes, retained);
    assert_eq!(
        (
            q.new_retained_bytes,
            q.preparation_peak_bytes,
            q.compile_work
        ),
        (0, 0, 0)
    );
    let before = scope.footprint().live;
    let borrowed =
        prepare_programs_checked(&doc, Caller::Base(programs.workspace()), &|| Ok(()), &|n| {
            a.reserve_work(n)
        })
        .unwrap();
    assert!(std::ptr::eq(
        programs.workspace().prepared_levels().unwrap(),
        borrowed.workspace().prepared_levels().unwrap()
    ));
    assert_eq!(scope.footprint().live, before);
    let peak = super::super::control_quote() + retained;
    for (bytes, success) in [(peak, true), (peak - 1, false)] {
        let child = borrowed.workspace().tightened(bytes, u64::MAX).unwrap();
        let result = crate::spatial::with_workspace(child, || {
            render_call_checked(&doc, doc.canvas_rect(), &borrowed, &|| Ok(()))
        });
        if success {
            assert_eq!(result.unwrap(), [249, 233, 225, 255]);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        assert_eq!(scope.footprint().live, before);
    }
}
#[test]
fn foreign_authority_and_mismatched_noise_pair_refuse_without_replacement() {
    use crate::noise_work::{NoiseJobPolicy, NoiseLimits};
    let doc = scene();
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let a = owner(scope.clone(), u64::MAX);
    let foreign = owner(scope.clone(), u64::MAX);
    crate::spatial::with_workspace(a.clone(), || {
        assert_eq!(
            program_quote_checked(&doc, Caller::Base(&foreign), &|| Ok(()), &|n| foreign
                .reserve_work(n))
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    assert_eq!((a.work_used(), foreign.work_used()), (0, 0));
    let limits = NoiseLimits {
        bytes: 1 << 20,
        base_work: 1 << 20,
        noise_work: 1 << 20,
    };
    let first = NoiseAdmission::new_root(scope.clone(), NoiseJobPolicy::BrowserV1, limits).unwrap();
    let second = NoiseAdmission::new_root(scope, NoiseJobPolicy::BrowserV1, limits).unwrap();
    crate::noise_work::with_noise_workspace(first.clone(), || {
        assert_eq!(
            program_quote_checked(&doc, Caller::Noise(&second), &|| Ok(()), &|n| second
                .reserve_base_work(n))
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(first.used().unwrap().checked_total().unwrap(), 0);
    assert_eq!(second.used().unwrap().checked_total().unwrap(), 0);
}
#[test]
fn wider_parent_callbacks_cannot_spend_past_actual_base_or_noise_child_minima() {
    use crate::noise_work::{NoiseJobPolicy, NoiseLimits};
    // Two roots: entry/root metadata4160, then top's one stored filter16.
    // Child4175 must retain4160 and deny16 BEFORE invoking the wider callback.
    for preparing in [false, true] {
        let mut doc = scene();
        level(&mut doc, LevelsRgbV1::default());
        let before = doc.clone();
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let parent = owner(scope.clone(), u64::MAX);
        let child = parent.tightened(1 << 20, 4175).unwrap();
        let offending_callback = Cell::new(false);
        let spend = |units| {
            if units == 16 {
                offending_callback.set(true);
            }
            parent.reserve_work(units)
        };
        let error = crate::spatial::with_workspace(child, || {
            if preparing {
                prepare_programs_checked(&doc, Caller::Base(&parent), &|| Ok(()), &spend)
                    .map(|_| ())
            } else {
                program_quote_checked(&doc, Caller::Base(&parent), &|| Ok(()), &spend).map(|_| ())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(parent.work_used(), 4160);
        assert!(!offending_callback.get());
        assert_eq!(scope.footprint().live, 0);
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
        let pair = NoiseAdmission::new_root(
            scope.clone(),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 1 << 20,
                base_work: 1 << 20,
                noise_work: 1 << 20,
            },
        )
        .unwrap();
        let tight = pair
            .child(NoiseLimits {
                bytes: 1 << 20,
                base_work: 4175,
                noise_work: 0,
            })
            .unwrap();
        let noise_offending = Cell::new(false);
        let spend = |units| {
            if units == 16 {
                noise_offending.set(true);
            }
            pair.reserve_base_work(units)
        };
        let error = crate::noise_work::with_noise_workspace(tight, || {
            if preparing {
                prepare_programs_checked(&doc, Caller::Noise(&pair), &|| Ok(()), &spend).map(|_| ())
            } else {
                program_quote_checked(&doc, Caller::Noise(&pair), &|| Ok(()), &spend).map(|_| ())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(
            pair.used().unwrap(),
            crate::noise_work::WorkComponents {
                base: 4160,
                noise: 0
            }
        );
        assert!(!noise_offending.get());
        assert_eq!(scope.footprint().live, pair.retained_control_bytes());
        assert!(doc.same_semantics(&before));
        assert_eq!(doc.state_id, before.state_id);
    }
}
#[test]
fn distinct_prepared_owners_require_their_union_not_only_explicit_retention() {
    let mut doc = scene();
    level(&mut doc, LevelsRgbV1::default());
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let parent = owner(scope.clone(), u64::MAX);
    let first = prepare_programs_checked(&doc, Caller::Base(&parent), &|| Ok(()), &|n| {
        parent.reserve_work(n)
    })
    .unwrap();
    let second = prepare_programs_checked(&doc, Caller::Base(&parent), &|| Ok(()), &|n| {
        parent.reserve_work(n)
    })
    .unwrap();
    assert!(!std::ptr::eq(
        first.workspace().prepared_levels().unwrap(),
        second.workspace().prepared_levels().unwrap()
    ));
    let retained = first.retained_bytes() + second.retained_bytes();
    let before = scope.footprint().live;
    assert_eq!(before, retained);
    for (ceiling, success) in [
        (
            super::super::control_quote() + second.retained_bytes(),
            false,
        ),
        (super::super::control_quote() + retained - 1, false),
        (super::super::control_quote() + retained, true),
    ] {
        let current = first.workspace().tightened(ceiling, u64::MAX).unwrap();
        crate::spatial::with_workspace(current, || {
            let q =
                program_quote_checked(&doc, Caller::Base(second.workspace()), &|| Ok(()), &|n| {
                    parent.reserve_work(n)
                })?;
            assert_eq!(
                (
                    q.required_retained_bytes,
                    q.new_retained_bytes,
                    q.preparation_peak_bytes
                ),
                (retained, 0, 0)
            );
            let q = call_quote_checked(
                &doc,
                doc.canvas_rect(),
                Caller::Base(second.workspace()),
                &|| Ok(()),
                &|n| parent.reserve_work(n),
            )?;
            assert_eq!(q.required_retained_bytes, retained);
            let result = render_call_checked(&doc, doc.canvas_rect(), &second, &|| Ok(()));
            if success {
                assert_eq!(result?, [249, 233, 225, 255]);
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(scope.footprint().live, before);
    }
    // Actual cloned top-owner identity deduplicates; equal keys alone did not.
    crate::spatial::with_workspace(second.workspace().clone(), || {
        let q = program_quote_checked(&doc, Caller::Base(second.workspace()), &|| Ok(()), &|n| {
            parent.reserve_work(n)
        })?;
        assert_eq!(q.required_retained_bytes, second.retained_bytes());
        Ok(())
    })
    .unwrap();
}
mod extended;
