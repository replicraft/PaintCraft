use super::*;
use crate::spatial::{WorkspaceAdmission, with_workspace};
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, NoiseDistribution},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack};
fn limits(base: u64, noise: u64) -> NoiseLimits {
    NoiseLimits {
        bytes: 1 << 20,
        base_work: base,
        noise_work: noise,
    }
}
fn root(b: &SharedBudget, base: u64, noise: u64) -> NoiseAdmission {
    NoiseAdmission::new_root(
        b.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        limits(base, noise),
    )
    .unwrap()
}
fn scene(blur: bool) -> Document {
    let mut d = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut f = vec![FilterInstance::new(
        paintcraft_core::types::ids::deterministic_id(&["noise-work-test", "1"]),
        true,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(100, 65535, false, NoiseDistribution::Uniform).unwrap(),
        ),
    )];
    if blur {
        f.push(FilterInstance::new(
            paintcraft_core::types::ids::deterministic_id(&["noise-work-test", "2"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ));
    }
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(f).unwrap();
    d
}
#[test]
fn paired_denials_are_atomic_and_each_component_has_its_own_ceiling() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 3, 5);
    a.reserve_components(WorkComponents { base: 1, noise: 2 })
        .unwrap();
    for delta in [
        WorkComponents { base: 3, noise: 1 },
        WorkComponents { base: 1, noise: 4 },
    ] {
        assert_eq!(
            a.reserve_components(delta).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(a.used().unwrap(), WorkComponents { base: 1, noise: 2 });
    }
    a.reserve_components(WorkComponents { base: 2, noise: 3 })
        .unwrap();
    assert_eq!(a.used().unwrap(), WorkComponents { base: 3, noise: 5 });
    assert_eq!(a.workspace().work_used(), 3);
}
#[test]
fn external_base_debits_keep_the_pair_and_respect_a_depleted_child_noise_ceiling() {
    let budget = SharedBudget::new(1 << 20);
    let parent = root(&budget, 4, 3);
    let child = parent.child(limits(3, 2)).unwrap();
    parent
        .reserve_components(WorkComponents { base: 0, noise: 3 })
        .unwrap();
    assert!(parent.same_job(&child));
    assert_eq!(
        child.reserve_base_work(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(parent.used().unwrap(), WorkComponents { base: 0, noise: 3 });
    parent.reserve_base_work(1).unwrap();
    assert_eq!(child.used().unwrap(), WorkComponents { base: 1, noise: 3 });
    assert!(parent.reserve_base_work(4).is_err());
    assert_eq!(parent.used().unwrap(), WorkComponents { base: 1, noise: 3 });
}
#[test]
fn child_limits_identity_and_prior_usage_cannot_reset_or_enlarge_either_counter() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 20, 30);
    a.reserve_components(WorkComponents { base: 4, noise: 6 })
        .unwrap();
    assert!(a.child(limits(3, 30)).is_err());
    assert!(a.child(limits(20, 5)).is_err());
    let mut no_bytes = limits(8, 9);
    no_bytes.bytes = 0;
    assert!(a.child(no_bytes).is_err());
    let c = a.child(limits(8, 9)).unwrap();
    assert!(a.same_job(&c));
    assert_eq!(c.limits(), limits(8, 9));
    c.reserve_components(WorkComponents { base: 4, noise: 3 })
        .unwrap();
    assert_eq!(a.used().unwrap(), WorkComponents { base: 8, noise: 9 });
    assert!(
        c.reserve_components(WorkComponents { base: 0, noise: 1 })
            .is_err()
    );
    a.reserve_components(WorkComponents { base: 1, noise: 1 })
        .unwrap();
    assert!(c.check_available(WorkComponents::default()).is_err());
    assert_eq!(a.used().unwrap().checked_total().unwrap(), 19);
}
#[test]
fn concurrent_clones_share_one_cumulative_pair_and_do_not_double_or_lose_debits() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 800, 1600);
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let a = a.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    a.reserve_components(WorkComponents { base: 1, noise: 2 })
                        .unwrap();
                }
            })
        })
        .collect();
    for t in tasks {
        t.join().unwrap();
    }
    assert_eq!(
        a.used().unwrap(),
        WorkComponents {
            base: 800,
            noise: 1600
        }
    );
    assert!(
        a.reserve_components(WorkComponents { base: 1, noise: 0 })
            .is_err()
    );
    assert!(
        a.reserve_components(WorkComponents { base: 0, noise: 1 })
            .is_err()
    );
}
#[test]
fn constructor_and_capture_never_upgrade_an_installed_legacy_parent_or_call_fallback() {
    let b = SharedBudget::new(1 << 20);
    let legacy = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 100);
    let called = std::cell::Cell::new(false);
    with_workspace(legacy, || {
        assert_eq!(
            NoiseAdmission::new_root(
                b.scope(AllocationClass::SharedBudget),
                NoiseJobPolicy::BrowserV1,
                limits(10, 10)
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        let e = capture_noise_or_current(
            || {
                called.set(true);
                Ok(root(&b, 10, 10))
            },
            limits(10, 10),
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        Ok(())
    })
    .unwrap();
    assert!(!called.get());
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn unrelated_install_is_rejected_while_same_owner_nesting_tightens_and_restores() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 100, 100);
    let other = root(&b, 100, 100);
    with_noise_workspace(a.clone(), || {
        assert_eq!(
            with_noise_workspace(other.clone(), || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        with_noise_workspace(a.child(limits(5, 6))?, || {
            let active = required()?;
            assert_eq!(active.limits(), limits(5, 6));
            active.reserve_components(WorkComponents { base: 2, noise: 3 })?;
            Ok(())
        })?;
        assert_eq!(required()?.limits(), limits(100, 100));
        let base = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 100);
        with_workspace(base, || {
            assert!(required().is_err());
            Ok(())
        })?;
        Ok(())
    })
    .unwrap();
    assert!(current().is_none());
    assert!(crate::spatial::current().is_none());
}
#[test]
fn capture_is_lazy_retains_prior_work_and_survives_submitter_scope_exit() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 20, 30);
    let captured = with_noise_workspace(a.clone(), || {
        required()?.reserve_components(WorkComponents { base: 3, noise: 5 })?;
        capture_noise_or_current(|| panic!("must not mint another owner"), limits(10, 12))
    })
    .unwrap();
    assert!(a.same_job(&captured));
    assert_eq!(
        captured.used().unwrap(),
        WorkComponents { base: 3, noise: 5 }
    );
    let t = std::thread::spawn(move || {
        with_noise_workspace(captured, || {
            required()?.reserve_components(WorkComponents { base: 2, noise: 4 })
        })
    });
    t.join().unwrap().unwrap();
    assert_eq!(a.used().unwrap(), WorkComponents { base: 5, noise: 9 });
    let called = std::cell::Cell::new(0);
    let fresh = capture_noise_or_current(
        || {
            called.set(called.get() + 1);
            Ok(root(&b, 10, 10))
        },
        limits(8, 9),
    )
    .unwrap();
    assert_eq!(called.get(), 1);
    assert!(!a.same_job(&fresh));
}
#[test]
fn errors_panics_and_captured_absence_restore_both_contexts() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 10, 20);
    let error = with_noise_workspace(a.clone(), || -> Result<()> { Err(limit()) }).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_noise_workspace(a.clone(), || -> Result<()> {
            panic!("expected test unwind")
        })
    }));
    assert!(unwind.is_err());
    assert!(current().is_none());
    assert!(crate::spatial::current().is_none());
    with_noise_workspace(a, || {
        captured(&None, || {
            assert!(current().is_none());
            assert!(crate::spatial::current().is_none());
            Ok(())
        })?;
        assert!(required().is_ok());
        Ok(())
    })
    .unwrap();
}
#[test]
fn control_storage_is_same_abi_charged_before_allocation_and_released_by_last_owner() {
    let q = NoiseAdmission::control_quote().unwrap();
    assert_eq!(q.handle_bytes, std::mem::size_of::<NoiseAdmission>() as u64);
    assert!(q.counter_bytes >= super::admission::counter_payload_bytes());
    let tiny = SharedBudget::new(q.counter_bytes - 1);
    assert!(
        NoiseAdmission::new_root(
            tiny.scope(AllocationClass::SharedBudget),
            NoiseJobPolicy::BrowserV1,
            limits(10, 20)
        )
        .is_err()
    );
    assert_eq!(tiny.usage().reserved, 0);
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 10, 20);
    assert_eq!(b.usage().reserved, q.counter_bytes);
    let clone = a.clone();
    drop(a);
    assert_eq!(b.usage().reserved, q.counter_bytes);
    drop(clone);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn quote_only_does_not_allocate_compile_spend_or_create_an_admission() {
    let d = scene(true);
    let b = SharedBudget::new(1 << 20);
    for _ in 0..3 {
        let q = quote_layer(&d.layers[&d.roots[0]], d.canvas_rect(), &|| Ok(())).unwrap();
        assert!(q.work.noise > 0);
    }
    assert!(current().is_none());
    let a = root(&b, 268_435_456, NOISE_WORK_MAX);
    let live = b.usage().reserved;
    let activity = b.activity();
    let used = a.used().unwrap();
    with_noise_workspace(a.clone(), || {
        for _ in 0..3 {
            let q = quote_layer(&d.layers[&d.roots[0]], d.canvas_rect(), &|| Ok(()))?;
            a.check_available(q.work)?;
            a.validate_document(&d, &|| Ok(()))?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(b.usage().reserved, live);
    assert_eq!(b.activity(), activity);
    assert_eq!(a.used().unwrap(), used);
    let programs = crate::prepared::quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(programs.unique_programs, 0);
}
#[test]
fn trusted_split_matches_original_total_and_spatial_noise_is_debited_once() {
    let d = scene(true);
    let layer = &d.layers[&d.roots[0]];
    let q = quote_layer(layer, d.canvas_rect(), &|| Ok(())).unwrap();
    let old = crate::spatial::quote_layer(layer, d.canvas_rect(), &|| Ok(())).unwrap();
    assert_eq!(q.work.checked_total().unwrap(), old.work);
    assert_eq!(q.bytes, old.bytes);
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, q.work.base, q.work.noise);
    let image = with_noise_workspace(a.clone(), || crate::render_flat(&d)).unwrap();
    assert_eq!(image.len(), 8);
    assert_eq!(a.used().unwrap(), q.work);
    assert!(with_noise_workspace(a.clone(), || crate::render_flat(&d)).is_err());
    assert_eq!(a.used().unwrap(), q.work);
}
#[test]
fn nonspatial_chunks_and_parallel_partitions_do_not_spend_noise_twice() {
    let d = scene(false);
    let b = SharedBudget::new(1 << 20);
    for partition in [
        crate::parallel::Partition::Bands { rows: 1 },
        crate::parallel::Partition::Tiles { edge: 1 },
    ] {
        let a = root(&b, 1000, 2048);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let image = with_noise_workspace(a.clone(), || {
            crate::parallel::render_in_pool(&pool, &d, d.canvas_rect(), partition, 1 << 20, || {
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(image, [255, 211, 0, 255, 51, 0, 97, 255]);
        assert_eq!(
            a.used().unwrap(),
            WorkComponents {
                base: 0,
                noise: 1024
            }
        );
    }
}
#[test]
fn checked_components_overflow_and_policy_caps_are_closed() {
    assert!(
        WorkComponents {
            base: u64::MAX,
            noise: 1
        }
        .checked_total()
        .is_err()
    );
    assert!(
        WorkComponents { base: 1, noise: 1 }
            .checked_mul(u64::MAX)
            .is_err()
    );
    let b = SharedBudget::new(1 << 20);
    for l in [limits(268_435_457, 0), limits(0, NOISE_WORK_MAX + 1)] {
        assert!(
            NoiseAdmission::new_root(
                b.scope(AllocationClass::SharedBudget),
                NoiseJobPolicy::BrowserV1,
                l
            )
            .is_err()
        );
    }
    assert_eq!(b.usage().reserved, 0);
    assert!(
        NoiseAdmission::new_root(
            b.scope(AllocationClass::SharedBudget),
            NoiseJobPolicy::NativeV1,
            limits(1_000_000_000, NOISE_WORK_MAX)
        )
        .is_ok()
    );
    assert_eq!(b.usage().reserved, 0);
}

#[test]
fn pure_policy_validation_and_control_quote_do_not_create_or_spend_a_job() {
    assert_eq!(
        NoiseJobPolicy::BrowserV1.max_limits(),
        NoiseLimits {
            bytes: 64 << 20,
            base_work: 268_435_456,
            noise_work: 1 << 35
        }
    );
    assert_eq!(
        NoiseJobPolicy::NativeV1.max_limits().base_work,
        1_000_000_000
    );
    assert_eq!(WORKLOAD_CLASS_ID, "noise-source-work-v1");
    assert_eq!(WORKLOAD_CLASS_REVISION, 1);
    for _ in 0..10 {
        NoiseJobPolicy::BrowserV1
            .validate(limits(100, 200))
            .unwrap();
        NoiseAdmission::control_quote().unwrap();
    }
    assert!(current().is_none());
    assert!(crate::spatial::current().is_none());
    assert!(
        NoiseJobPolicy::BrowserV1
            .validate(limits(268_435_457, 0))
            .is_err()
    );
}
#[test]
fn prepared_owner_rebinding_keeps_pair_and_quote_only_missing_keys_never_compile() {
    use paintcraft_core::types::filters::LevelsRgbV1;
    let mut doc = scene(false);
    let id = doc.roots[0];
    let noise = doc.layers[&id].filters.as_slice()[0].clone();
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
        noise.clone(),
        FilterInstance::new(
            paintcraft_core::types::ids::deterministic_id(&["noise-work-test", "2"]),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        ),
    ])
    .unwrap();
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 268_435_456, 1 << 20);
    a.validate_document(&doc, &|| Ok(())).unwrap();
    assert_eq!(a.used().unwrap(), WorkComponents::default());
    let frozen = a.prepare_document(&doc, &|| Ok(())).unwrap();
    assert!(a.same_job(&frozen));
    assert!(frozen.workspace().has_prepared_programs());
    let used = frozen.used().unwrap();
    assert!(used.base > 0);
    let storage = b.usage().reserved;
    frozen.validate_document(&doc, &|| Ok(())).unwrap();
    let again = frozen.prepare_document(&doc, &|| Ok(())).unwrap();
    assert!(frozen.same_job(&again));
    assert_eq!(again.used().unwrap(), used);
    assert_eq!(b.usage().reserved, storage);
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
        noise,
        FilterInstance::new(
            paintcraft_core::types::ids::deterministic_id(&["noise-work-test", "2"]),
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap()),
        ),
    ])
    .unwrap();
    assert_eq!(
        frozen.validate_document(&doc, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(frozen.used().unwrap(), used);
    assert_eq!(b.usage().reserved, storage);
}

#[test]
fn invalid_source_quote_geometry_refuses_before_callbacks_without_wrapping() {
    let doc = scene(false);
    let layer = &doc.layers[&doc.roots[0]];
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    for rect in [
        Rect {
            x0: i64::MIN,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: -1,
            x1: 1,
            y1: 1,
        },
        Rect {
            x0: 0,
            y0: 0,
            x1: 3,
            y1: 1,
        },
    ] {
        assert_eq!(
            quote_layer(layer, rect, &check).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            quote_direct(layer, rect, &check).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn actual_render_cannot_exchange_disabled_noise_credit_for_base_or_base_for_noise() {
    let b = SharedBudget::new(1 << 20);
    for enabled in [false, true] {
        let mut doc = scene(true);
        let id = doc.roots[0];
        let mut f = doc.layers[&id].filters.as_slice().to_vec();
        f[0] = f[0].with_enabled(enabled);
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(f).unwrap();
        let before = doc.clone();
        let a = root(&b, 1, NOISE_WORK_MAX);
        assert_eq!(
            with_noise_workspace(a.clone(), || crate::render_flat(&doc))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(a.used().unwrap(), WorkComponents::default());
        assert!(doc.same_semantics(&before));
    }
    let doc = scene(false);
    let a = root(&b, 268_435_456, 1);
    assert_eq!(
        with_noise_workspace(a.clone(), || crate::render_flat(&doc))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.used().unwrap(), WorkComponents::default());
}
#[test]
fn approved_default_and_1080p_noise_quotes_fit_without_changing_base_ceiling() {
    for (w, h) in [(1000, 486), (1920, 1080), (2000, 2000)] {
        for distribution in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
            let mut doc = paintcraft_document::new_document(
                w,
                h,
                Some(Rgba([64, 16, 128, 255])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![FilterInstance::new(
                    paintcraft_core::types::ids::deterministic_id(&["noise-work-test", "1"]),
                    true,
                    Descriptor::AddNoiseRgbV1(
                        AddNoiseRgbV1::new(10, 1, true, distribution).unwrap(),
                    ),
                )])
                .unwrap();
            let q = quote_layer(&doc.layers[&doc.roots[0]], doc.canvas_rect(), &|| Ok(()))
                .unwrap()
                .work
                .checked_mul(2)
                .unwrap();
            assert!(q.base <= NoiseJobPolicy::BrowserV1.max_limits().base_work);
            assert!(q.noise <= NoiseJobPolicy::BrowserV1.max_limits().noise_work);
            assert_eq!(
                q.noise,
                paintcraft_effects::noise::quote(
                    AddNoiseRgbV1::new(10, 1, true, distribution).unwrap(),
                    (w as usize) * (h as usize)
                )
                .unwrap()
                .work
                    * 2
            );
            assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
        }
    }
    // Admission arithmetic only: no measured latency, pixels or 4MP memory claim.
}

#[test]
fn escaped_legacy_base_keeps_only_legacy_authority_after_sidecar_release() {
    let b = SharedBudget::new(1 << 20);
    let a = root(&b, 1000, 10000);
    let base = a.workspace().clone();
    assert!(b.usage().reserved > 0);
    drop(a);
    assert_eq!(b.usage().reserved, 0);
    let doc = scene(false);
    assert_eq!(
        with_workspace(base, || crate::render_flat(&doc))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}

#[test]
fn release_callback_sees_both_arc_allocations_gone_and_can_readmit_full_charge() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let q = NoiseAdmission::control_quote().unwrap();
    let b = SharedBudget::new(q.counter_bytes);
    let a = root(&b, 10, 20);
    let clone = a.clone();
    let counter = Arc::new(AtomicBool::new(false));
    let charge = Arc::new(AtomicBool::new(false));
    let _observation = super::owner::observation::install(counter.clone(), charge.clone());
    let wakes = Arc::new(AtomicUsize::new(0));
    let seen = wakes.clone();
    let budget = b.clone();
    let c = counter.clone();
    let h = charge.clone();
    let watch = b
        .watch(1, move || {
            if budget.usage().reserved == 0 {
                assert!(
                    c.load(Ordering::Acquire),
                    "counter allocation still alive at release"
                );
                assert!(
                    h.load(Ordering::Acquire),
                    "charge-owner allocation still alive at release"
                );
                let reentrant = budget.reserve(q.counter_bytes).unwrap();
                seen.fetch_add(1, Ordering::AcqRel);
                drop(reentrant);
            }
        })
        .unwrap();
    watch.observe();
    watch.wait_for_release(true);
    drop(a);
    assert_eq!(b.usage().reserved, q.counter_bytes);
    assert!(!counter.load(Ordering::Acquire));
    assert!(!charge.load(Ordering::Acquire));
    drop(clone);
    assert!(counter.load(Ordering::Acquire));
    assert!(charge.load(Ordering::Acquire));
    assert_eq!(wakes.load(Ordering::Acquire), 1);
    assert_eq!(b.usage().reserved, 0);
}

#[test]
fn concurrent_last_owner_drops_release_storage_before_budget_reentry_exactly_once() {
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    for _ in 0..16 {
        let q = NoiseAdmission::control_quote().unwrap();
        let b = SharedBudget::new(q.counter_bytes);
        let a = root(&b, 10, 20);
        let sibling = a.clone();
        let counter = Arc::new(AtomicBool::new(false));
        let charge = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let budget = b.clone();
        let c = counter.clone();
        let h = charge.clone();
        let watch = b
            .watch(1, move || {
                if budget.usage().reserved == 0 {
                    assert!(c.load(Ordering::Acquire));
                    assert!(h.load(Ordering::Acquire));
                    let lease = budget.reserve(q.counter_bytes).unwrap();
                    seen.fetch_add(1, Ordering::AcqRel);
                    drop(lease);
                }
            })
            .unwrap();
        watch.observe();
        watch.wait_for_release(true);
        let barrier = Arc::new(Barrier::new(3));
        let tasks: Vec<_> = [a, sibling]
            .into_iter()
            .map(|owner| {
                let b = barrier.clone();
                let c = counter.clone();
                let h = charge.clone();
                std::thread::spawn(move || {
                    let _observation = super::owner::observation::install(c, h);
                    b.wait();
                    drop(owner);
                })
            })
            .collect();
        barrier.wait();
        for task in tasks {
            task.join().unwrap();
        }
        assert_eq!(calls.load(Ordering::Acquire), 1);
        assert_eq!(b.usage().reserved, 0);
        assert!(counter.load(Ordering::Acquire));
        assert!(charge.load(Ordering::Acquire));
    }
}
