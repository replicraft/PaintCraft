use super::*;
fn app() -> App {
    let mut app = App::new(None);
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((8, 8));
    assert!(app.editor.is_editing_text());
    app
}
fn preedit(text: &str) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::Preedit {
        text: text.into(),
        active_range_chars: Some(0..text.chars().count()),
    })
}
fn commit(text: &str) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::Commit(text.into()))
}
fn event(app: &mut App, ctx: &egui::Context, event: egui::Event) {
    assert!(app.canvas_text_event(ctx, &event));
}
fn preview_text(app: &App) -> String {
    match app.canvas_text_preview().as_deref() {
        Some(Preview::Text { text, .. }) => text.clone(),
        _ => panic!(),
    }
}
#[test]
fn preedit_is_visible_but_never_enters_tool_actions_or_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("Base ");
    let original = app.editor.document_snapshot();
    let publications = app.editor.publications();
    event(&mut app, &ctx, preedit("候補"));
    assert_eq!(preview_text(&app), "Base 候補");
    assert_eq!(app.editor.canvas_text_content(), Some("Base "));
    assert!(app.editor.document().same_semantics(&original));
    assert_eq!(app.editor.publications(), publications);
    event(&mut app, &ctx, commit("日本語"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base 日本語"));
    assert!(app.canvas_ime.binding.is_none());
    assert_eq!(app.editor.publications(), publications);
}
#[test]
fn cached_provisional_pixels_cannot_publish_and_real_result_is_one_undo_step() {
    let mut app = app();
    let ctx = egui::Context::default();
    let original = app.editor.document_snapshot();
    event(&mut app, &ctx, preedit("temporary"));
    let staged = app
        .editor
        .prepare_preview(app.canvas_text_preview().as_deref().cloned())
        .build(|| Ok(()))
        .unwrap()
        .unwrap();
    let provisional =
        paintcraft_editor::execute::execute(app.editor.document(), app.editor.selection(), &staged)
            .unwrap()
            .doc;
    assert!(!provisional.same_semantics(&original));
    assert!(matches!(app.editor.preview(), Some(Preview::Text { text, .. }) if text.is_empty()));
    event(&mut app, &ctx, commit("Final"));
    let real = app.editor.preview_batch().unwrap().unwrap();
    let expected =
        paintcraft_editor::execute::execute(app.editor.document(), app.editor.selection(), &real)
            .unwrap()
            .doc;
    app.editor.commit_text();
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&expected));
    assert!(!app.editor.document().same_semantics(&provisional));
    app.editor.request(81001, Action::Undo);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&original));
}
#[test]
fn direct_result_policy_and_empty_preedit_followed_by_result_are_explicit() {
    let mut app = app();
    let ctx = egui::Context::default();
    event(&mut app, &ctx, commit("direct"));
    assert_eq!(app.editor.canvas_text_content(), Some("direct"));
    event(&mut app, &ctx, preedit("draft"));
    event(&mut app, &ctx, preedit(""));
    assert!(app.ime_composing);
    event(&mut app, &ctx, commit(" result"));
    event(&mut app, &ctx, preedit(" next"));
    assert_eq!(preview_text(&app), "direct result next");
    assert_eq!(app.editor.canvas_text_content(), Some("direct result"));
}
#[test]
fn replacement_draft_and_focus_revocation_refuse_late_result() {
    let mut app = app();
    let ctx = egui::Context::default();
    event(&mut app, &ctx, preedit("old"));
    app.editor.cancel_text();
    app.editor.pointer_down((20, 20));
    event(&mut app, &ctx, preedit(""));
    event(&mut app, &ctx, commit("late"));
    assert_eq!(app.editor.canvas_text_content(), Some(""));
    event(&mut app, &ctx, preedit("fresh"));
    app.canvas_text_event(&ctx, &egui::Event::WindowFocused(false));
    assert!(!app.editor.is_editing_text());
    app.editor.pointer_down((30, 30));
    event(&mut app, &ctx, commit("late"));
    assert_eq!(app.editor.canvas_text_content(), Some(""));
    app.editor.type_text("keep");
    event(
        &mut app,
        &ctx,
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 1,
            after_chars: 0,
        }),
    );
    assert_eq!(app.editor.canvas_text_content(), Some("keep"));
}

