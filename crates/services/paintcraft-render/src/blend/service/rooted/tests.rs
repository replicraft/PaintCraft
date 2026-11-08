use super::*;
use crate::blend::metadata::quote_unbound_rooted_call_checked;
use paintcraft_core::types::{
    LayerBlendMode, Rgba,
    filters::{AddNoiseRgbV1, Descriptor},
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
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = deterministic_id(&["rooted-control-front"]);
    top.blend_mode = LayerBlendMode::Multiply;
    top.opacity = 64;
    top.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([204, 77, 13, 128]));
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    doc
}
fn debt(used: &Cell<u64>, limit: u64, n: u64) -> Result<()> {
    let next = used
        .get()
        .checked_add(n)
        .filter(|v| *v <= limit)
        .ok_or_else(limit_error)?;
    used.set(next);
    Ok(())
}
fn limit_error() -> Error {
    Error::new(ErrorCode::ResourceLimit, "rooted fixture debt")
}
#[test]
fn distinct_quote_loop_and_facts_pay_before_each_helper_and_keep_prefix() {
    let doc = scene();
    for (maximum, paid, denied) in [(8767, 4672, 4096), (12945, 8768, 4178)] {
        let used = Cell::new(0u64);
        let refused = Cell::new(0u64);
        let result = quote_unbound_rooted_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
            if used.get().checked_add(n).is_none_or(|v| v > maximum) {
                refused.set(n);
            }
            debt(&used, maximum, n)
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!((used.get(), refused.get()), (paid, denied));
    }
    let used = Cell::new(0u64);
    let q = quote_unbound_rooted_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
        debt(&used, 12946, n)
    })
    .unwrap();
    assert_eq!(used.get(), 12946);
    assert_eq!(q.traversal_work, 24658);
    assert_eq!(q.call.prepared_validation_work, 4160);
    assert_eq!(
        (q.call.setup_work, q.call.opaque_work, q.call.general_work),
        (512, 32, 0)
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(q.traversal_control_bytes, 24);
    #[cfg(target_arch = "wasm32")]
    assert_eq!(q.traversal_control_bytes, 16);
}
#[test]
fn bound_quote_matches_pure_ledger_and_actual_tighter_some_refuses_extra_pass() {
    let doc = scene();
    let b = SharedBudget::new(2 << 20);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, u64::MAX);
    let q = rooted_call_quote_checked(
        &doc,
        doc.canvas_rect(),
        Caller::Base(&a),
        &|| Ok(()),
        &|n| a.reserve_work(n),
    )
    .unwrap();
    assert_eq!(a.work_used(), 12946);
    assert_eq!(q.traversal_work, 24658);
    let fresh = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, u64::MAX);
    let child = fresh.tightened(1 << 20, 8767).unwrap();
    crate::spatial::with_workspace(child, || {
        let q = rooted_call_quote_checked(
            &doc,
            doc.canvas_rect(),
            Caller::Base(&fresh),
            &|| Ok(()),
            &|n| fresh.reserve_work(n),
        );
        assert_eq!(q.unwrap_err().code, ErrorCode::ResourceLimit);
        Ok(())
    })
    .unwrap();
    assert_eq!(fresh.work_used(), 4672);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn exact_and_one_short_keep_original_counter_and_byte_witness_drops() {
    let doc = scene();
    for (maximum, pixel) in [(33522, true), (33521, false)] {
        let b = SharedBudget::new(2 << 20);
        let scope = b.scope(AllocationClass::SharedBudget);
        let a = WorkspaceAdmission::new(scope.clone(), 1 << 20, maximum);
        let programs =
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
                .unwrap();
        let mut output = scope.account();
        output.resize(4).unwrap();
        let result = render_rooted_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(()));
        if pixel {
            let bytes = result.unwrap();
            output.resize(bytes.capacity() as u64).unwrap();
            assert_eq!(bytes, [249, 233, 225, 255]);
            drop(bytes);
            assert_eq!(a.work_used(), 33522);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(a.work_used(), 33490);
        }
        drop(output);
        drop(programs);
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(b.usage().reserved, 0);
    }
}
#[test]
fn witness_plus_context_bytes_exact_and_minus_one_use_same_original_scope() {
    let doc = scene();
    for short in [0, 1] {
        let b = SharedBudget::new(2 << 20);
        let scope = b
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget);
        let a = WorkspaceAdmission::new(scope.clone(), 1 << 20, u64::MAX);
        let mut programs =
            prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
                .unwrap();
        let bytes = controls::control_bytes() + crate::blend::control_quote() - short;
        programs.base = programs.base.tightened(bytes, u64::MAX).unwrap();
        let mut output = scope.account();
        output.resize(4).unwrap();
        let result = render_rooted_call_checked(&doc, doc.canvas_rect(), &programs, &|| Ok(()));
        if short == 0 {
            let pixels = result.unwrap();
            assert_eq!(pixels, [249, 233, 225, 255]);
            drop(pixels);
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(a.work_used(), 12498);
        }
        drop(output);
        drop(programs);
        assert_eq!(scope.footprint().live, 0);
        drop(a);
        drop(scope);
        assert_eq!(b.usage().reserved, 0);
    }
}
#[test]
fn sparse_tombstoned_layer_table_has_identical_rooted_trace_and_pixels() {
    let compact = scene();
    let mut sparse = compact.clone();
    let layer = sparse.layers[&sparse.roots[0]].clone();
    let keys: Vec<_> = (0..2048)
        .map(|i| deterministic_id(&["rooted-tombstone", &i.to_string()]))
        .collect();
    for key in &keys {
        let mut l = layer.clone();
        l.id = *key;
        sparse.layers.insert(*key, l);
    }
    for key in keys {
        sparse.layers.remove(&key);
    }
    assert!(sparse.layers.capacity() > compact.layers.capacity());
    let mut traces = Vec::new();
    for doc in [&compact, &sparse] {
        let trace = std::cell::RefCell::new(Vec::new());
        let q = quote_unbound_rooted_call_checked(doc, doc.canvas_rect(), &|| Ok(()), &|n| {
            trace.borrow_mut().push(n);
            Ok(())
        })
        .unwrap();
        assert_eq!(q.traversal_work, 24658);
        traces.push(trace.into_inner());
        let b = SharedBudget::new(2 << 20);
        let scope = b.scope(AllocationClass::SharedBudget);
        let a = WorkspaceAdmission::new(scope.clone(), 1 << 20, 33522);
        let p = prepare_programs_checked(doc, Caller::Base(&a), &|| Ok(()), &|n| a.reserve_work(n))
            .unwrap();
        let mut out = scope.account();
        out.resize(4).unwrap();
        let bytes = render_rooted_call_checked(doc, doc.canvas_rect(), &p, &|| Ok(())).unwrap();
        assert_eq!(bytes, [249, 233, 225, 255]);
        drop(bytes);
        drop(out);
        drop(p);
        assert_eq!(a.work_used(), 33522);
        assert_eq!(scope.footprint().live, 0);
    }
    assert_eq!(traces[0], traces[1]);
}
#[test]
fn enabled_and_disabled_stored_noise_have_exact_branch_supplements_and_known_pixels() {
    use crate::noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits};
    for (enabled, extra, pixel) in [(true, 20578, 203), (false, 24674, 192)] {
        let mut doc = scene();
        let top = doc.layers.get_mut(&doc.roots[1]).unwrap();
        top.opacity = 255;
        top.raster = paintcraft_raster::RasterLayer::new(1, 1, Rgba([128, 128, 128, 127]));
        top.filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["rooted-noise"]),
            enabled,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
        let used = Cell::new(0u64);
        let q = quote_unbound_rooted_call_checked(&doc, doc.canvas_rect(), &|| Ok(()), &|n| {
            debt(&used, 12978, n)
        })
        .unwrap();
        assert_eq!(used.get(), 12978);
        assert!(q.call.requires_noise);
        assert_eq!(q.traversal_work, extra);
        let mut work = Vec::new();
        for rooted in [false, true] {
            let b = SharedBudget::new(4 << 20);
            let scope = b.scope(AllocationClass::SharedBudget);
            let pair = NoiseAdmission::new_root(
                scope.clone(),
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: 1 << 20,
                    base_work: 1_000_000,
                    noise_work: 1_000_000,
                },
            )
            .unwrap();
            let p = prepare_programs_checked(&doc, Caller::Noise(&pair), &|| Ok(()), &|n| {
                pair.reserve_base_work(n)
            })
            .unwrap();
            let mut output = scope.account();
            output.resize(4).unwrap();
            let bytes = if rooted {
                render_rooted_call_checked(&doc, doc.canvas_rect(), &p, &|| Ok(()))
            } else {
                render_call_checked(&doc, doc.canvas_rect(), &p, &|| Ok(()))
            }
            .unwrap();
            assert_eq!(bytes, [pixel, pixel, pixel, 255]);
            drop(bytes);
            work.push(pair.used().unwrap());
            drop(output);
            drop(p);
            drop(pair);
            assert_eq!(scope.footprint().live, 0);
        }
        assert_eq!(work[1].base - work[0].base, extra);
        assert_eq!(work[1].noise, work[0].noise);
    }
}
#[test]
fn root_facts_reject_duplicate_or_missing_identity_and_cancel_without_unpriced_scan() {
    let mut doc = scene();
    doc.roots[1] = doc.roots[0];
    let used = Cell::new(0u64);
    let err = RootFacts::capture(&doc, &|| Ok(()), &|n| debt(&used, u64::MAX, n))
        .err()
        .unwrap();
    assert_eq!(err.code, ErrorCode::InputSchema);
    assert_eq!(used.get(), 4178);
    // Independent prefix4178 is fixed before lookup of the first invalid root.
    for mismatched_identity in [false, true] {
        let mut invalid = scene();
        if mismatched_identity {
            let id = invalid.roots[0];
            invalid.layers.get_mut(&id).unwrap().id =
                deterministic_id(&["rooted-mismatched-layer-id"]);
        } else {
            invalid.roots[0] = deterministic_id(&["rooted-missing-layer-id"]);
        }
        let used = Cell::new(0u64);
        let err = RootFacts::capture(&invalid, &|| Ok(()), &|n| debt(&used, u64::MAX, n))
            .err()
            .unwrap();
        assert_eq!(err.code, ErrorCode::InputSchema);
        assert_eq!(used.get(), 4178);
    }
    let doc = scene();
    let checks = Cell::new(0u64);
    let used = Cell::new(0u64);
    let err = RootFacts::capture(
        &doc,
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 2 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel facts"))
            } else {
                Ok(())
            }
        },
        &|n| debt(&used, u64::MAX, n),
    )
    .err()
    .unwrap();
    assert_eq!(err.code, ErrorCode::OperationCancelled);
    assert_eq!(used.get(), 4178);
}
