//! Actual layer-name widget reproduction, not native OS/IME qualification.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

struct State {
    app: App,
    outside: String,
    other_first: bool,
}
fn harness() -> Harness<'static, State> {
    harness_with(Experience::Studio)
}
fn harness_with(experience: Experience) -> Harness<'static, State> {
    let mut app = App::new(None);
    app.settings.experience = experience;
    Harness::builder()
        .with_size(Vec2::new(700.0, 800.0))
        .with_max_steps(20)
        .build_ui_state(
            |ui, state: &mut State| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                paintcraft_ui::i18n::set_language(ui.ctx(), paintcraft_ui::i18n::Language::English);
                state.app.prepare_name_input(ui.ctx());
                if state.other_first {
                    outside(ui, &mut state.outside);
                }
                state.app.layer_panel(ui);
                if !state.other_first {
                    outside(ui, &mut state.outside);
                }
                state.app.editor.run_pending();
            },
            State {
                app,
                outside: String::new(),
                other_first: false,
            },
        )
}
fn outside(ui: &mut egui::Ui, text: &mut String) {
    let label = ui.label("Other input");
    ui.text_edit_singleline(text).labelled_by(label.id);
}
fn name(h: &Harness<'_, State>) -> String {
    let app = &h.state().app;
    app.editor.document().layers[&app.editor.target().unwrap()]
        .name
        .clone()
}
fn begin(h: &mut Harness<'_, State>) -> (String, usize, u64) {
    h.run();
    let before = name(h);
    let history = h.state().app.editor.undo_depth();
    let publications = h.state().app.editor.publications();
    h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
        .click();
    h.run();
    let id = h
        .ctx
        .memory(|m| m.focused())
        .expect("actual focused name widget");
    let mut state = egui::TextEdit::load_state(&h.ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(before.chars().count()),
        )));
    state.store(&h.ctx, id);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "候補".into(),
            active_range_chars: Some(0..2),
        }));
    h.run();
    assert_eq!(h.state().app.name_edit.as_ref().unwrap().1, before);
    assert_eq!(
        h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("候補")
    );
    assert!(
        h.output().platform_output.ime.is_some(),
        "actual TextEdit publishes IME caret"
    );
    assert_eq!(name(h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
    (before, history, publications)
}
fn blur(h: &mut Harness<'_, State>) {
    h.get_by_role_and_label(egui::Role::TextInput, "Other input")
        .click();
    h.run();
}

#[test]
fn cancelled_preedit_without_commit_must_not_publish_layer_rename() {
    let mut h = harness();
    let (before, history, publications) = begin(&mut h);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        }));
    h.run();
    blur(&mut h);
    eprintln!(
        "before={before:?} after={:?} undo_before={history} undo_after={} publications_before={publications} publications_after={}",
        name(&h),
        h.state().app.editor.undo_depth(),
        h.state().app.editor.publications()
    );
    assert_eq!(name(&h), before, "no actual IME Commit was delivered");
    assert_eq!(h.state().app.editor.undo_depth(), history);
    assert_eq!(h.state().app.editor.publications(), publications);
}

#[test]
fn focusloss_without_commit_must_not_publish_provisional_layer_name() {
    let mut h = harness();
    let (before, history, publications) = begin(&mut h);
    blur(&mut h);
    eprintln!(
        "before={before:?} after={:?} undo_before={history} undo_after={} publications_before={publications} publications_after={}",
        name(&h),
        h.state().app.editor.undo_depth(),
        h.state().app.editor.publications()
    );
    assert_eq!(name(&h), before, "preedit alone cannot authorize Rename");
    assert_eq!(h.state().app.editor.undo_depth(), history);
    assert_eq!(h.state().app.editor.publications(), publications);
}

#[test]
fn actual_ime_commit_then_focusloss_publishes_one_layer_rename() {
    let mut h = harness();
    let (_, history, publications) = begin(&mut h);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit("確定".into())));
    h.run();
    blur(&mut h);
    assert_eq!(name(&h), "確定");
    assert!(
        h.state().outside.is_empty(),
        "old result must not reach the new input"
    );
    assert_eq!(h.state().app.editor.undo_depth(), history + 1);
    assert_eq!(h.state().app.editor.publications(), publications + 1);
}

