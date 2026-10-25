use crate::{
    WebApp,
    editor::{Editor, WHITE},
};
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Vec2};
use egui_kittest::Harness;
use paintcraft_classic::ClassicVariant;
use paintcraft_core::types::Rgba;
use paintcraft_raster::Selection;
use paintcraft_tools::{Action, BlendMode, Stroke, Tool};
use paintcraft_ui::{
    actions::Action as UiAction,
    i18n::{Language, set_language},
};
use std::sync::Arc;

fn setup(keyed: bool) -> WebApp {
    let mut app = WebApp::new(&egui::Context::default());
    app.editor = Editor::new(32, 24).unwrap();
    let layer = app.editor.target.unwrap();
    for (point, color) in [
        ((5, 5), Rgba([220, 30, 40, 255])),
        ((6, 6), Rgba([17, 33, 55, 0])),
        ((18, 6), Rgba([30, 170, 80, 255])),
        ((10, 9), Rgba([190, 130, 40, 255])),
    ] {
        app.editor
            .apply(Action::Stroke(Stroke {
                layer,
                points: vec![point],
                point_scale: 1,
                color,
                size: 1,
                round: false,
                media: Default::default(),
                mode: BlendMode::Replace,
            }))
            .unwrap();
    }
    app.editor
        .apply(Action::SelectPolygon {
            points: vec![
                (4, 4),
                (16, 4),
                (16, 14),
                (4, 14),
                (4, 4),
                (8, 7),
                (8, 11),
                (12, 11),
                (12, 7),
                (8, 7),
                (4, 4),
            ],
        })
        .unwrap();
    app.editor.session.mark_saved();
    app.controls.tool = Tool::Select;
    app.controls.options.transparent_selection = keyed;
    app.controls.secondary = WHITE;
    app
}
fn harness(app: WebApp) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360., 900.))
        .build_eframe(move |cc| {
            let mut host = WebApp::new(&cc.egui_ctx);
            host.editor = app.editor;
            host.controls = app.controls;
            host.variant = app.variant;
            host
        });
    h.run();
    h
}
fn pos(h: &Harness<WebApp>, x: i64, y: i64) -> Pos2 {
    h.state().canvas.rect.unwrap().min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * h.state().zoom
}
fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
fn start(h: &mut Harness<WebApp>) {
    let point = pos(h, 5, 5);
    h.event(Event::PointerMoved(point));
    h.event(button(point, PointerButton::Primary, true));
    h.run();
    assert!(h.state().editor.tools.floating_selection().is_some());
}
fn pixels(doc: &paintcraft_document::Document, layer: uuid::Uuid) -> Vec<Rgba> {
    let l = &doc.layers[&layer];
    (0..24)
        .flat_map(|y| (0..32).map(move |x| l.raster.get(x - l.origin.0, y - l.origin.1)))
        .collect()
}
fn selected(x: i64, y: i64) -> bool {
    (4..16).contains(&x) && (4..14).contains(&y) && !((8..12).contains(&x) && (7..11).contains(&y))
}
fn moved(original: &[Rgba], keyed: bool) -> Vec<Rgba> {
    let mut result = original.to_vec();
    for y in 0..24 {
        for x in 0..32 {
            if selected(x, y) {
                result[(y * 32 + x) as usize] = WHITE;
            }
        }
    }
    for y in 0..24 {
        for x in 0..32 {
            if selected(x, y) {
                let pixel = original[(y * 32 + x) as usize];
                if !keyed || pixel.0[..3] != WHITE.0[..3] {
                    result[((y + 2) * 32 + x + 4) as usize] = pixel;
                }
            }
        }
    }
    result
}

