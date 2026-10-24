use super::*;
use paintcraft_core::types::canvas::Anchor;
use std::time::Instant;

fn frame(app: &mut App, ctx: &egui::Context, escape: bool) {
    let Some(Confirm::Canvas(draft)) = app.confirm.clone() else {
        panic!("canvas dialog")
    };
    let mut input = egui::RawInput::default();
    if escape {
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    let mut output = ctx.run_ui(input, |ui| {
        app.canvas_settings_dialog(ui.ctx(), draft.clone())
    });
    output.textures_delta.clear();
}
fn drain(app: &mut App, ctx: &egui::Context) {
    let end = Instant::now() + Duration::from_secs(10);
    while app.editor.has_pending_edits() {
        assert!(Instant::now() < end, "editing stalled");
        app.poll_edits(ctx);
        std::thread::yield_now();
    }
}
#[test]
fn studio_canvas_waits_for_earlier_edits_before_capturing_dimensions_and_source_revision() {
    let ctx = egui::Context::default();
    paintcraft_ui::fonts::install(&ctx);
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.context = ctx.clone();
    app.initialize_editing(ctx.clone());
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(
        11,
        Action::ResizeCanvasAnchored {
            width: 600,
            height: 420,
            anchor: Anchor::BottomRight,
        },
    );
    let preferences = app.settings.canvas.clone();
    app.begin_canvas_dialog(&ctx, false);
    assert!(matches!(
        &app.confirm,
        Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio {
            revision: None,
            ..
        }))
    ));
    frame(&mut app, &ctx, false);
    assert!(matches!(
        &app.confirm,
        Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio {
            revision: None,
            ..
        }))
    ));
    release.send(()).unwrap();
    drain(&mut app, &ctx);
    assert_eq!(app.editor.document().canvas, (600, 420));
    frame(&mut app, &ctx, false);
    let Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio { revision, view, .. })) =
        &app.confirm
    else {
        panic!("ready canvas dialog")
    };
    assert_eq!(*revision, Some(app.editor.render_key()));
    assert_eq!(view.size, (600, 420));
    assert_eq!(view.anchor, Anchor::Center);
    assert_eq!(app.settings.canvas, preferences);
}
#[test]
fn cancelling_the_wait_does_not_discard_the_earlier_authored_edit() {
    let ctx = egui::Context::default();
    paintcraft_ui::fonts::install(&ctx);
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.context = ctx.clone();
    app.initialize_editing(ctx.clone());
    let release = app.editing.as_ref().unwrap().pause();
    app.editor.request(
        12,
        Action::ResizeCanvasAnchored {
            width: 600,
            height: 420,
            anchor: Anchor::Center,
        },
    );
    app.begin_canvas_dialog(&ctx, false);
    frame(&mut app, &ctx, false); // Display the modal before sending keyboard input.
    frame(&mut app, &ctx, true);
    assert!(app.confirm.is_none());
    release.send(()).unwrap();
    drain(&mut app, &ctx);
    assert_eq!(app.editor.document().canvas, (600, 420));
    assert_eq!(app.editor.undo_depth(), 1);
}
#[test]
fn studio_canvas_rejects_changed_documents_tabs_and_unpublished_edits() {
    for cause in 0..3 {
        let ctx = egui::Context::default();
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.begin_canvas_dialog(&ctx, false);
        let Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio {
            mut tab,
            revision,
            view,
        })) = app.confirm.clone()
        else {
            panic!("canvas dialog")
        };
        match cause {
            0 => {
                app.editor.request(17, Action::AddLayer);
                app.editor.run_pending();
            }
            1 => tab = Uuid::new_v4(),
            _ => {
                app.editor.request(17, Action::AddLayer);
            }
        }
        let before = app.editor.document_snapshot();
        let history = app.editor.history_view().ticket;
        app.studio_canvas_settings_dialog(&ctx, tab, revision, view);
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&before));
        assert_eq!(app.editor.history_view().ticket, history);
        assert!(app.editor.status().contains("source document changed"));
    }
}
