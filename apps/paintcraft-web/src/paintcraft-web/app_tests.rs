use crate::WebApp;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_tools::Tool;

fn harness() -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h
}
fn shortcut(h: &mut Harness<WebApp>, modifiers: Modifiers, key: Key) {
    h.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    h.step();
    h.event(Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers,
    });
    h.step();
}
fn button(h: &mut Harness<WebApp>, pressed: bool) {
    let (w, hgt) = h.state().editor.session.document().canvas;
    let pos = h.state().canvas.rect.unwrap().min
        + Vec2::new((w / 2) as f32 + 0.5, (hgt / 2) as f32 + 0.5) * h.state().zoom;
    h.event(Event::PointerMoved(pos));
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.step();
}

#[test]
fn canvas_publishes_once_on_release_and_keyboard_undo_redo_works() {
    let mut h = harness();
    h.state_mut().controls.tool = Tool::Brush;
    let before = h.state().editor.session.ticket();
    button(&mut h, true);
    assert!(h.state().editor.tools.gesture_in_progress());
    assert_eq!(h.state().editor.session.ticket(), before);
    button(&mut h, false);
    assert_eq!(h.state().editor.session.undo_depth(), 1);
    let painted = h.state().editor.session.ticket().state_id;
    assert_ne!(painted, before.state_id);
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.ticket().state_id, before.state_id);
    shortcut(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(h.state().editor.session.ticket().state_id, painted);
}

#[test]
fn new_requires_discard_and_cancel_keeps_the_document() {
    let mut h = harness();
    button(&mut h, true);
    button(&mut h, false);
    let before = h.state().editor.session.ticket();
    shortcut(&mut h, Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    shortcut(&mut h, Modifiers::COMMAND, Key::N);
    h.run();
    h.get_by_label("Discard and continue").click();
    h.run();
    assert!(!h.state().editor.session.is_dirty());
    assert_eq!(h.state().editor.session.undo_depth(), 0);
    assert_ne!(h.state().editor.session.ticket().state_id, before.state_id);
}

#[test]
fn escape_and_pointer_loss_cancel_without_history() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    button(&mut h, true);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().editor.tools.gesture_in_progress());
    button(&mut h, false);
    button(&mut h, true);
    h.event(Event::PointerGone);
    h.step();
    assert!(!h.state().editor.tools.gesture_in_progress());
    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn layers_add_delete_and_undo_retarget_paint_safely() {
    let mut h = harness();
    h.get_by_role_and_label(egui::Role::Button, "Layers")
        .click();
    h.run();
    h.get_by_label("Add layer").click();
    h.run();
    let target = h.state().editor.target.unwrap();
    assert_eq!(h.state().editor.session.document().roots.len(), 2);
    h.get_by_label("Delete layer").click();
    h.run();
    assert_ne!(h.state().editor.target, Some(target));
    shortcut(&mut h, Modifiers::COMMAND, Key::Z);
    h.run();
    assert!(
        h.state()
            .editor
            .session
            .document()
            .layers
            .contains_key(&target)
    );
    button(&mut h, true);
    button(&mut h, false);
    assert!(h.state().editor.session.is_dirty());
}

#[test]
fn command_scroll_zooms_without_publishing() {
    let mut h = harness();
    let before = h.state().editor.session.ticket();
    let p = h.state().canvas.rect.unwrap().center();
    h.event(Event::PointerMoved(p));
    h.event(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        modifiers: Modifiers::COMMAND,
        source: egui::MouseWheelSource::Unknown,
        phase: egui::TouchPhase::Move,
    });
    h.run();
    assert!(h.state().zoom > 1.0);
    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn png_export_requires_explicit_flattening_confirmation() {
    let mut h = harness();
    h.get_by_role_and_label(egui::Role::Button, "File").click();
    h.run();
    h.get_by_label("Save as").hover();
    h.run();
    h.get_by_label("Picture…").click();
    h.run();
    assert!(h.query_by_label("Download PNG").is_some());
    let before = h.state().editor.session.ticket();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().editor.session.ticket(), before);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}