#[test]
fn escape_cancels_only_preedit_and_select_all_replaces_only_on_real_result() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("original");
    event(&mut app, &ctx, preedit("discard"));
    event(
        &mut app,
        &ctx,
        egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        },
    );
    assert_eq!(app.editor.canvas_text_content(), Some("original"));
    event(
        &mut app,
        &ctx,
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..Default::default()
            },
        },
    );
    event(&mut app, &ctx, preedit("candidate"));
    assert_eq!(preview_text(&app), "candidate");
    assert_eq!(app.editor.canvas_text_content(), Some("original"));
    event(&mut app, &ctx, commit("result"));
    assert_eq!(app.editor.canvas_text_content(), Some("result"));
    event(
        &mut app,
        &ctx,
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..Default::default()
            },
        },
    );
    event(&mut app, &ctx, preedit("discard"));
    event(&mut app, &ctx, commit(""));
    assert_eq!(app.editor.canvas_text_content(), Some("result"));
}
#[test]
fn capacity_ranges_and_deletes_are_atomic_without_truncation() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("ab");
    event(&mut app, &ctx, preedit(&"x".repeat(4095)));
    assert!(app.canvas_ime.binding.is_none());
    assert_eq!(app.editor.canvas_text_content(), Some("ab"));
    // A new draft below tests independent deletion; the rejected composition
    // above remains quarantined until an observable new start.
    app = self::app();
    app.editor.type_text("ab");
    event(
        &mut app,
        &ctx,
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 1,
            after_chars: 1,
        }),
    );
    assert_eq!(app.editor.canvas_text_content(), Some("ab"));
    event(
        &mut app,
        &ctx,
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 1,
            after_chars: 0,
        }),
    );
    assert_eq!(app.editor.canvas_text_content(), Some("a"));
    event(
        &mut app,
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ab".into(),
            active_range_chars: Some(0..1),
        }),
    );
    assert!(app.canvas_ime.binding.is_none());
    event(&mut app, &ctx, commit(&"x".repeat(4096)));
    assert_eq!(app.editor.canvas_text_content(), Some("a"));
}
#[test]
fn ordered_real_canvas_pointer_then_preedit_and_candidate_geometry() {
    fn frame(app: &mut App, ctx: &egui::Context, input: egui::RawInput) -> egui::FullOutput {
        let mut output = ctx.run_ui(input, |ui| app.ui(ui));
        // No GPU is present in this headless test; explicitly acknowledge the
        // generated font/icon texture deltas rather than leaking them on drop.
        output.textures_delta.clear();
        output
    }
    let mut app = App::new(None);
    let ctx = egui::Context::default();
    app.editor.select_tool(Tool::Text);
    let _ = frame(
        &mut app,
        &ctx,
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        },
    );
    let pos = app.canvas_rect.unwrap().min + egui::vec2(20.0, 20.0);
    let output = frame(
        &mut app,
        &ctx,
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                preedit("Visible"),
            ],
            ..Default::default()
        },
    );
    assert_eq!(preview_text(&app), "Visible");
    assert_eq!(app.editor.canvas_text_content(), Some(""));
    // Rasterization/cache readiness may require one more headless render pass.
    let output = if output.platform_output.ime.is_some() {
        output
    } else {
        frame(&mut app, &ctx, egui::RawInput::default())
    };
    assert!(output.platform_output.ime.is_some());
}

fn ui_events(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| app.ui(ui),
    );
    output.textures_delta.clear();
}
fn expected_text(app: &App, content: &str) -> paintcraft_document::Document {
    let mut preview = app.editor.preview().unwrap();
    let Preview::Text { text, .. } = &mut preview else {
        panic!()
    };
    *text = content.into();
    let batch = app
        .editor
        .prepare_preview(Some(preview))
        .build(|| Ok(()))
        .unwrap()
        .unwrap();
    paintcraft_editor::execute::execute(app.editor.document(), app.editor.selection(), &batch)
        .unwrap()
        .doc
}
#[test]
fn actual_result_before_focus_loss_or_command_enter_publishes_once_and_undoes() {
    for finish in [
        egui::Event::WindowFocused(false),
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ..Default::default()
            },
        },
    ] {
        let mut app = app();
        let ctx = egui::Context::default();
        app.editor.type_text("Base");
        let original = app.editor.document_snapshot();
        let expected = expected_text(&app, "Base result");
        let before = app.editor.publications();
        event(&mut app, &ctx, preedit("candidate"));
        ui_events(&mut app, &ctx, vec![commit(" result"), finish]);
        assert_eq!(app.editor.publications(), before + 1);
        assert!(app.editor.document().same_semantics(&expected));
        assert!(!app.editor.is_editing_text());
        app.editor.request(81002, Action::Undo);
        app.editor.run_pending();
        assert!(app.editor.document().same_semantics(&original));
    }
}
#[test]
fn preedit_then_focus_loss_places_only_committed_base_and_modal_never_places() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("Base");
    let expected = expected_text(&app, "Base");
    let before = app.editor.publications();
    ui_events(
        &mut app,
        &ctx,
        vec![preedit("candidate"), egui::Event::WindowFocused(false)],
    );
    assert_eq!(app.editor.publications(), before + 1);
    assert!(app.editor.document().same_semantics(&expected));
    let mut app = self::app();
    let ctx = egui::Context::default();
    app.editor.type_text("private");
    let original = app.editor.document_snapshot();
    event(&mut app, &ctx, preedit("candidate"));
    app.confirm = Some(Confirm::RecentFiles);
    ui_events(
        &mut app,
        &ctx,
        vec![commit("late"), egui::Event::WindowFocused(false)],
    );
    assert!(app.editor.document().same_semantics(&original));
    assert_eq!(app.editor.canvas_text_content(), Some("private"));
    assert!(app.canvas_ime.binding.is_none());
}
