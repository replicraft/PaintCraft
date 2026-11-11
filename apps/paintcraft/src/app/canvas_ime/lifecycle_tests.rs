use super::*;
#[expect(
    deprecated,
    reason = "explicit owned-loop compatibility lifecycle regression"
)]
fn facility(enabled: bool) -> egui::Event {
    egui::Event::Ime(if enabled {
        egui::ImeEvent::Enabled
    } else {
        egui::ImeEvent::Disabled
    })
}
#[test]
fn disabled_discards_only_provisional_text_and_late_commit_then_fresh_starts_recover() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("Base ");
    let original = app.editor.document_snapshot();
    let history = app.editor.history_view().ticket;
    let publications = app.editor.publications();
    event(&mut app, &ctx, preedit("candidate"));
    assert_eq!(preview_text(&app), "Base candidate");
    event(&mut app, &ctx, facility(false));
    assert!(app.canvas_ime.binding.is_none());
    assert!(!app.ime_composing);
    event(&mut app, &ctx, commit("late"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base "));
    assert_eq!(app.editor.history_view().ticket, history);
    assert_eq!(app.editor.publications(), publications);
    assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
    event(&mut app, &ctx, facility(true));
    assert!(app.canvas_ime.binding.is_none());
    assert!(!app.ime_composing);
    event(&mut app, &ctx, egui::Event::Text("plain ".into()));
    event(&mut app, &ctx, commit("direct"));
    event(&mut app, &ctx, commit(" result"));
    assert_eq!(
        app.editor.canvas_text_content(),
        Some("Base plain direct result")
    );
    event(&mut app, &ctx, facility(false));
    event(&mut app, &ctx, preedit("next"));
    event(&mut app, &ctx, commit(" fresh"));
    assert_eq!(
        app.editor.canvas_text_content(),
        Some("Base plain direct result fresh")
    );
    assert_eq!(app.editor.publications(), publications);
}
#[test]
fn initial_direct_commit_compatibility_and_duplicate_enabled_do_not_erase_preedit() {
    let mut app = app();
    let ctx = egui::Context::default();
    event(&mut app, &ctx, commit("initial"));
    event(&mut app, &ctx, facility(true));
    event(&mut app, &ctx, preedit(" provisional"));
    event(&mut app, &ctx, facility(true));
    assert_eq!(preview_text(&app), "initial provisional");
    event(&mut app, &ctx, commit(" accepted"));
    assert_eq!(app.editor.canvas_text_content(), Some("initial accepted"));
}
#[test]
fn same_content_replacement_session_or_draft_cannot_reuse_old_enabled_scope() {
    for replace_session in [false, true] {
        let mut app = app();
        let ctx = egui::Context::default();
        app.editor.type_text("Same");
        event(&mut app, &ctx, facility(true));
        if replace_session {
            app.editor = Editor::new_document(100, 100).unwrap();
            app.editor.select_tool(Tool::Text);
        } else {
            app.editor.cancel_text();
        }
        app.editor.pointer_down((8, 8));
        app.editor.type_text("Same");
        let original = app.editor.document_snapshot();
        let before = app.editor.publications();
        event(&mut app, &ctx, commit("late"));
        assert_eq!(app.editor.canvas_text_content(), Some("Same"));
        assert!(app.canvas_ime.enabled.is_none());
        assert_eq!(app.editor.publications(), before);
        assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
        event(&mut app, &ctx, facility(true));
        event(&mut app, &ctx, commit(" new"));
        assert_eq!(app.editor.canvas_text_content(), Some("Same new"));
    }
}
#[test]
fn modal_and_focus_revoke_facility_without_making_text_or_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("Base");
    let before = app.editor.publications();
    event(&mut app, &ctx, facility(true));
    app.confirm = Some(Confirm::RecentFiles);
    assert!(!app.canvas_text_event(&ctx, &commit("late")));
    assert!(app.canvas_ime.enabled.is_none());
    app.confirm = None;
    event(&mut app, &ctx, commit("later"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base"));
    event(&mut app, &ctx, facility(true));
    ctx.input_mut(|i| i.focused = false);
    assert!(!app.canvas_text_event(&ctx, &facility(true)));
    assert!(app.canvas_ime.enabled.is_none());
    assert!(!app.canvas_text_event(&ctx, &commit("unfocused")));
    ctx.input_mut(|i| i.focused = true);
    event(&mut app, &ctx, commit("old"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base"));
    assert_eq!(app.editor.publications(), before);
    event(&mut app, &ctx, facility(true));
    event(&mut app, &ctx, commit(" new"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base new"));
}
#[test]
fn widget_owned_facility_does_not_arm_canvas_commit_after_widget_releases_focus() {
    for previously_armed in [false, true] {
        let mut app = app();
        let ctx = egui::Context::default();
        app.editor.type_text("Base");
        if previously_armed {
            event(&mut app, &ctx, facility(true));
        }
        let id = egui::Id::unique("ime-widget-owner");
        let mut text = String::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).id(id))
                .request_focus();
        });
        output.textures_delta.clear();
        assert!(ctx.egui_wants_keyboard_input());
        assert!(!app.canvas_text_event(&ctx, &facility(true)));
        assert!(app.canvas_ime.enabled.is_none());
        ctx.memory_mut(|m| m.surrender_focus(id));
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        event(&mut app, &ctx, commit("late"));
        assert_eq!(app.editor.canvas_text_content(), Some("Base"));
    }
}
#[test]
fn ordered_ui_disabled_then_enabled_result_publishes_only_real_text_and_undoes_once() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.editor.type_text("Base");
    let original = app.editor.document_snapshot();
    let before = app.editor.publications();
    let depth = app.editor.undo_depth();
    let expected = expected_text(&app, "Base new");
    ui_events(
        &mut app,
        &ctx,
        vec![
            facility(true),
            preedit(" discarded"),
            facility(false),
            commit(" late"),
            facility(true),
            commit(" new"),
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
        ],
    );
    assert_eq!(app.editor.publications(), before + 1);
    assert_eq!(app.editor.undo_depth(), depth + 1);
    assert!(app.editor.document().same_semantics(&expected));
    assert!(!app.editor.is_editing_text());
    app.editor.request(918701, Action::Undo);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&original));
}

