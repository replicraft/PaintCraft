//! Ordered whole-RGB HSL and captured direct-work admission controls.
use crate::spatial::{LayerAppearancePlan, WorkspaceAdmission, with_workspace};
use crate::*;
use paintcraft_core::types::{
    filters::{BoxBlurV1, Descriptor, HueSaturationRgbV1, VibranceRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{FilterInstance, FilterStack};
fn h() -> Descriptor {
    Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::new(60, 100, 10).unwrap())
}
fn scene(filters: Vec<Descriptor>) -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[20, 115, 200, 255, 64, 16, 128, 255],
        "direct-hsl",
    )
    .unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(
        filters
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                FilterInstance::new(deterministic_id(&["hsl-direct", &i.to_string()]), true, p)
            })
            .collect(),
    )
    .unwrap();
    d
}
fn factory(b: &SharedBudget, work: u64) -> WorkspaceAdmission {
    WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, work)
}
#[test]
fn tuple_policy_is_lookup_free_and_charges_actual_same_parent128() {
    let d = scene(vec![h()]);
    let q = prepared::quote_document_programs(&d, &|| Ok(())).unwrap();
    assert_eq!(
        (
            q.unique_programs,
            q.compile_work,
            q.retained_bytes,
            q.preparation_peak_bytes
        ),
        (0, 0, 0, 0)
    );
    let region = d.canvas_rect();
    let layer = &d.layers[&d.roots[0]];
    assert_eq!(
        spatial::quote_direct_work(layer, region, &|| Ok(())).unwrap(),
        256
    );
    let b = SharedBudget::new(1 << 20);
    let low = factory(&b, 511);
    assert_eq!(
        with_workspace(low.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(low.work_used(), 0);
    let a = factory(&b, 512);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d)).unwrap(),
        [129, 16, 255, 255, 195, 0, 111, 255]
    );
    assert_eq!(a.work_used(), 256);
    assert_eq!(
        with_workspace(a.clone(), || render_flat(&d))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 256);
    assert_eq!(b.usage().reserved, 0);
}
#[test]
fn mixed_direct_kinds_keep_their_own_qualified_prices() {
    let d = scene(vec![
        h(),
        Descriptor::VibranceRgbV1(VibranceRgbV1::new(100).unwrap()),
    ]);
    let p = LayerAppearancePlan::new(&d.layers[&d.roots[0]], d.canvas_rect(), &|| Ok(())).unwrap();
    assert_eq!(p.direct_work(&|| Ok(())).unwrap(), 2 * (128 + 64));
    assert_eq!(p.quote().work, 2 + 4 * 2 + 2 * (128 + 64));
}
#[test]
fn box_halos_price_hsl128_each_actual_stage_slot_and_bulk_spend_once() {
    let b = SharedBudget::new(1 << 20);
    let blur = Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap());
    for (ops, direct) in [(vec![h(), blur], 256), (vec![blur, h()], 128)] {
        let d = scene(ops);
        let p = LayerAppearancePlan::new(
            &d.layers[&d.roots[0]],
            Rect::new(0, 0, 1, 1).unwrap(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(p.direct_work(&|| Ok(())).unwrap(), direct);
        let a = factory(&b, p.quote().work);
        p.with_pixels(&a, &|| Ok(()), |pixels| {
            assert_eq!(pixels.len(), 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(a.work_used(), p.quote().work);
        assert_eq!(b.usage().reserved, 0);
    }
}

#[path = "hue_saturation_tests/ordered_data.rs"]
mod ordered_data;
#[test]
fn independent_fraction_orders_preserve_stage_rounding_masks_source_and_intent() {
    use paintcraft_core::types::filters::CurvesRgbV1;
    use paintcraft_document::FilterMask;
    for (params, coverage, other, forward, reverse) in ordered_data::CASES {
        let hsl = Descriptor::HueSaturationRgbV1(
            HueSaturationRgbV1::new(params[0], params[1], params[2]).unwrap(),
        );
        let other = match other {
            0 => Descriptor::VibranceRgbV1(VibranceRgbV1::new(100).unwrap()),
            1 => Descriptor::CurvesRgbV1(CurvesRgbV1::new([23, 9, 240, 73, 201])),
            2 => Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
            _ => unreachable!(),
        };
        for (reverse_order, expected) in [(false, forward), (true, reverse)] {
            let b = SharedBudget::new(1 << 20);
            let mut d = paintcraft_document::import_image(
                3,
                1,
                &[20, 115, 200, 255, 64, 16, 128, 255, 200, 30, 30, 255],
                "ordered-independent",
            )
            .unwrap();
            let layer = d.layers.get_mut(&d.roots[0]).unwrap();
            let raw = layer.raster.clone();
            let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
                Rect::new(0, 0, 3, 1).unwrap(),
                coverage,
                &b,
                Default::default(),
                &|| Ok(()),
            )
            .unwrap();
            let hsl = FilterInstance::new(deterministic_id(&["ordered-hsl"]), true, hsl)
                .with_mask(Some(FilterMask::new(true, mask)));
            let other = FilterInstance::new(deterministic_id(&["ordered-other"]), true, other);
            let instances = if reverse_order {
                vec![other, hsl]
            } else {
                vec![hsl, other]
            };
            layer.filters = FilterStack::new(instances).unwrap();
            let hash = layer.filters.digest();
            let state = d.state_id;
            let a = factory(&b, 10_000_000);
            assert_eq!(with_workspace(a, || render_flat(&d)).unwrap(), expected);
            let layer = &d.layers[&d.roots[0]];
            assert_eq!(layer.raster, raw);
            assert_eq!(layer.filters.digest(), hash);
            assert_eq!(d.state_id, state);
        }
    }
}
