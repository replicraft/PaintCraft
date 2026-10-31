//! Actual Add menu/radius control plus private preview and native FIFO Apply.
use super::*;
use paintcraft_studio::filters::Kind;

#[test]
fn real_box_controls_preview_cancel_apply_once_and_undo_with_no_source_rewrite() {
    for asynchronous in [false, true] {
        let mut h = harness(app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        h.get_by_label("Add filter").click();
        h.run();
        h.get_by_label("Box Blur").click();
        h.run();
        let _ = h.get_by_role_and_label(egui::Role::Slider, "Radius (px)");
        let draft = h
            .state()
            .filter_editing
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap();
        assert_eq!(draft.parameters.kind, Kind::BoxBlur);
        assert_eq!(draft.parameters.radius, 2);
        assert!(
            h.state()
                .editor
                .capture_preview(h.state().editor.preview_snapshot())
                .requires_spatial_workspace()
        );
        frame(h.state_mut(), &ctx);
        // Independent 5-tap finite CLAMP: three left/two right, then vice versa.
        let expected = [46, 72, 98, 255, 64, 98, 132, 255];
        assert_eq!(h.state().rendered, expected);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.get_by_label("Cancel").click();
        h.run();
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        h.get_by_label("Add filter").click();
        h.run();
        h.get_by_label("Box Blur").click();
        h.run();
        h.get_by_label("Apply").click();
        h.run_steps(1);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, expected);
        assert_eq!(h.state().editor.undo_depth(), 1);
        let current = h.state().editor.document();
        let layer = &current.layers[&current.roots[0]];
        assert_eq!(
            layer.filters.as_slice()[0].descriptor(),
            Parameters::new(Kind::BoxBlur).descriptor().unwrap()
        );
        assert_eq!(layer.raster, source.layers[&source.roots[0]].raster);
        h.state_mut().editor.request(991, Action::Undo);
        drain(h.state_mut());
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, RAW);
        assert!(h.state().editor.document().same_semantics(&source));
    }
}