#[test]
fn initially_unarmed_ineligible_facility_cannot_authorize_a_later_new_canvas_draft() {
    let mut app = app();
    let ctx = egui::Context::default();
    ctx.input_mut(|input| input.focused = true);
    app.editor.cancel_text();
    assert!(!app.canvas_text_event(&ctx, &facility(true)));
    app.editor.pointer_down((8, 8));
    app.editor.type_text("Base");
    event(&mut app, &ctx, commit("foreign late result"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base"));
    event(&mut app, &ctx, facility(true));
    event(&mut app, &ctx, commit(" actual"));
    assert_eq!(app.editor.canvas_text_content(), Some("Base actual"));
}

#[test]
fn actual_same_frame_lifecycle_order_controls_shortcut_exclusion_at_each_key() {
    for case in 0..3 {
        let mut app = app();
        let ctx = egui::Context::default();
        app.editor.type_text("Base");
        let original = app.editor.document_snapshot();
        let before = app.editor.publications();
        let fullscreen = app.fullscreen;
        let mut events = match case {
            0 => vec![preedit(" candidate"), facility(false)],
            1 => vec![preedit(" candidate"), facility(true)],
            2 => vec![facility(false), preedit(" new candidate")],
            _ => unreachable!(),
        };
        events.push(egui::Event::Key {
            key: egui::Key::F11,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
        ui_events(&mut app, &ctx, events);
        assert_eq!(
            app.fullscreen,
            if case == 0 { !fullscreen } else { fullscreen }
        );
        assert_eq!(app.editor.canvas_text_content(), Some("Base"));
        assert_eq!(app.editor.publications(), before);
        assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
        assert_eq!(app.canvas_ime.binding.is_some(), case != 0);
    }
}
