//! Real shared five-control dialog and native execution, without an OS window.
use super::*;
use paintcraft_core::types::filters::{CurvesRgbV1, Descriptor};

fn open_constant_curves(h: &mut Harness<'_, App>) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Curves").click();
    h.run();
    assert!(
        h.state()
            .editor
            .capture_preview(h.state().editor.preview_snapshot())
            .requires_spatial_workspace()
    );
    for caption in [
        "Black (input 0)",
        "Shadows (input 64)",
        "Midtones (input 128)",
        "Highlights (input 192)",
        "White (input 255)",
    ] {
        h.get_by_role_and_label(egui::Role::Slider, caption)
            .set_value_accesskit(37.0);
        h.run();
    }
}

#[test]
fn curves_five_controls_private_constant_cancel_apply_history_and_fresh_open() {
    // Constant Hermite outputs have zero tangents: all RGB bytes are exactly37.
    // This literal does not call a product evaluator to create its expectation.
    let expected = [37, 37, 37, 255, 37, 37, 37, 255];
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        let history = h.state().editor.history_view();
        open_constant_curves(&mut h);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert_eq!(h.state().editor.undo_depth(), 0);
        assert!(!h.state().editor.published_is_dirty());
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view(), history);
        assert!(!h.state().editor.published_is_dirty());
        open_constant_curves(&mut h);
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert_eq!(h.state().editor.undo_depth(), 1);
        let doc = h.state().editor.document();
        let layer = &doc.layers[&doc.roots[0]];
        assert_eq!(layer.filters.len(), 1);
        assert_eq!(
            layer.filters.as_slice()[0].descriptor(),
            Descriptor::CurvesRgbV1(CurvesRgbV1::new([37; 5]))
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        assert_eq!(doc.document_id, source.document_id);
        assert_ne!(doc.state_id, source.state_id);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 6);
        let path =
            std::env::temp_dir().join(format!("paintcraft-native-curves-{}.pcnd", Uuid::new_v4()));
        std::fs::write(&path, &bytes).unwrap();
        let reopened = Editor::open(&path);
        std::fs::remove_file(&path).unwrap();
        let reopened = reopened.unwrap();
        assert!(reopened.document().same_semantics(doc));
        let applied_state = doc.state_id;
        let mut reopened_app = app(asynchronous);
        reopened_app.editor = reopened;
        let mut fresh = harness(reopened_app);
        fresh.run();
        let fresh_ctx = fresh.ctx.clone();
        frame(fresh.state_mut(), &fresh_ctx);
        assert_eq!(fresh.state().rendered, expected);
        assert_eq!(fresh.state().editor.document().state_id, applied_state);
        assert_eq!(fresh.state().editor.undo_depth(), 0);
        assert_eq!(
            paintcraft_formats::pcnd::write_compact(fresh.state().editor.document()).unwrap(),
            bytes
        );
        h.state_mut().editor.request(905, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(906, Action::Redo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert_eq!(h.state().editor.document().state_id, applied_state);
        assert_eq!(
            h.state().editor.document().layers[&source.roots[0]].raster,
            source.layers[&source.roots[0]].raster
        );
    }
}
