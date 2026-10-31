//! Real controls and native preparation; no operating-system window.
use super::*;
use paintcraft_core::types::filters::{Descriptor, HueSaturationRgbV1};

const RAW_HSL: [u8; 8] = [64, 16, 128, 255, 64, 16, 128, 255];
fn hsl_app(asynchronous: bool) -> App {
    let mut a = app(false);
    a.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &RAW_HSL, "native-hsl-input").unwrap(),
        None,
    );
    if asynchronous {
        a.initialize_editing(a.context.clone());
        a.renderer = Some(
            Executor::new(WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            })
            .unwrap(),
        );
    }
    a
}
fn open_hsl(h: &mut Harness<'_, App>) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Hue / Saturation").click();
    h.run();
    assert!(
        h.state()
            .editor
            .capture_preview(h.state().editor.preview_snapshot())
            .requires_spatial_workspace()
    );
    let source = h.state().editor.document_snapshot();
    let ctx = h.ctx.clone();
    for (label, value, rgb) in [
        ("Hue (degrees)", 137.0, [128, 96, 16]),
        ("Saturation adjustment (%)", 73.0, [144, 103, 0]),
        ("Lightness adjustment (%)", -19.0, [47, 34, 0]),
    ] {
        h.get_by_role_and_label(egui::Role::Slider, label)
            .set_value_accesskit(value);
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, [rgb[0], rgb[1], rgb[2], 255].repeat(2));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
    }
}

#[test]
fn hsl_controls_private_frames_cancel_apply_history_and_project_roundtrip() {
    // Independently derived by the Fraction oracle, not the product compiler.
    let expected = [47, 34, 0, 255, 47, 34, 0, 255];
    for asynchronous in [false, true] {
        let mut h = harness(hsl_app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        open_hsl(&mut h);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW_HSL);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        open_hsl(&mut h);
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
            Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::new(137, 73, -19).unwrap())
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 6);
        let path =
            std::env::temp_dir().join(format!("paintcraft-native-hsl-{}.pcnd", Uuid::new_v4()));
        std::fs::write(&path, &bytes).unwrap();
        let reopened = Editor::open(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(reopened.unwrap().document().same_semantics(doc));
        h.state_mut().editor.request(903, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW_HSL);
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(904, Action::Redo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
    }
}