#[test]
fn actual_ime_commit_before_sameframe_focuschange_preserves_result() {
    let mut h = harness();
    let (_, history, publications) = begin(&mut h);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit("確定".into())));
    // Harness::event steps events individually. Use one actual RawInput batch.
    let at = h
        .get_by_role_and_label(egui::Role::TextInput, "Other input")
        .rect()
        .center();
    h.input_mut().events.extend([
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.run();
    assert_eq!(name(&h), "確定");
    assert!(
        h.state().outside.is_empty(),
        "old result must not reach the new input"
    );
    assert_eq!(h.state().app.editor.undo_depth(), history + 1);
    assert_eq!(h.state().app.editor.publications(), publications + 1);
}

#[test]
fn ordinary_nonime_name_edit_keeps_document_rename_and_undo() {
    let mut h = harness();
    h.run();
    let before = name(&h);
    h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
        .click();
    h.run();
    let id = h.ctx.memory(|m| m.focused()).unwrap();
    let mut state = egui::TextEdit::load_state(&h.ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(before.chars().count()),
        )));
    state.store(&h.ctx, id);
    h.input_mut()
        .events
        .push(egui::Event::Text("Ordinary rename".into()));
    h.run();
    blur(&mut h);
    assert_eq!(name(&h), "Ordinary rename");
    assert_eq!(h.state().app.editor.undo_depth(), 1);
    h.state_mut().app.editor.request(9123, Action::Undo);
    h.state_mut().app.editor.run_pending();
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), 0);
}

#[test]
fn explicit_escape_discards_preedit_name_draft_without_rename() {
    let mut h = harness();
    let (before, history, publications) = begin(&mut h);
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    blur(&mut h);
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
    assert_eq!(h.state().app.editor.publications(), publications);
}

// Characterize the selected opaque history before attempting a clone admission
// bound. Default max_undos does not trim redo()'s push into the undo deque.
#[test]
fn pinned_undoer_redo_cycles_can_exceed_default_history_limit() {
    let mut undoer = egui::util::undoer::Undoer::<String>::default();
    let mut current = String::from("baseline");
    undoer.feed_state(0.0, &current);
    for i in 1..=150 {
        let next = format!("edit {i}");
        undoer.feed_state(f64::from(i) * 0.001, &next);
        let previous = undoer.undo(&next).unwrap().clone();
        current = undoer.redo(&previous).unwrap().clone();
        assert_eq!(current, next);
    }
    let mut count = 0;
    while let Some(previous) = undoer.undo(&current).cloned() {
        current = previous;
        count += 1;
    }
    assert_eq!(count, 150);
    assert_eq!(current, "baseline");
}

#[test]
fn actual_commit_then_new_preedit_sameframe_keeps_only_result_on_blur_and_undo() {
    let mut h = harness();
    let (before, history, publications) = begin(&mut h);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit("確定".into())));
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "次候補".into(),
            active_range_chars: Some(0..3),
        }));
    h.run();
    assert_eq!(h.state().app.name_edit.as_ref().unwrap().1, "確定");
    assert_eq!(
        h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("確定次候補")
    );
    assert_eq!(name(&h), before);
    blur(&mut h);
    assert_eq!(name(&h), "確定");
    assert!(
        h.state().outside.is_empty(),
        "old result must not reach the new input"
    );
    assert_eq!(h.state().app.editor.undo_depth(), history + 1);
    assert_eq!(h.state().app.editor.publications(), publications + 1);
    h.state_mut().app.editor.request(9921, Action::Undo);
    h.state_mut().app.editor.run_pending();
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
}

#[test]
fn replacement_cancel_restores_full_selection_and_original_widget_history() {
    let mut h = harness();
    h.run();
    let before = name(&h);
    h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
        .click();
    h.run();
    let id = h.ctx.memory(|m| m.focused()).unwrap();
    let range =
        egui::text::CCursorRange::two(egui::text::CCursor::new(1), egui::text::CCursor::new(4));
    let mut state = egui::TextEdit::load_state(&h.ctx, id).unwrap();
    state.cursor.set_char_range(Some(range));
    let undo_before = state.undoer().undo(&(range, before.clone())).cloned();
    state.store(&h.ctx, id);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "候補".into(),
            active_range_chars: Some(0..2),
        }));
    h.run();
    assert_eq!(
        h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("B候補ground")
    );
    assert_eq!(h.state().app.name_edit.as_ref().unwrap().1, before);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        }));
    h.run();
    blur(&mut h);
    assert_eq!(name(&h), before);
    let restored = egui::TextEdit::load_state(&h.ctx, id).unwrap();
    assert_eq!(restored.cursor.char_range(), Some(range));
    assert_eq!(
        restored.undoer().undo(&(range, before)).cloned(),
        undo_before
    );
    assert_eq!(h.state().app.editor.undo_depth(), 0);
}

