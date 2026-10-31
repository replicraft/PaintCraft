//! Real controls and private native render jobs; no operating-system window.
use super::*;
use paintcraft_core::types::filters::LevelsRgbV1;

fn open_levels(h: &mut Harness<'_, App>) {
    h.get_by_label("Add filter").click();
    h.run();
    h.get_by_label("Levels").click();
    h.run();
    for label in [
        "Input black",
        "Input white",
        "Gamma",
        "Output black",
        "Output white",
    ] {
        let _ = h.get_by_role_and_label(egui::Role::Slider, label);
    }
    assert!(
        h.state()
            .editor
            .capture_preview(h.state().editor.preview_snapshot())
            .requires_spatial_workspace()
    );
    h.get_by_role_and_label(egui::Role::Slider, "Gamma")
        .set_value_accesskit(2.0);
    h.run();
}
#[test]
fn levels_native_controls_private_frames_cancel_apply_history_and_project_roundtrip() {
    // Round(sqrt(255*x)) independently gives these six opaque channel bytes.
    let expected = [50, 71, 87, 255, 160, 196, 226, 255];
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        open_levels(&mut h);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        open_levels(&mut h);
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
            paintcraft_core::types::filters::Descriptor::LevelsRgbV1(
                LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap()
            )
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 6);
        let path =
            std::env::temp_dir().join(format!("paintcraft-native-levels-{}.pcnd", Uuid::new_v4()));
        std::fs::write(&path, &bytes).unwrap();
        let reopened = Editor::open(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(reopened.unwrap().document().same_semantics(doc));
        h.state_mut().editor.request(901, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(h.state().editor.document().same_semantics(&source));
        h.state_mut().editor.request(902, Action::Redo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
    }
}