#[test]
fn reopened_project_ground_is_used_by_eraser_and_clear_with_undo() {
    use paintcraft_core::types::Rgba;
    let mut h = harness();
    let ground = Rgba([18, 45, 67, 255]);
    let doc =
        paintcraft_document::new_document(16, 16, Some(ground), 0, paintcraft_document::POLICY)
            .unwrap();
    let bytes = paintcraft_formats::pcnd::write(&doc).unwrap();
    let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
    h.state_mut().editor = crate::editor::Editor::from_document(reopened).unwrap();
    h.run();
    button(&mut h, true);
    button(&mut h, false);
    let pixel = |h: &Harness<WebApp>| {
        let layer = h.state().editor.target.unwrap();
        h.state().editor.session.document().layers[&layer]
            .raster
            .get(8, 8)
    };
    assert_eq!(pixel(&h), Rgba([0, 0, 0, 255]));
    h.get_by_role_and_label(egui::Role::Button, "Eraser")
        .click();
    h.run();
    button(&mut h, true);
    button(&mut h, false);
    assert_eq!(pixel(&h), ground);
    h.get_by_role_and_label(egui::Role::Button, "Pencil")
        .click();
    h.run();
    button(&mut h, true);
    button(&mut h, false);
    assert_eq!(pixel(&h), Rgba([0, 0, 0, 255]));
    h.get_by_label("Clear Canvas").click();
    h.run();
    assert!(
        paintcraft_render::render_flat(h.state().editor.session.document())
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == ground.0)
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(pixel(&h), Rgba([0, 0, 0, 255]));
}

#[test]
fn browser_starts_at_the_same_scale_as_zoom_to_window() {
    let mut h = harness();
    let zoom = h.state().zoom;
    let source = h.state().editor.session.snapshot();
    h.get_by_label("Zoom to window").click();
    h.run();
    assert_eq!(h.state().zoom, zoom);
    assert!(h.state().canvas.rect.unwrap().width() < h.state().canvas.viewport.x);
    assert!(h.state().canvas.rect.unwrap().height() < h.state().canvas.viewport.y);
    assert!(h.state().editor.session.document().same_semantics(&source));
}

#[test]
fn historical_browser_selection_menu_routes_delete_undo_and_safe_disabled_choices() {
    use eframe::egui::Role;
    use paintcraft_classic::ClassicVariant;
    use paintcraft_core::types::Rgba;
    use paintcraft_editor::execute::Selection;
    use paintcraft_tools::Action;
    use paintcraft_ui::i18n::{Language, set_language, translate};
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let mut h = harness();
            h.state_mut().variant = variant;
            set_language(&h.ctx, language);
            let layer = h.state().editor.target.unwrap();
            h.state_mut()
                .editor
                .apply(Action::Shape {
                    layer,
                    tool: Tool::Rect,
                    from: (0, 0),
                    to: (15, 15),
                    stroke: None,
                    fill: Some(Rgba([12, 34, 56, 255])),
                })
                .unwrap();
            h.run();
            let label = |s| translate(language, s);
            let ticket = h.state().editor.session.ticket();
            h.get_by_role_and_label(Role::Button, &label("Selection options"))
                .click();
            h.run();
            assert!(
                h.query_by_role_and_label(Role::Label, &label("Selection shapes"))
                    .is_some()
            );
            h.get_by_role_and_label(Role::Button, &label("Free-form selection"))
                .click();
            h.run();
            assert_eq!(h.state().editor.session.ticket(), ticket);
            assert_eq!(
                h.state().controls.options.selection_mode,
                paintcraft_tools::SelectionMode::FreeForm
            );
            h.get_by_role_and_label(Role::Button, &label("Selection options"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::Button, &label("Select all"))
                .click();
            h.run();
            assert!(matches!(
                h.state().editor.session.selection(),
                Selection::Rect(_)
            ));
            h.get_by_role_and_label(Role::Button, &label("Selection options"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::Button, &label("Delete"))
                .click();
            h.run();
            assert_eq!(
                h.state().editor.session.document().layers[&layer]
                    .raster
                    .get(5, 5),
                Rgba([255; 4])
            );
            h.get_by_role_and_label(Role::Button, &label("Undo"))
                .click();
            h.run();
            assert_eq!(
                h.state().editor.session.document().layers[&layer]
                    .raster
                    .get(5, 5),
                Rgba([12, 34, 56, 255])
            );
            h.get_by_role_and_label(Role::Button, &label("Selection options"))
                .click();
            h.run();
            h.get_by_role_and_label(Role::Button, &label("Deselect"))
                .click();
            h.run();
            assert_eq!(h.state().editor.session.selection(), Selection::Inactive);
        }
    }
}
