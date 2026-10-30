//! Real viewport event routing while the nonmodal filter editor is open.
use super::*;
use egui_kittest::{Harness, kittest::Queryable};
#[test]
fn nonmodal_draft_accepts_canvas_pan_zoom_but_not_editor_window_or_paint() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.studio.layers = false;
    app.initial_fit = false;
    app.editor = Editor::with_document(
        paintcraft_document::new_document(
            1024,
            1024,
            Some(Rgba([10, 20, 30, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        None,
    );
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200., 900.))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run();
    let ctx = h.ctx.clone();
    h.state_mut()
        .begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
    h.run();
    let source = h.state().editor.document_snapshot();
    let base = h
        .state()
        .filter_editing
        .as_ref()
        .unwrap()
        .draft
        .as_ref()
        .unwrap()
        .base
        .clone();
    let viewport = h.state().canvas_viewport.unwrap();
    // Pick an actually uncovered canvas point, not a screen coordinate underneath a window.
    let p = [
        viewport.right_bottom() + egui::vec2(-24., -24.),
        viewport.right_top() + egui::vec2(-24., 24.),
        viewport.left_bottom() + egui::vec2(24., -24.),
    ]
    .into_iter()
    .find(|p| {
        h.ctx
            .layer_id_at(*p)
            .is_some_and(|layer| layer.order == egui::Order::Background)
    })
    .expect("uncovered canvas point");
    assert!(h.state().filter_navigation_ready());
    let editor_point = h.get_by_label("Preview").rect().center();
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(editor_point));
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: editor_point,
        button: egui::PointerButton::Middle,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    assert!(
        !h.state().studio.pan.is_active(),
        "editor window input must not pan underneath"
    );
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: editor_point,
        button: egui::PointerButton::Middle,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    let before_rect = h.state().canvas_rect.unwrap();
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Middle,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    assert!(
        h.state().studio.pan.is_active(),
        "navigation={} blocked={} viewport={viewport:?} point={p:?} layer={:?}",
        h.state().filter_navigation_ready(),
        h.state().action_context().blocked,
        h.ctx.layer_id_at(p)
    );
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(p + egui::vec2(-80., -60.)));
    h.run_steps(1);
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: p + egui::vec2(-80., -60.),
        button: egui::PointerButton::Middle,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(!h.state().studio.pan.is_active());
    assert_ne!(h.state().canvas_rect.unwrap().min, before_rect.min);
    let zoom = h.state().zoom;
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0., 1.),
        modifiers: egui::Modifiers::CTRL,
        source: egui::MouseWheelSource::Unknown,
        phase: egui::TouchPhase::Move,
    });
    h.run();
    assert!(h.state().zoom > zoom);
    assert_eq!(
        h.state()
            .filter_editing
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .base,
        base
    );
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    assert_eq!(h.state().editor.undo_depth(), 0);
    // Primary input over the navigable canvas still cannot become paint.
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    assert!(!h.state().editor.gesture_in_progress());
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
    h.state_mut().cancel_filter_edit();
    h.run();
    assert!(h.state().editor.preview().is_none());
}
