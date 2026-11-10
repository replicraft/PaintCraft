use super::*;
use crate::{
    blend::service::{self, Caller},
    noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    spatial::{WorkspaceAdmission, with_workspace},
};
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, LevelsRgbV1, NoiseDistribution},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack, LayerMask};
use std::cell::Cell;
fn doc(mode: Mode) -> Document {
    let mut doc = paintcraft_document::new_document(
        4,
        2,
        Some(Rgba([200, 30, 80, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = mode;
    doc
}
fn render(doc: &Document, seeded: bool, noise: bool) -> (Vec<u8>, u64, u64) {
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let run = |base: &WorkspaceAdmission, caller: Caller<'_>| -> Result<(Vec<u8>, u64, u64)> {
        let programs =
            service::prepare_programs_checked(doc, caller, &|| Ok(()), &|n| base.reserve_work(n))?;
        let pixels = if seeded {
            service::render_seeded_rooted_call_checked(doc, doc.canvas_rect(), &programs, &|| {
                Ok(())
            })?
        } else {
            service::render_rooted_call_checked(doc, doc.canvas_rect(), &programs, &|| Ok(()))?
        };
        let noise = match caller {
            Caller::Noise(n) => n.used()?.noise,
            _ => 0,
        };
        Ok((pixels, base.work_used(), noise))
    };
    let result = if noise {
        let pair = NoiseAdmission::new_root(
            scope.clone(),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 64 << 20,
                base_work: 268435456,
                noise_work: 1048576,
            },
        )
        .unwrap();
        with_noise_workspace(pair.clone(), || run(pair.workspace(), Caller::Noise(&pair))).unwrap()
    } else {
        let base = WorkspaceAdmission::new(scope.clone(), 64 << 20, 268435456);
        with_workspace(base.clone(), || run(&base, Caller::Base(&base))).unwrap()
    };
    assert_eq!(scope.footprint().live, 0);
    result
}
#[test]
fn all_modes_alpha_opacity_and_hidden_rgb_match_independent_seed_identity() {
    for mode in [Mode::Normal, Mode::Multiply, Mode::Screen] {
        for rgb in [
            [0, 0, 0],
            [255, 255, 255],
            [5, 9, 11],
            [200, 30, 80],
            [1, 128, 254],
        ] {
            for alpha in 0..=255 {
                for opacity in 0..=255 {
                    let source = Rgba([rgb[0], rgb[1], rgb[2], alpha]);
                    let a = (u32::from(alpha) * u32::from(opacity) + 127) / 255;
                    let expected = if a == 0 {
                        [0; 4]
                    } else {
                        [rgb[0], rgb[1], rgb[2], a as u8]
                    };
                    assert_eq!(compose::pixel(source, opacity), expected);
                    assert_eq!(
                        mode.composite(source.with_opacity(opacity), Rgba::TRANSPARENT)
                            .0,
                        expected
                    );
                }
            }
        }
    }
}
#[test]
fn actual_seeded_pixels_and_exact_work_remove_only_real_general_kernel() {
    for mode in [Mode::Multiply, Mode::Screen] {
        let d = doc(mode);
        let (old, old_work, _) = render(&d, false, false);
        let (new, new_work, _) = render(&d, true, false);
        assert_eq!(new, old);
        assert_eq!(new, [200, 30, 80, 128].repeat(8));
        assert_eq!(new_work, old_work + future_work(&d).unwrap() - 256 * 8);
    }
}
#[test]
fn hidden_zero_outside_partial_and_order_choose_only_first_contributor() {
    for mode in [Mode::Multiply, Mode::Screen] {
        for case in 0..5 {
            let mut d = doc(mode);
            let mut lower = d.layers[&d.roots[0]].clone();
            lower.id = deterministic_id(&["seed-lower"]);
            lower.blend_mode = Mode::Normal;
            lower.raster = paintcraft_raster::RasterLayer::new(4, 2, Rgba([20, 100, 40, 128]));
            match case {
                0 => lower.visible = false,
                1 => lower.opacity = 0,
                2 => lower.origin = (99, 0),
                3 => lower.origin = (1, 0),
                _ => {}
            }
            d.roots.insert(0, lower.id);
            d.layers.insert(lower.id, lower);
            if case == 4 {
                d.roots.swap(0, 1);
            }
            assert_eq!(render(&d, true, false).0, render(&d, false, false).0);
        }
    }
}
fn mask() -> paintcraft_raster::coverage_mask::A8Mask {
    use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
    let budget = SharedBudget::new(1 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let runs = [Run {
        x0: 0,
        x1: 4,
        coverage: 128,
    }];
    let bands = [Band {
        y0: 0,
        y1: 2,
        runs: &runs,
    }];
    A8Mask::from_bands_in_scope(
        Rect::new(0, 0, 4, 2).unwrap(),
        &bands,
        &scope,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn filter_then_filtermask_then_ownmask_then_opacity_keep_rounding() {
    for filter_mask in [false, true] {
        let mut d = doc(Mode::Multiply);
        let layer = d.layers.get_mut(&d.roots[0]).unwrap();
        let mut filter = FilterInstance::new(
            deterministic_id(&["seed-invert"]),
            true,
            Descriptor::InvertRgbV1,
        );
        if filter_mask {
            filter = filter.with_mask(Some(FilterMask::new(true, mask())));
        }
        layer.filters = FilterStack::new(vec![filter]).unwrap();
        layer.mask = Some(LayerMask::new(true, mask()));
        layer.opacity = 127;
        let expected = if filter_mask {
            [127, 128, 128, 32]
        } else {
            [55, 225, 175, 32]
        };
        let output = render(&d, true, false).0;
        assert_eq!(output, expected.repeat(8));
        assert_eq!(output, render(&d, false, false).0);
    }
}
#[test]
fn prepared_spatial_and_noise_keep_same_pixels_and_noise_counter() {
    for descriptor in [
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(25, 17, false, NoiseDistribution::Uniform).unwrap(),
        ),
    ] {
        let noise = matches!(descriptor, Descriptor::AddNoiseRgbV1(_));
        let mut d = doc(Mode::Screen);
        d.layers.get_mut(&d.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                deterministic_id(&["seed-filter"]),
                true,
                descriptor,
            )])
            .unwrap();
        let old = render(&d, false, noise);
        let new = render(&d, true, noise);
        assert_eq!(new.0, old.0);
        assert_eq!(new.2, old.2);
        if noise {
            assert!(new.2 > 0);
        }
    }
}
#[test]
fn quote_is_new_variant_and_original_general_quote_remains_exact() {
    let d = doc(Mode::Multiply);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let base = WorkspaceAdmission::new(scope, 64 << 20, 268435456);
    with_workspace(base.clone(), || {
        let old = service::rooted_call_quote_checked(
            &d,
            d.canvas_rect(),
            Caller::Base(&base),
            &|| Ok(()),
            &|n| base.reserve_work(n),
        )?;
        let new = service::seeded_rooted_call_quote_checked(
            &d,
            d.canvas_rect(),
            Caller::Base(&base),
            &|| Ok(()),
            &|n| base.reserve_work(n),
        )?;
        assert_eq!(old.call.general_samples, 8);
        assert_eq!(old.call.general_work, 2048);
        assert_eq!(new.call.general_samples, 0);
        assert_eq!(new.call.general_work, 0);
        assert_eq!(new.traversal_work, old.traversal_work + future_work(&d)?);
        assert_eq!(
            new.traversal_control_bytes,
            old.traversal_control_bytes + 2048
        );
        assert_eq!(crate::blend::GENERAL_WORK, 256);
        Ok(())
    })
    .unwrap();
}
#[test]
fn control_admission_exact_minus_one_never_creates_replacement_capacity() {
    for remaining in [2047, 2048] {
        let d = doc(Mode::Multiply);
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget);
        let base = WorkspaceAdmission::new(scope.clone(), 64 << 20, 268435456);
        let mut blocker = scope.account();
        blocker.resize((4 << 20) - remaining).unwrap();
        with_workspace(base.clone(), || {
            let before = base.work_used();
            let result = service::seeded_rooted_call_quote_checked(
                &d,
                d.canvas_rect(),
                Caller::Base(&base),
                &|| Ok(()),
                &|n| base.reserve_work(n),
            );
            if remaining == 2048 {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
                assert_eq!(base.work_used() - before, 4288);
            }
            assert_eq!(scope.footprint().live, (4 << 20) - remaining);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn malformed_private_proof_binding_and_bytes_refuse_before_output() {
    let d = doc(Mode::Multiply);
    let other = d.clone();
    let proof = Seed {
        doc: &d,
        region: d.canvas_rect(),
        index: 0,
    };
    assert_eq!(
        proof
            .require_matches(&other, d.canvas_rect())
            .unwrap_err()
            .code,
        ErrorCode::InvariantViolation
    );
    assert!(
        proof
            .require_matches(&d, Rect::new(0, 0, 1, 1).unwrap())
            .is_err()
    );
    let context = crate::blend::context(&other).unwrap();
    assert_eq!(
        crate::blend::quote::seeded_quote(&other, other.canvas_rect(), context, &proof)
            .unwrap_err()
            .code,
        ErrorCode::InvariantViolation
    );
    assert!(fresh_output(&proof, 1, &|| Ok(())).is_err());
}
#[test]
fn typed_cancel_retires_controls_and_keeps_original_cumulative_counter() {
    let d = doc(Mode::Multiply);
    let budget = SharedBudget::new(8 << 20);
    let scope = budget
        .reserve_capacity(4 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let base = WorkspaceAdmission::new(scope.clone(), 64 << 20, 268435456);
    let checks = Cell::new(0);
    with_workspace(base.clone(), || {
        let p = service::prepare_programs_checked(&d, Caller::Base(&base), &|| Ok(()), &|n| {
            base.reserve_work(n)
        })?;
        let before = base.work_used();
        let error = service::render_seeded_rooted_call_checked(&d, d.canvas_rect(), &p, &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 5 {
                Err(Error::new(ErrorCode::OperationCancelled, "seed original").at(41))
            } else {
                Ok(())
            }
        })
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.operation_index, Some(41));
        assert_eq!(error.message, "seed original");
        assert!(base.work_used() >= before);
        drop(p);
        Ok(())
    })
    .unwrap();
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn empty_normal_prefix_preserves_hidden_rgb_zero_and_seeds_actual_general() {
    for mode in [Mode::Multiply, Mode::Screen] {
        let mut d = doc(mode);
        let mut lower = d.layers[&d.roots[0]].clone();
        lower.id = deterministic_id(&["seed-empty-normal"]);
        lower.blend_mode = Mode::Normal;
        lower.raster = paintcraft_raster::RasterLayer::new(4, 2, Rgba([7, 9, 11, 0]));
        d.roots.insert(0, lower.id);
        d.layers.insert(lower.id, lower);
        assert_eq!(render(&d, true, false).0, [200, 30, 80, 128].repeat(8));
        assert_eq!(render(&d, true, false).0, render(&d, false, false).0);
    }
}

#[test]
fn actual_resident_dense_row_seeds_hidden_zero_and_fractional_opacity() {
    use paintcraft_raster::{Admission, ReservedDecodeFactories};
    for mode in [Mode::Multiply, Mode::Screen] {
        let mut d = paintcraft_document::new_document(
            2,
            1,
            Some(Rgba::TRANSPARENT),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let budget = SharedBudget::new(8 << 20);
        let c = budget
            .reserve_capacity(4096)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let m = budget
            .reserve_capacity(65536)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let q = budget.reserve_capacity(262144).unwrap();
        let factories = ReservedDecodeFactories {
            selection_budget: budget.clone(),
            coverage_budget: budget.clone(),
            paint_metadata_scope: m.clone(),
        };
        let mut admission =
            Admission::with_reserved_decode_factories(65536, q, factories, &c, &|| Ok(())).unwrap();
        let layer = d.layers.get_mut(&d.roots[0]).unwrap();
        layer.blend_mode = mode;
        layer.opacity = 128;
        admission
            .write_block(
                &mut layer.raster,
                0,
                0,
                &[7, 9, 11, 0, 200, 30, 80, 128],
                &|| Ok(()),
            )
            .unwrap();
        assert_eq!(layer.raster.resident_tiles(), 1);
        let output = render(&d, true, false).0;
        assert_eq!(output, [0, 0, 0, 0, 200, 30, 80, 64]);
        assert_eq!(output, render(&d, false, false).0);
    }
}
