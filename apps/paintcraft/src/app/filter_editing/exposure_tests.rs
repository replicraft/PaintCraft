//! Real controls and native preparation; no operating-system window.
use super::*;
use paintcraft_core::types::filters::{Descriptor, ExposureSrgbV1};

fn open_exposure(h: &mut Harness<'_, App>) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Exposure").click();
    h.run();
    assert!(
        h.state()
            .editor
            .capture_preview(h.state().editor.preview_snapshot())
            .requires_spatial_workspace()
    );
    h.get_by_role_and_label(egui::Role::Slider, "Exposure (EV)")
        .set_value_accesskit(1.0);
    h.run();
}

#[test]
fn exposure_controls_private_frames_cancel_apply_history_and_project_roundtrip() {
    // Independently derived by the Fraction oracle, not the product compiler.
    let expected = [18, 31, 45, 255, 138, 205, 255, 255];
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        open_exposure(&mut h);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        open_exposure(&mut h);
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
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap())
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 6);
        let path = std::env::temp_dir().join(format!(
            "paintcraft-native-exposure-{}.pcnd",
            Uuid::new_v4()
        ));
        std::fs::write(&path, &bytes).unwrap();
        let reopened = Editor::open(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(reopened.unwrap().document().same_semantics(doc));
        h.state_mut().editor.request(903, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(904, Action::Redo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
    }
}
