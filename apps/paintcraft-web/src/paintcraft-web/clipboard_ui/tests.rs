use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::{Action as Edit, Tool};
use std::sync::Arc;

fn fixture(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    app.editor = crate::editor::Editor::new(8, 6).unwrap();
    app.editor
        .apply(Edit::Select {
            corners: Some(((2, 1), (4, 3))),
        })
        .unwrap();
    app
}
#[test]
fn copy_is_read_only_deduplicated_and_only_success_replaces_internal_capture() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let selection = app.editor.session.selection();
    let history = app.editor.session.history_view();
    app.copy_with(&ctx, |image| {
        assert_eq!((image.width(), image.height()), (3, 3));
        assert_eq!(image.rgba(), vec![255; 36]);
        Ok(clipboard::Job::completed(Ok(())))
    });
    assert!(
        app.clipboard.last.is_none(),
        "an unconsumed acknowledgment is not success"
    );
    app.copy_with(&ctx, |_| {
        panic!("one key and Copy event in the same frame must not repeat")
    });
    app.poll_copy(&ctx);
    let retained = app.clipboard.last.as_ref().unwrap().image.clone();
    assert_eq!(app.clipboard.last.as_ref().unwrap().origin, (2, 1));
    let mut output = ctx.run_ui(Default::default(), |_| {});
    output.textures_delta.clear();
    app.copy_with(&ctx, |_| {
        Ok(clipboard::Job::completed(Err(Error::new(
            ErrorCode::InputSchema,
            "permission denied",
        ))))
    });
    app.poll_copy(&ctx);
    assert_eq!(
        app.clipboard.last.as_ref().unwrap().image.rgba(),
        retained.rgba()
    );
    assert!(
        app.message.contains("permission denied"),
        "{} activation {:?} frame {}",
        app.message,
        app.clipboard.activation,
        ctx.cumulative_frame_nr()
    );
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.selection(), selection);
    assert_eq!(app.editor.session.history_view(), history);
}
#[test]
fn a_draft_or_empty_selection_does_not_write_or_place_pixels() {
    for text in [false, true] {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx);
        let layer = app.editor.target.unwrap();
        if text {
            app.editor.tools.begin_text(layer, (1, 1));
        } else {
            app.editor.tools.pointer_down(layer, (1, 1));
        }
        let source = app.editor.session.snapshot();
        let preview = app.editor.tools.preview();
        app.copy_with(&ctx, |_| panic!("private drafts are not canvas copies"));
        assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
        assert_eq!(app.editor.tools.preview(), preview);
        assert!(app.clipboard.last.is_none());
    }
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.editor
        .apply(Edit::Select {
            corners: Some(((20, 20), (22, 22))),
        })
        .unwrap();
    app.copy_with(&ctx, |_| panic!("empty field must not write"));
    assert!(app.clipboard.job.is_none());
}
#[test]
fn copy_event_and_shortcut_leave_custom_text_input_in_control() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.editor.tools.activate_tool(Tool::Text);
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (1, 1));
    assert!(!app.keyboard_event(&ctx, &egui::Event::Copy));
    assert!(!app.keyboard_event(
        &ctx,
        &egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }
    ));
    assert!(app.clipboard.job.is_none());
    assert!(app.editor.tools.is_editing_text());
}

#[test]
fn merged_capture_works_without_an_active_layer_and_waits_for_acknowledgment() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.editor.target = None;
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let selection = app.editor.session.selection();
    app.copy_source_with(&ctx, true, |image| {
        assert_eq!((image.width(), image.height()), (3, 3));
        assert_eq!(image.rgba(), vec![255; 36]);
        Ok(clipboard::Job::completed(Ok(())))
    });
    assert!(app.clipboard.last.is_none());
    app.poll_copy(&ctx);
    assert_eq!(app.clipboard.last.as_ref().unwrap().origin, (2, 1));
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(ticket, app.editor.session.ticket());
    assert_eq!(selection, app.editor.session.selection());
}

#[test]
fn coalesced_merged_shortcut_keeps_merged_source_in_either_event_order() {
    for copy_first in [false, true] {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx);
        app.editor.target = None;
        let depth = app.editor.session.undo_depth();
        let key = egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        };
        let events = if copy_first {
            vec![egui::Event::Copy, key]
        } else {
            vec![key, egui::Event::Copy]
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                for event in ui.input(|input| input.events.clone()) {
                    assert!(app.keyboard_event(ui.ctx(), &event));
                }
            },
        );
        output.textures_delta.clear();
        // Native test hosts cannot write a browser clipboard. Reaching this
        // adapter proves merged copy was chosen despite the absent active layer.
        assert!(
            app.message.contains("Image clipboard is unavailable"),
            "{}",
            app.message
        );
        assert_eq!(
            app.clipboard.activation,
            Some(ctx.cumulative_frame_nr() - 1)
        );
        assert_eq!(app.editor.session.undo_depth(), depth);
    }
}

mod cut;