#[test]
fn browser_floating_preview_then_release_matches_frozen_pixels_in_all_skins_and_two_languages() {
    for variant in ClassicVariant::ALL {
        for language in [Language::English, Language::Japanese] {
            for keyed in [false, true] {
                let mut app = setup(keyed);
                app.variant = variant;
                let mut h = harness(app);
                set_language(&h.ctx, language);
                h.run();
                let layer = h.state().editor.target.unwrap();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                let selection = h.state().editor.session.selection();
                let depth = h.state().editor.session.undo_depth();
                let original = pixels(&before, layer);
                let expected = moved(&original, keyed);
                let hover = pos(&h, 5, 5);
                h.event(Event::PointerMoved(hover));
                h.run();
                assert_eq!(
                    h.output().platform_output.cursor_icon,
                    egui::CursorIcon::Move
                );
                start(&mut h);
                assert!(
                    h.state().editor.is_dirty(),
                    "active floating draft participates in unload guard"
                );
                let point = pos(&h, 9, 7);
                h.event(Event::PointerMoved(point));
                h.run();
                assert_eq!(h.state().editor.session.ticket(), ticket);
                assert_eq!(h.state().editor.session.undo_depth(), depth);
                assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                assert_eq!(h.state().editor.session.selection(), selection);
                let preview = h.state().editor.display_document().unwrap().unwrap();
                assert_eq!(pixels(&preview, layer), expected);
                let Selection::Mask(field) = h.state().editor.display_selection().unwrap() else {
                    panic!("moving mask");
                };
                for y in 0..24 {
                    for x in 0..32 {
                        assert_eq!(field.coverage(x, y) != 0, selected(x - 4, y - 2));
                    }
                }
                assert_eq!(h.state().selected_dimensions(), Some((12, 10)));
                // Preferences after the press do not change the captured Color 2 key.
                h.state_mut().controls.secondary = Rgba([220, 30, 40, 255]);
                h.state_mut().controls.options.transparent_selection = !keyed;
                h.event(button(point, PointerButton::Secondary, false));
                h.run();
                assert!(h.state().editor.tools.floating_selection().is_some());
                assert_eq!(h.state().editor.session.ticket(), ticket);
                h.event(button(point, PointerButton::Primary, false));
                h.run();
                assert_eq!(h.state().editor.session.undo_depth(), depth + 1);
                assert_eq!(pixels(h.state().editor.session.document(), layer), expected);
                assert!(h.state().editor.tools.floating_selection().is_none());
                let committed = h.state().editor.session.ticket();
                h.event(button(point, PointerButton::Primary, false));
                h.run();
                assert_eq!(h.state().editor.session.ticket(), committed);
                h.state_mut().editor.apply(Action::Undo).unwrap();
                h.run();
                assert!(h.state().editor.session.document().same_semantics(&before));
                assert_eq!(h.state().editor.session.selection(), selection);
                h.state_mut().editor.apply(Action::Redo).unwrap();
                h.run();
                assert_eq!(pixels(h.state().editor.session.document(), layer), expected);
            }
        }
    }
}

