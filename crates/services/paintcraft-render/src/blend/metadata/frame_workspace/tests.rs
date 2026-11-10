use super::*;
use crate::{noise_work, spatial};
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode, Rgba,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, LevelsRgbV1, NoiseDistribution},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::cell::Cell;

fn blur(r: i16) -> Descriptor {
    Descriptor::BoxBlurV1(BoxBlurV1::new(r).unwrap())
}
fn noise() -> Descriptor {
    Descriptor::AddNoiseRgbV1(
        AddNoiseRgbV1::new(100, 17, false, NoiseDistribution::Uniform).unwrap(),
    )
}
fn scene(w: u32, h: u32, policies: &[(bool, Descriptor)]) -> Document {
    let mut doc = paintcraft_document::new_document(
        w,
        h,
        Some(Rgba([7, 9, 11, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    layer.filters = FilterStack::new(
        policies
            .iter()
            .enumerate()
            .map(|(i, (enabled, value))| {
                FilterInstance::new(
                    deterministic_id(&["universal-workspace", &i.to_string()]),
                    *enabled,
                    *value,
                )
            })
            .collect(),
    )
    .unwrap();
    doc
}
fn forecast(doc: &Document) -> UnboundRootedFrameWorkspace {
    quote_unbound_rooted_frame_workspace_checked(doc, &|| Ok(()), &|_| Ok(())).unwrap()
}
fn layer(doc: &Document) -> &Layer {
    &doc.layers[&doc.roots[0]]
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn stage_bytes() -> u64 {
    if cfg!(target_pointer_width = "64") {
        240
    } else {
        208
    }
}
#[test]
fn independent_byte_components_cover_zero_radius_odd_margins_and_composed_halos() {
    // Frozen independent recurrence: front+back8*P, scratch8*S, Stage240/208.
    for (w, h, radii, pixels, scratch, base) in [
        (64, 64, vec![0], 4096, 0, 32768),
        (100, 101, vec![1], 4356, 16896, 170016),
        (65, 1, vec![1], 65, 256, 2568),
        (100, 101, vec![1, 2], 4900, 19040, 191520),
        (64, 64, vec![0, 0], 4096, 0, 32768),
    ] {
        let filters = radii.iter().map(|r| (true, blur(*r))).collect::<Vec<_>>();
        let doc = scene(w, h, &filters);
        let plan =
            LayerAppearancePlan::new(layer(&doc), centered(layer(&doc), 64).unwrap(), &|| Ok(()))
                .unwrap();
        assert_eq!(plan.quote().pixels_per_buffer, pixels);
        assert_eq!(plan.quote().scratch_elements, scratch);
        assert_eq!(
            plan.quote().bytes,
            base + radii.len() as u64 * stage_bytes()
        );
        let q = forecast(&doc);
        assert_eq!(q.spatial_workspace_bytes, plan.quote().bytes);
        assert_eq!(
            q.native_rgba_bytes,
            u64::from(w.min(64)) * u64::from(h.min(64)) * 4
        );
        assert_eq!(q.display_accumulator_bytes, q.display_cells * 40);
        assert_eq!(q.query_control_bytes, 32768);
    }
}
#[test]
fn actual_small_domain_plans_are_bounded_for_every_smaller_offcenter_roi() {
    for w in 1..=6 {
        for h in 1..=6 {
            for radii in [[0, 0], [0, 2], [1, 2], [2, 1]] {
                let doc = scene(w, h, &[(true, blur(radii[0])), (true, blur(radii[1]))]);
                let l = layer(&doc);
                let max = LayerAppearancePlan::new(l, centered(l, 3).unwrap(), &|| Ok(()))
                    .unwrap()
                    .quote();
                for rw in 1..=w.min(3) {
                    for rh in 1..=h.min(3) {
                        for x in 0..=w - rw {
                            for y in 0..=h - rh {
                                let q = LayerAppearancePlan::new(
                                    l,
                                    rect(x.into(), y.into(), rw.into(), rh.into()),
                                    &|| Ok(()),
                                )
                                .unwrap()
                                .quote();
                                assert!(
                                    q.bytes <= max.bytes,
                                    "{w}x{h} {radii:?} {x},{y} {rw}x{rh}"
                                );
                                assert!(q.pixels_per_buffer <= max.pixels_per_buffer);
                                assert!(q.scratch_elements <= max.scratch_elements);
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn point_noise_masks_and_disabled_filters_preserve_geometry() {
    let doc = scene(100, 101, &[(true, blur(1))]);
    let plain = forecast(&doc);
    let mut mixed = scene(
        100,
        101,
        &[
            (true, Descriptor::InvertRgbV1),
            (true, blur(1)),
            (true, noise()),
            (false, blur(32)),
        ],
    );
    let q = forecast(&mixed);
    assert_eq!(
        q.spatial_workspace_bytes,
        plain.spatial_workspace_bytes + 2 * stage_bytes()
    );
    assert!(q.requires_noise);
    assert_eq!(
        q.noise_sidecar_bytes,
        NoiseAdmission::control_quote().unwrap().counter_bytes
    );
    let budget = SharedBudget::new(8192);
    let mask = FilterMask::new(
        true,
        A8Mask::uniform(
            rect(0, 0, 100, 101),
            128,
            &budget,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    );
    let l = mixed.layers.get_mut(&mixed.roots[0]).unwrap();
    l.filters = FilterStack::new(
        l.filters
            .iter()
            .map(|f| f.with_mask(Some(mask.clone())))
            .collect(),
    )
    .unwrap();
    assert_eq!(
        forecast(&mixed).spatial_workspace_bytes,
        q.spatial_workspace_bytes
    );
    let disabled = scene(100, 101, &[(false, blur(32)), (false, noise())]);
    let q = forecast(&disabled);
    assert_eq!(q.spatial_workspace_bytes, 0);
    assert!(q.requires_noise);
    assert!(q.noise_sidecar_bytes > 0);
}
#[test]
fn hidden_and_zero_opacity_spatial_layers_do_not_create_a_live_workspace_peak() {
    for hidden in [true, false] {
        let mut doc = scene(100, 101, &[(true, blur(32))]);
        let l = doc.layers.get_mut(&doc.roots[0]).unwrap();
        if hidden {
            l.visible = false;
        } else {
            l.opacity = 0;
        }
        let q = forecast(&doc);
        assert_eq!(q.spatial_workspace_bytes, 0);
        assert_eq!(
            q.workspace_bytes,
            q.blend_context_bytes + q.rooted_witness_bytes + 2048
        );
    }
}
#[test]
fn fresh_preparation_and_serial_render_peaks_coexist_as_declared_not_summed() {
    let doc = scene(
        100,
        101,
        &[
            (true, Descriptor::LevelsRgbV1(LevelsRgbV1::default())),
            (true, blur(2)),
        ],
    );
    let q = forecast(&doc);
    let old = quote_unbound_programs_checked(&doc, &|| Ok(()), &|_| Ok(())).unwrap();
    assert_eq!(q.prepared_retained_bytes, old.numerical.retained_bytes);
    assert_eq!(
        q.preparation_peak_bytes,
        old.numerical.preparation_peak_bytes
    );
    assert!(q.prepared_retained_bytes > 0);
    let render = q.prepared_retained_bytes
        + q.spatial_workspace_bytes
        + q.blend_context_bytes
        + q.rooted_witness_bytes
        + q.seed_witness_bytes;
    assert_eq!(q.workspace_bytes, q.preparation_peak_bytes.max(render));
    assert_eq!(q.seed_witness_bytes, 2048);
    assert!(spatial::current().is_none());
    assert!(noise_work::capture_current_noise_workspace().is_none());
}
#[test]
fn original_uninstalled_counter_keeps_exact_formation_cut_and_paid_retry() {
    let doc = scene(1, 1, &[]);
    let budget = SharedBudget::new(1 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let base = spatial::WorkspaceAdmission::new(scope.clone(), 1 << 20, 82944);
    let before = budget.usage();
    let activity = budget.activity();
    let footprint = scope.footprint();
    let checks = Cell::new(0);
    let check = || {
        checks.set(checks.get() + 1);
        Ok(())
    };
    let err = quote_unbound_rooted_frame_workspace_checked(&doc, &check, &|n| base.reserve_work(n))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert_eq!(base.work_used(), 82944); //81920 formation+1024 first raw check.
    assert_eq!(checks.get(), 1);
    assert!(
        quote_unbound_rooted_frame_workspace_checked(&doc, &check, &|n| base.reserve_work(n))
            .is_err()
    );
    assert_eq!(base.work_used(), 82944);
    assert_eq!(checks.get(), 1);
    assert_eq!(budget.usage(), before);
    assert_eq!(budget.activity(), activity);
    assert_eq!(scope.footprint(), footprint);
    let small = spatial::WorkspaceAdmission::new(scope, 1 << 20, 81919);
    assert!(
        quote_unbound_rooted_frame_workspace_checked(&doc, &check, &|n| small.reserve_work(n))
            .is_err()
    );
    assert_eq!(small.work_used(), 0);
}
#[test]
fn successful_query_spends_only_the_original_counter_and_preserves_source_and_factories() {
    let doc = scene(65, 1, &[(true, blur(1))]);
    let original_doc = doc.clone();
    let budget = SharedBudget::new(1 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let base = spatial::WorkspaceAdmission::new(scope.clone(), 1 << 20, 268435456);
    base.reserve_work(17).unwrap();
    let before = (budget.usage(), budget.activity(), scope.footprint());
    let q =
        quote_unbound_rooted_frame_workspace_checked(&doc, &|| Ok(()), &|n| base.reserve_work(n))
            .unwrap();
    assert!(q.workspace_bytes > 0);
    assert!(base.work_used() > 81920 + 17);
    assert_eq!(
        (budget.usage(), budget.activity(), scope.footprint()),
        before
    );
    assert!(doc.same_semantics(&original_doc));
    assert!(spatial::current().is_none());
    assert!(noise_work::capture_current_noise_workspace().is_none());
}
#[test]
fn original_arbitrary_callback_error_identity_is_not_replaced() {
    let doc = scene(1, 1, &[]);
    let value = Error::new(
        ErrorCode::OperationCancelled,
        String::from("original Main callback owned message"),
    )
    .at(23);
    let pointer = value.message.as_ptr();
    let error = std::cell::RefCell::new(Some(value));
    let paid = Cell::new(0);
    let e = quote_unbound_rooted_frame_workspace_checked(
        &doc,
        &|| Err(error.borrow_mut().take().unwrap()),
        &|n| {
            paid.set(paid.get() + n);
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(e.message.as_ptr(), pointer);
    assert_eq!(e.operation_index, Some(23));
    assert_eq!(paid.get(), 82944);
}
#[test]
fn installed_base_or_noise_refuses_before_callbacks_without_renewal() {
    let doc = scene(1, 1, &[]);
    let budget = SharedBudget::new(1 << 20);
    let callback = || -> Result<()> { panic!("must refuse before callback") };
    let spend = |_| -> Result<()> { panic!("must refuse before spend") };
    let base =
        spatial::WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 1 << 20, 100);
    spatial::with_workspace(base.clone(), || {
        let e = quote_unbound_rooted_frame_workspace_checked(&doc, &callback, &spend).unwrap_err();
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        Ok(())
    })
    .unwrap();
    assert_eq!(base.work_used(), 0);
    let noise = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        noise_work::NoiseJobPolicy::BrowserV1,
        noise_work::NoiseLimits {
            bytes: 1 << 20,
            base_work: 100,
            noise_work: 100,
        },
    )
    .unwrap();
    noise_work::with_noise_workspace(noise.clone(), || {
        assert!(quote_unbound_rooted_frame_workspace_checked(&doc, &callback, &spend).is_err());
        Ok(())
    })
    .unwrap();
    assert_eq!(noise.workspace().work_used(), 0);
}
#[test]
fn duplicate_root_and_missing_nonnormal_are_typed_refusals() {
    let mut doc = scene(1, 1, &[]);
    doc.roots.push(doc.roots[0]);
    assert_eq!(
        quote_unbound_rooted_frame_workspace_checked(&doc, &|| Ok(()), &|_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let mut doc = scene(1, 1, &[]);
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Normal;
    assert!(quote_unbound_rooted_frame_workspace_checked(&doc, &|| Ok(()), &|_| Ok(())).is_err());
}

#[test]
fn maximum_stack_is_source_bounded_without_full_canvas_or_coordinate_sampling() {
    let policies = vec![(true, blur(32)); 256];
    let doc = scene(65, 67, &policies);
    let q = forecast(&doc);
    // Once halo reaches the finite domain, both buffers stop growing.
    // Scratch peaks at full65*67*4; all256 stored stages remain present.
    assert_eq!(
        q.spatial_workspace_bytes,
        8 * 65 * 67 + 8 * 4 * 65 * 67 + 256 * stage_bytes()
    );
    assert_eq!(q.native_rgba_bytes, 16384);
    assert_eq!(q.display_accumulator_bytes, 163840);
}
