use super::*;
use crate::{
    export::{self, FlatOptions},
    image::ImageFormat,
};
use paintcraft_core::types::{
    LayerBlendMode, Rect, Rgba,
    filters::Descriptor,
    resources::{AllocationClass, SharedBudget},
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
use paintcraft_document::{
    FilterInstance, FilterMask, FilterStack, LayerMask, StyleDescriptor, StyleInstance, StyleStack,
};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits, Run};
use std::cell::Cell;
const PNG: Target = Target::new(Family::Png, Operation::ExportFlat);
fn mask(a: u8, b: u8, budget: &SharedBudget) -> A8Mask {
    let runs = [
        Run {
            x0: 0,
            x1: 1,
            coverage: a,
        },
        Run {
            x0: 1,
            x1: 2,
            coverage: b,
        },
    ];
    A8Mask::from_bands(
        Rect::new(0, 0, 2, 1).unwrap(),
        &[Band {
            y0: 0,
            y1: 1,
            runs: &runs,
        }],
        budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn document() -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([31, 127, 241, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut top = doc.layers[&doc.roots[0]].clone();
    top.id = Uuid::from_u128(13);
    top.raster = paintcraft_raster::RasterLayer::new(2, 1, Rgba([204, 77, 13, 128]));
    let budget = SharedBudget::new(1 << 20);
    top.filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(14), true, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, mask(0, 255, &budget)))),
    ])
    .unwrap();
    top.styles = StyleStack::new(vec![
        StyleInstance::new(
            Uuid::from_u128(15),
            true,
            StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [220, 40, 0],
                128,
                EffectBlendMode::Normal,
            )),
        )
        .unwrap(),
    ])
    .unwrap();
    top.mask = Some(LayerMask::new(true, mask(127, 255, &budget)));
    top.opacity = 128;
    top.blend_mode = LayerBlendMode::Multiply;
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    doc
}
fn options(doc: &Document) -> FlatOptions {
    let mut o = FlatOptions::new(ImageFormat::Png, 90);
    o.appearance_bake = Policy::capture(doc, PNG, Intent::Bake, &|| Ok(())).unwrap();
    o
}
fn allowed() -> Vec<String> {
    [
        "flatten",
        BAKED_REASON,
        super::super::filter_bake::BAKED_REASON,
        COMPOUND_REASON,
    ]
    .map(str::to_owned)
    .to_vec()
}
#[test]
fn compound_png_runs_complete_cpu_pipeline_returns_actual_disclosure_and_preserves_pcnd9() {
    let doc = document();
    let before = crate::pcnd::write_compact(&doc).unwrap();
    let budget = SharedBudget::new(4 << 20);
    let original = paintcraft_render::spatial::WorkspaceAdmission::new(
        budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget),
        4 << 20,
        268_435_456,
    );
    original.reserve_work(17).unwrap();
    let out = paintcraft_render::spatial::with_workspace(original.clone(), || {
        export::export_flat_options(&doc, options(&doc), &allowed())
    })
    .unwrap();
    assert_eq!(
        crate::image::decode(&out.bytes).unwrap().rgba,
        [50, 108, 189, 144, 49, 109, 192, 160]
    );
    assert_eq!(out.losses, allowed());
    let value = out.disclosure.unwrap();
    assert_eq!(
        value.get("schema").unwrap().as_str(),
        Some("paintcraft.export-disclosure/layer-styles-v1")
    );
    assert_eq!(
        value.get("source_state_id").unwrap().as_str(),
        Some(doc.state_id.to_string().as_str())
    );
    for key in [
        "styles_baked",
        "filters_baked",
        "filter_masks_baked",
        "layer_masks_baked",
        "layer_blends_baked",
    ] {
        assert_eq!(value.get(key).unwrap().as_bool(), Some(true));
    }
    assert_eq!(
        value.get("counts").unwrap().get("styles").unwrap().as_i64(),
        Some(1)
    );
    assert!(original.work_used() > 17);
    assert_eq!(original.scope().footprint().live, 0);
    assert_eq!(crate::pcnd::write_compact(&doc).unwrap(), before);
}
#[test]
fn consent_and_loss_acknowledgement_are_independent_and_refuse_before_renderer() {
    let doc = document();
    let calls = Cell::new(0);
    for (o, losses) in [
        (FlatOptions::new(ImageFormat::Png, 90), allowed()),
        (options(&doc), vec![]),
    ] {
        let e = export::export_flat_options_with(&doc, o, &losses, |_| {
            calls.set(calls.get() + 1);
            Err(invalid("unexpected render"))
        })
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(calls.get(), 0);
    }
    // A typed legacy filter-only witness cannot be captured for this masked,
    // blended source; no old loss-name acknowledgement bypasses that refusal.
    assert!(
        export::filter_bake::Policy::capture(&doc, PNG, export::filter_bake::Intent::Bake, &|| Ok(
            ()
        ))
        .is_err()
    );
}
#[test]
fn witness_binds_target_source_order_and_all_discarded_intentions() {
    let doc = document();
    let policy = options(&doc).appearance_bake;
    for change in 0..6 {
        let mut changed = doc.clone();
        let top = *changed.roots.last().unwrap();
        match change {
            0 => changed.state_id = Uuid::from_u128(77),
            1 => changed.roots.reverse(),
            2 => changed.layers.get_mut(&top).unwrap().name.push('!'),
            3 => changed.layers.get_mut(&top).unwrap().opacity = 127,
            4 => changed.layers.get_mut(&top).unwrap().mask = None,
            _ => changed.layers.get_mut(&top).unwrap().filters = FilterStack::default(),
        }
        assert_eq!(
            policy.validate(&changed, PNG, &|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
    for target in [
        Target::new(Family::Bmp, Operation::ExportFlat),
        Target::new(Family::Pcnd, Operation::SaveAsCopy),
    ] {
        assert_eq!(
            policy.validate(&doc, target, &|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        policy
            .validate(
                &doc,
                Target::new(Family::Png, Operation::SaveAsCopy),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let cancelled = Policy::capture(&doc, PNG, Intent::Bake, &|| {
        Err(Error::new(ErrorCode::OperationCancelled, "capture stopped"))
    })
    .unwrap_err();
    assert_eq!(cancelled.code, ErrorCode::OperationCancelled);
}
#[test]
fn disabled_zero_opacity_styles_still_need_consent_and_unstyled_exports_have_no_new_disclosure() {
    let mut doc = document();
    let top = *doc.roots.last().unwrap();
    let l = doc.layers.get_mut(&top).unwrap();
    let style = l.styles.as_slice()[0];
    l.styles = StyleStack::new(vec![style.with_enabled(false)]).unwrap();
    l.opacity = 0;
    l.visible = false;
    assert!(Policy::Refuse.validate(&doc, PNG, &|| Ok(())).is_err());
    let inv = inspect(&doc, &|| Ok(())).unwrap();
    assert_eq!(inv.style_count(), 1);
    assert_eq!(inv.filter_count(), 1);
    assert_eq!(inv.filter_mask_count(), 1);
    assert_eq!(inv.layer_mask_count(), 1);
    assert_eq!(inv.non_normal_layer_count(), 1);
    assert!(
        options(&doc)
            .appearance_bake
            .validate(&doc, PNG, &|| Ok(()))
            .unwrap()
    );
    let plain = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([1, 2, 3, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let out = export::export_flat(&plain, ImageFormat::Png, 90, &[]).unwrap();
    assert!(out.disclosure.is_none());
    assert!(out.losses.is_empty());
    assert!(Policy::capture(&plain, PNG, Intent::Bake, &|| Ok(())).is_err());
}
