//! Real shared controls and native FIFO publication for the four RGB profiles.
use super::*;
use paintcraft_core::types::Rect;
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_studio::filters::Kind;
const CROSS: [u8; 8] = [20, 115, 0, 255, 0, 178, 4, 255];
fn cases() -> [(Kind, [u8; 8]); 4] {
    [
        (Kind::BlackWhite, [19, 19, 19, 255, 143, 143, 143, 255]),
        (Kind::Threshold, [0, 0, 0, 255, 255, 255, 255, 255]),
        (Kind::Posterize, [0, 0, 51, 255, 102, 153, 204, 255]),
        (Kind::Solarize, [10, 20, 30, 255, 100, 105, 55, 255]),
    ]
}
fn open_kind(h: &mut Harness<'_, App>, kind: Kind) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label(kind.label()).click();
    h.run();
    assert_eq!(
        h.state()
            .filter_editing
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .parameters
            .kind,
        kind
    );
    let label = match kind {
        Kind::BlackWhite => Some("Strength (%)"),
        Kind::Threshold => Some("Threshold"),
        Kind::Posterize => Some("Levels"),
        Kind::Solarize => None,
        _ => unreachable!(),
    };
    if let Some(label) = label {
        let _ = h.get_by_role_and_label(egui::Role::Slider, label);
    } else {
        assert!(h.query_by_role(egui::Role::Slider).is_none());
    }
}
#[test]
fn four_actual_add_controls_preview_cancel_apply_one_undo_and_keep_original_pixels() {
    for asynchronous in [false, true] {
        for (kind, expected) in cases() {
            let mut h = harness(app(asynchronous));
            h.run();
            let ctx = h.ctx.clone();
            let source = h.state().editor.document_snapshot();
            open_kind(&mut h, kind);
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, expected, "{kind:?}");
            assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            assert_eq!(h.state().editor.undo_depth(), 0);
            assert!(!h.state().editor.published_is_dirty());
            assert!(h.state().editor.export_snapshot().is_err());
            h.get_by_label("Cancel").click();
            h.run();
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, RAW);
            assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            open_kind(&mut h, kind);
            h.get_by_label("Apply").click();
            h.run_steps(1);
            drain(h.state_mut());
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, expected);
            assert_eq!(h.state().editor.undo_depth(), 1);
            let doc = h.state().editor.document();
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(
                layer.filters.as_slice()[0].descriptor(),
                Parameters::new(kind).descriptor().unwrap()
            );
            assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
            let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
            assert!(doc.same_semantics(&paintcraft_formats::pcnd::read(&bytes).unwrap()));
            h.state_mut().editor.request(7601, Action::Undo);
            drain(h.state_mut());
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, RAW);
        }
    }
}
fn edit_app(kind: Kind, asynchronous: bool) -> App {
    let mut a = app(asynchronous);
    let mut doc =
        paintcraft_document::import_image(2, 1, &CROSS, "native-point-filter-edit/v1").unwrap();
    let coverage = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        128,
        &paintcraft_core::types::resources::default_coverage_budget(),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let mut params = Parameters::new(kind);
    if kind == Kind::BlackWhite {
        params.strength = 0;
    }
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(81), true, params.descriptor().unwrap())
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    a.editor = Editor::with_document(doc, None);
    if asynchronous {
        a.initialize_editing(a.context.clone());
    }
    a
}
struct MaskCase {
    kind: Kind,
    control: Option<(&'static str, f64)>,
    before: [u8; 8],
    expected: [u8; 8],
}
fn masked_cases() -> [MaskCase; 4] {
    [
        MaskCase {
            kind: Kind::BlackWhite,
            control: Some(("Strength (%)", 100.)),
            before: CROSS,
            expected: [54, 101, 44, 255, 64, 153, 66, 255],
        },
        MaskCase {
            kind: Kind::Threshold,
            control: Some(("Threshold", 127.)),
            before: [10, 57, 0, 255, 0, 89, 2, 255],
            expected: [10, 57, 0, 255, 128, 217, 130, 255],
        },
        MaskCase {
            kind: Kind::Posterize,
            control: Some(("Levels", 3.)),
            before: [10, 108, 0, 255, 0, 165, 2, 255],
            expected: [10, 122, 0, 255, 0, 153, 2, 255],
        },
        MaskCase {
            kind: Kind::Solarize,
            control: None,
            before: [20, 115, 0, 255, 0, 127, 4, 255],
            expected: [20, 115, 0, 255, 0, 127, 4, 255],
        },
    ]
}
#[test]
fn actual_parameter_controls_keep_fractional_masks_and_preview_off_applies_final_values() {
    for asynchronous in [false, true] {
        for MaskCase {
            kind,
            control,
            before,
            expected,
        } in masked_cases()
        {
            let mut h = harness(edit_app(kind, asynchronous));
            h.run();
            let ctx = h.ctx.clone();
            let source = h.state().editor.document_snapshot();
            let old_mask = source.layers[&source.roots[0]].filters.as_slice()[0]
                .mask()
                .unwrap()
                .clone();
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, before, "{kind:?}");
            h.get_by_label("Edit").click();
            h.run();
            if let Some((label, value)) = control {
                h.get_by_role_and_label(egui::Role::Slider, label)
                    .set_value_accesskit(value);
                h.run();
            }
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, expected, "{kind:?}");
            assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            assert_eq!(h.state().editor.undo_depth(), 0);
            h.get_by_label("Preview").click();
            h.run();
            frame(h.state_mut(), &ctx);
            assert_eq!(h.state().rendered, before);
            h.get_by_label("Apply").click();
            h.run_steps(1);
            drain(h.state_mut());
            frame(h.state_mut(), &ctx);
            assert!(h.state().filter_editing.is_none());
            assert_eq!(h.state().rendered, expected);
            let changed = usize::from(kind != Kind::Solarize);
            assert_eq!(h.state().editor.undo_depth(), changed);
            let doc = h.state().editor.document();
            let layer = &doc.layers[&doc.roots[0]];
            assert_eq!(layer.filters.as_slice()[0].mask(), Some(&old_mask));
            assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
            assert_eq!(layer.raster.get(0, 0), Rgba([20, 115, 0, 255]));
            if changed != 0 {
                h.state_mut().editor.request(7602, Action::Undo);
                drain(h.state_mut());
                frame(h.state_mut(), &ctx);
                assert_eq!(h.state().rendered, before);
            }
        }
    }
}
