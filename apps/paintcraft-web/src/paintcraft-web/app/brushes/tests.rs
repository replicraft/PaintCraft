use super::*;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/abr-fixtures/native-presets.abr"
);
fn setup() -> WebApp {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(
        paintcraft_document::new_document(16, 16, None, 0, paintcraft_document::POLICY).unwrap(),
    )
    .unwrap();
    app.begin_brushes();
    app.brushes.loaded = Some(Loaded::new(SOURCE, "native.abr".into()).unwrap());
    app
}
#[test]
fn browser_adoption_pointer_preview_and_release_are_exact_and_blank_first_stroke_is_atomic() {
    let mut app = setup();
    let before = app.editor.session.snapshot();
    app.adopt_brush().unwrap();
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    app.editor.tools.pointer_down_subpixel(
        uuid::Uuid::nil(),
        (5 * 256, 5 * 256),
        paintcraft_input::PointerButton::Primary,
    );
    app.editor.tools.pointer_move_subpixel((8 * 256, 5 * 256));
    let Some(paintcraft_tools::Preview::SampledStroke(prefix)) = app.editor.tools.preview() else {
        panic!()
    };
    let operations = app.editor.sampled_operations(&prefix).unwrap();
    let preview = paintcraft_editor::execute::execute(
        &before,
        app.editor.session.selection(),
        &app.editor.batch(operations).unwrap(),
    )
    .unwrap()
    .doc;
    let (_, stroke) = app.editor.tools.pointer_up().unwrap();
    app.edit(stroke);
    assert!(app.editor.session.document().same_semantics(&preview));
    assert_eq!(app.editor.session.undo_depth(), 1);
    assert_eq!(app.editor.session.document().layers.len(), 1);
    app.edit(Edit::Undo);
    assert!(app.editor.session.document().layers.is_empty());
    app.edit(Edit::Redo);
    assert!(app.editor.session.document().same_semantics(&preview));
}
#[test]
fn browser_work_limit_rejects_entire_stroke_without_layer_history_or_owner_changes() {
    let mut app = setup();
    app.adopt_brush().unwrap();
    let before = app.editor.session.snapshot();
    let tip = app.editor.tools.sampled_tip.as_ref().unwrap().0.clone();
    let mut stroke = paintcraft_tools::sampled_brush::SampledStroke::begin(
        uuid::Uuid::nil(),
        paintcraft_core::types::Rgba([0, 0, 0, 255]),
        tip,
        (0, 0),
        1,
        1,
    )
    .unwrap();
    stroke.append((100000, 0)).unwrap();
    assert!(app.editor.sampled_operations(&stroke).is_err());
    app.edit(Edit::SampledStroke(stroke));
    assert!(app.message.contains("work limit"));
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(app.editor.session.document().layers.is_empty());
    assert!(app.editor.adopted_brush.is_some());
}
#[test]
fn browser_consent_unsupported_stale_and_library_replacement_preserve_prior_adoption() {
    let mut app = setup();
    app.brushes.selected = 1;
    assert!(app.adopt_brush().is_err());
    app.brushes.controls.consent = true;
    app.adopt_brush().unwrap();
    let adopted = app.editor.adopted_brush.clone().unwrap();
    app.begin_brushes();
    app.brushes.loaded = Some(Loaded::new(SOURCE, "replacement.abr".into()).unwrap());
    assert!(Arc::ptr_eq(
        adopted.source(),
        app.editor.adopted_brush.as_ref().unwrap().source()
    ));
    app.brushes.selected = 2;
    assert!(app.adopt_brush().is_err());
    let ticket = app.editor.session.ticket();
    app.brushes.owner = Some((ticket.state_id, ticket.publication_epoch + 1));
    app.brushes.selected = 0;
    assert!(app.adopt_brush().is_err());
    assert!(Arc::ptr_eq(
        adopted.source(),
        app.editor.adopted_brush.as_ref().unwrap().source()
    ));
}
#[test]
fn browser_classic_workspace_and_cancelled_failed_or_stale_resource_callbacks_cannot_replace_adoption()
 {
    let ctx = egui::Context::default();
    let mut app = setup();
    app.adopt_brush().unwrap();
    let source = app.editor.adopted_brush.clone().unwrap();
    app.switch_frontend(&ctx, Frontend::Classic);
    assert!(app.editor.tools.sampled_tip.is_none());
    assert!(app.editor.adopted_brush.is_none());
    app.switch_frontend(&ctx, Frontend::Studio);
    assert!(Arc::ptr_eq(
        source.source(),
        app.editor.adopted_brush.as_ref().unwrap().source()
    ));
    app.begin_brushes();
    let epoch = app.brushes.epoch;
    let before = app.editor.session.snapshot();
    for result in [
        Ok(None),
        Err(Error::new(ErrorCode::CodecFailed, "invalid brush")),
    ] {
        app.brushes.busy = true;
        *app.brushes.inbox.borrow_mut() = Inbox {
            epoch,
            result: Some(result),
        };
        let mut output = ctx.run_ui(Default::default(), |ui| app.brush_dialog(ui.ctx()));
        output.textures_delta.clear();
        assert!(!app.brushes.busy);
        assert!(Arc::ptr_eq(
            source.source(),
            app.editor.adopted_brush.as_ref().unwrap().source()
        ));
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    }
    *app.brushes.inbox.borrow_mut() = Inbox {
        epoch: uuid::Uuid::new_v4(),
        result: Some(Loaded::new(SOURCE, "stale.abr".into()).map(Some)),
    };
    let mut output = ctx.run_ui(Default::default(), |ui| app.brush_dialog(ui.ctx()));
    output.textures_delta.clear();
    assert_ne!(app.brushes.loaded.as_ref().unwrap().view.name, "stale.abr");
}