#[test]
fn floating_escape_focus_loss_zero_offset_and_hole_press_preserve_original_publication() {
    for event in [
        Event::PointerGone,
        Event::WindowFocused(false),
        Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    ] {
        let mut h = harness(setup(true));
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let selection = h.state().editor.session.selection();
        start(&mut h);
        let point = pos(&h, 9, 7);
        h.event(Event::PointerMoved(point));
        h.run();
        h.event(event);
        h.run();
        assert!(h.state().editor.tools.floating_selection().is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert_eq!(h.state().editor.session.selection(), selection);
    }
    let mut h = harness(setup(true));
    let ticket = h.state().editor.session.ticket();
    let depth = h.state().editor.session.undo_depth();
    start(&mut h);
    let point = pos(&h, 5, 5);
    h.event(button(point, PointerButton::Primary, false));
    h.run();
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert_eq!(h.state().editor.session.undo_depth(), depth);
    assert!(!h.state().editor.is_dirty());
    let point = pos(&h, 9, 8);
    h.event(Event::PointerMoved(point));
    h.event(button(point, PointerButton::Primary, true));
    h.run();
    assert!(
        h.state().editor.tools.floating_selection().is_none(),
        "hole coverage does not start a move"
    );
    assert!(
        h.state().editor.tools.gesture_is_selection(),
        "hole starts a new ordinary selection"
    );
}

#[test]
fn floating_stale_source_session_tool_target_and_file_actions_never_publish_old_drag() {
    for invalidation in 0..4 {
        let mut h = harness(setup(false));
        start(&mut h);
        let point = pos(&h, 9, 7);
        h.event(Event::PointerMoved(point));
        h.run();
        match invalidation {
            0 => {
                h.state_mut().editor.session.undo().unwrap();
            }
            1 => {
                let doc = (*h.state().editor.session.snapshot()).clone();
                h.state_mut().editor.session = paintcraft_editor::session::Session::new(doc);
            }
            2 => h.state_mut().controls.tool = Tool::Pencil,
            _ => h.state_mut().editor.target = None,
        }
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        h.event(button(point, PointerButton::Primary, false));
        h.run();
        assert!(h.state().editor.tools.floating_selection().is_none());
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
    }
    let mut h = harness(setup(true));
    start(&mut h);
    let point = pos(&h, 9, 7);
    h.event(Event::PointerMoved(point));
    h.run();
    let before = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    assert!(!h.state_mut().floating_file_ready());
    assert_eq!(h.state().message, crate::floating_ui::FINISH_MOVE);
    assert!(h.state().editor.tools.floating_selection().is_some());
    assert!(h.state().editor.copy_snapshot().is_err());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    assert_eq!(h.state().editor.session.ticket(), ticket);
    // The host-level download path has the same guard before it requests a download.
    let context = h.ctx.clone();
    h.state_mut().download(&context, false);
    assert!(!h.state().files.busy);
    assert!(h.state().editor.tools.floating_selection().is_some());
    for action in [
        UiAction::Save,
        UiAction::Export,
        UiAction::New,
        UiAction::Open,
    ] {
        h.state_mut().action(&context, action);
        assert!(h.state().editor.tools.floating_selection().is_some());
        assert!(!h.state().files.busy);
        assert!(h.state().export_confirm.is_none());
        assert!(h.state().confirm_replace.is_none());
    }
}

#[test]
fn identical_ticket_and_rect_in_a_replaced_session_still_rejects_pinned_source_and_late_action() {
    let initial = setup(false);
    let document = (*initial.editor.session.snapshot()).clone();
    let make = || {
        let mut editor = Editor::from_document(document.clone()).unwrap();
        editor
            .apply(Action::Select {
                corners: Some(((4, 4), (15, 13))),
            })
            .unwrap();
        editor.tools.tool = Tool::Select;
        editor
    };
    let mut editor = make();
    let bounds = paintcraft_core::types::Rect::new(4, 4, 12, 10).unwrap();
    assert!(
        editor
            .begin_floating(bounds, (5, 5), paintcraft_tools::PointerButton::Primary)
            .unwrap()
    );
    editor.tools.pointer_move((9, 7));
    assert!(editor.display_document().unwrap().is_some());
    let replacement = make();
    assert_eq!(editor.session.ticket(), replacement.session.ticket());
    assert_eq!(editor.session.selection(), replacement.session.selection());
    assert!(!Arc::ptr_eq(
        &editor.session.snapshot(),
        &replacement.session.snapshot()
    ));
    editor.session = replacement.session;
    let snapshot = editor.session.snapshot();
    let ticket = editor.session.ticket();
    assert!(editor.display_document().is_err());
    let (_, action) = editor.tools.pointer_up().unwrap();
    assert!(editor.apply(action).is_err());
    assert!(Arc::ptr_eq(&snapshot, &editor.session.snapshot()));
    assert_eq!(editor.session.ticket(), ticket);
}

#[test]
fn unfinished_text_busy_import_and_secondary_press_cannot_start_a_floating_move() {
    for blocked in 0..3 {
        let mut h = harness(setup(false));
        let source = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        match blocked {
            0 => {
                let layer = h.state().editor.target.unwrap();
                h.state_mut().editor.tools.begin_text(layer, (2, 2));
                h.state_mut().editor.tools.type_text("Keep editable");
            }
            1 => h.state_mut().files.busy = true,
            _ => {
                let p = pos(&h, 5, 5);
                h.event(Event::PointerMoved(p));
                h.event(button(p, PointerButton::Secondary, true));
                h.run();
                assert!(h.state().editor.tools.floating_selection().is_none());
                continue;
            }
        }
        let p = pos(&h, 5, 5);
        h.event(Event::PointerMoved(p));
        h.event(button(p, PointerButton::Primary, true));
        h.run();
        assert!(h.state().editor.tools.floating_selection().is_none());
        assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
        assert_eq!(h.state().editor.session.ticket(), ticket);
        if blocked == 0 {
            assert_eq!(h.state().editor.tools.text_content(), Some("Keep editable"));
        }
    }
}