#[test]
fn actual_commit_then_enter_sameframe_publishes_one_result() {
    let mut h = harness();
    let (_, history, publications) = begin(&mut h);
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit("確定".into())));
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(name(&h), "確定");
    assert!(
        h.state().outside.is_empty(),
        "old result must not reach the new input"
    );
    assert_eq!(h.state().app.editor.undo_depth(), history + 1);
    assert_eq!(h.state().app.editor.publications(), publications + 1);
}

#[test]
fn plain_h_key_during_preedit_preserves_visible_composition_without_rename() {
    let mut h = harness();
    let (before, history, _) = begin(&mut h);
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::H,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "新候補".into(),
            active_range_chars: Some(0..3),
        }));
    h.run();
    assert_eq!(
        h.get_by_role_and_label(egui::Role::TextInput, "Layer name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("新候補")
    );
    assert_eq!(h.state().app.name_edit.as_ref().unwrap().1, before);
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
    assert!(h.query_by_label("Edit cancelled").is_none());
}

#[test]
fn ctrl_h_with_preedit_explicitly_refuses_ime_and_keeps_ordinary_editor_usable() {
    let mut h = harness();
    let (before, history, _) = begin(&mut h);
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::H,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    });
    h.input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "新候補".into(),
            active_range_chars: Some(0..3),
        }));
    h.run();
    h.get_by_label("Edit cancelled");
    assert!(!h.state().app.name_edit.as_ref().unwrap().1.contains("候補"));
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
    let id = h.ctx.memory(|m| m.focused()).unwrap();
    let mut state = egui::TextEdit::load_state(&h.ctx, id).unwrap();
    let len = h.state().app.name_edit.as_ref().unwrap().1.chars().count();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(len),
        )));
    state.store(&h.ctx, id);
    h.input_mut()
        .events
        .push(egui::Event::Text("Ordinary after refusal".into()));
    h.run();
    blur(&mut h);
    assert_eq!(name(&h), "Ordinary after refusal");
}

#[test]
fn classic_name_preedit_focusloss_preserves_authoritative_name_and_history() {
    let mut h = harness_with(Experience::Classic);
    let (before, history, publications) = begin(&mut h);
    blur(&mut h);
    assert_eq!(name(&h), before);
    assert_eq!(h.state().app.editor.undo_depth(), history);
    assert_eq!(h.state().app.editor.publications(), publications);
}

#[test]
fn earlier_commit_is_not_stolen_by_an_input_rendered_before_name() {
    let mut h = harness();
    h.state_mut().other_first = true;
    let (_, history, publications) = begin(&mut h);
    let at = h
        .get_by_role_and_label(egui::Role::TextInput, "Other input")
        .rect()
        .center();
    h.input_mut().events.extend([
        egui::Event::Ime(egui::ImeEvent::Commit("確定".into())),
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.run();
    assert_eq!(name(&h), "確定");
    assert!(
        h.state().outside.is_empty(),
        "old result reached a different input"
    );
    assert_eq!(h.state().app.editor.undo_depth(), history + 1);
    assert_eq!(h.state().app.editor.publications(), publications + 1);
}

#[test]
fn pointer_before_result_cannot_rename_or_retarget_the_old_composition() {
    for other_first in [false, true] {
        let mut h = harness();
        h.state_mut().other_first = other_first;
        let (before, history, publications) = begin(&mut h);
        let at = h
            .get_by_role_and_label(egui::Role::TextInput, "Other input")
            .rect()
            .center();
        h.input_mut().events.extend([
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Ime(egui::ImeEvent::Commit("old result".into())),
        ]);
        h.run();
        assert_eq!(name(&h), before);
        assert_eq!(h.state().app.editor.undo_depth(), history);
        assert_eq!(h.state().app.editor.publications(), publications);
        assert!(
            h.state().outside.is_empty(),
            "old result reached a replacement input"
        );
    }
}

#[test]
fn confirmed_result_precedes_window_focus_loss_but_preedit_alone_cannot_publish() {
    for commit in [false, true] {
        let mut h = harness();
        let (before, history, publications) = begin(&mut h);
        if commit {
            h.input_mut()
                .events
                .push(egui::Event::Ime(egui::ImeEvent::Commit("確定".into())));
        }
        h.input_mut().events.push(egui::Event::WindowFocused(false));
        h.input_mut().focused = false;
        h.run();
        assert_eq!(name(&h), if commit { "確定" } else { &before });
        assert_eq!(
            h.state().app.editor.undo_depth(),
            history + usize::from(commit)
        );
        assert_eq!(
            h.state().app.editor.publications(),
            publications + u64::from(commit)
        );
    }
}
