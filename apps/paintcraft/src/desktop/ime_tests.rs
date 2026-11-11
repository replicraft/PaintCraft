use super::*;
use paintcraft_tools::Tool;
#[test]
fn actual_owned_loop_normalization_preserves_order_and_controller_cancels_late_result() {
    let mut app = App::new(None);
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((8, 8));
    app.editor.type_text("Base");
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    let original = app.editor.document_snapshot();
    let before = app.editor.publications();
    let mut raw = egui::RawInput::default();
    assert!(ime::append(&mut raw, &WindowEvent::Ime(winit::event::Ime::Enabled)).unwrap());
    raw.events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
        text: " candidate".into(),
        active_range_chars: Some(0..10),
    }));
    assert!(ime::append(&mut raw, &WindowEvent::Ime(winit::event::Ime::Disabled)).unwrap());
    raw.events
        .push(egui::Event::Ime(egui::ImeEvent::Commit(" late".into())));
    let count = raw.events.len();
    assert!(
        !ime::append(
            &mut raw,
            &WindowEvent::Ime(winit::event::Ime::Commit("official path".into()))
        )
        .unwrap()
    );
    assert_eq!(raw.events.len(), count);
    assert_eq!(count, 4);
    for event in &raw.events {
        assert!(app.canvas_text_event(&ctx, event));
    }
    assert_eq!(app.editor.canvas_text_content(), Some("Base"));
    assert!(!app.ime_composing);
    assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
    assert_eq!(app.editor.publications(), before);
    raw.events.clear();
    assert!(ime::append(&mut raw, &WindowEvent::Ime(winit::event::Ime::Enabled)).unwrap());
    raw.events
        .push(egui::Event::Ime(egui::ImeEvent::Commit(" fresh".into())));
    for event in &raw.events {
        assert!(app.canvas_text_event(&ctx, event));
    }
    assert_eq!(app.editor.canvas_text_content(), Some("Base fresh"));
}
#[test]
fn lifecycle_allocation_failure_retires_unpublished_composition_without_history_changes() {
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    let mut app = App::new(None);
    app.initialize_editing(ctx.clone());
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((8, 8));
    app.editor.type_text("Base");
    assert!(app.canvas_text_event(
        &ctx,
        &egui::Event::Ime(egui::ImeEvent::Preedit {
            text: " candidate".into(),
            active_range_chars: Some(0..10)
        })
    ));
    let original = app.editor.document_snapshot();
    let depth = app.editor.undo_depth();
    let budget = app.worker_budget.clone();
    let mut host = Host::new(app, 8, &budget).unwrap();
    let mut raw = egui::RawInput {
        events: vec![egui::Event::Text("pending input".into())],
        ..Default::default()
    };
    // Defining TryReserveError from a real checked overflow, without allocating
    // an enormous vector or replacing the allocator/framework.
    let error = Vec::<u8>::new().try_reserve(usize::MAX).unwrap_err();
    ime::retire_failed(&mut host, &mut raw, &error);
    assert!(raw.events.is_empty());
    let app = host.app_mut();
    assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
    assert_eq!(app.editor.undo_depth(), depth);
    assert!(app.canvas_text_event(
        &ctx,
        &egui::Event::Ime(egui::ImeEvent::Commit(" late".into()))
    ));
    assert_eq!(app.editor.canvas_text_content(), Some("Base"));
}
