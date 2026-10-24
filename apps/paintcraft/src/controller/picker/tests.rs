use super::*;
use paintcraft_core::types::picker::{PickerAfter, SampleSize};
use std::time::{Duration, Instant};
fn drain(e: &mut Editor) {
    let end = Instant::now() + Duration::from_secs(10);
    while e.has_pending_edits() {
        assert!(Instant::now() < end);
        e.run_pending();
        std::thread::yield_now();
    }
}
fn request(e: &mut Editor, button: PointerButton) -> ColorPick {
    e.select_tool(Tool::ColorPicker);
    e.tools
        .pointer_down_button(e.target().unwrap(), (1, 1), button);
    let (_, Action::PickColor(p)) = e.tools.pointer_up_button(button).unwrap() else {
        panic!("picker")
    };
    p
}
#[test]
fn picking_changes_only_one_session_slot_without_history_dirty_or_selection_publication() {
    let mut e = Editor::new_document(4, 4).unwrap();
    e.select_tool(Tool::Brush);
    e.options.picker_source = SampleSource::Image;
    e.options.picker_after = PickerAfter::Previous;
    e.secondary = Rgba([30, 40, 50, 255]);
    let before = e.document_snapshot();
    let ticket = e.session.history_ticket();
    let selection = e.selection();
    let primary = e.color;
    let p = request(&mut e, PointerButton::Secondary);
    e.request(9001, Action::PickColor(p));
    assert_eq!(e.run_pending(), 0);
    assert_eq!(e.secondary, Rgba([255; 4]));
    assert_eq!(e.color, primary);
    assert_eq!(e.tool, Tool::Brush);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.session.history_ticket(), ticket);
    assert_eq!(e.selection(), selection);
    assert_eq!(e.undo_depth(), 0);
    assert!(!e.is_dirty());
    e.select_tool(Tool::ColorPicker);
    e.pointer_down((-1, 1));
    e.pointer_up();
    e.run_pending();
    assert_eq!(e.color, primary);
    assert!(e.status().contains("outside the canvas"));
}
#[test]
fn ordered_worker_samples_earlier_accepted_ink_without_a_second_publication() {
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    let mut e = Editor::new_document(4, 4).unwrap();
    e.connect_editing(service.clone());
    let release = service.pause();
    let id = e.target().unwrap();
    let red = Rgba([255, 0, 0, 255]);
    e.request(
        9101,
        Action::Stroke(paintcraft_tools::Stroke {
            media: Default::default(),
            layer: id,
            points: vec![(1, 1)],
            point_scale: 1,
            color: red,
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::Replace,
        }),
    );
    let p = request(&mut e, PointerButton::Primary);
    e.request(9102, Action::PickColor(p));
    e.run_pending();
    assert_eq!(e.publications(), 0);
    release.send(()).unwrap();
    drain(&mut e);
    assert_eq!(e.color, red);
    assert_eq!(e.publications(), 1);
    assert_eq!(e.undo_depth(), 1);
    assert_eq!(e.document().layers[&id].raster.get(1, 1), red);
}
#[test]
fn late_worker_results_respect_tool_slot_document_changes_and_cancellation() {
    for change in 0..4 {
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let mut e = Editor::new_document(4, 4).unwrap();
        e.color = Rgba([10, 20, 30, 255]);
        e.pointer_down((1, 1));
        e.pointer_up();
        e.run_pending();
        e.connect_editing(service.clone());
        let pick = request(&mut e, PointerButton::Primary);
        let base = e.session.history_ticket();
        let doc = e.document_snapshot();
        let original = e.color;
        let (started, wait_started) = std::sync::mpsc::sync_channel(1);
        let (release, gate) = std::sync::mpsc::channel();
        e.pending_edit = Some(
            service
                .submit(
                    move |cancel| {
                        started.send(()).unwrap();
                        gate.recv_timeout(Duration::from_secs(5)).unwrap();
                        let color = sample(&doc, &pick, || cancel.check())?;
                        Ok(background::Completion::Pick(Box::new(Picked {
                            base,
                            pick,
                            color,
                        })))
                    },
                    "Sample color".into(),
                    "",
                    None,
                )
                .unwrap(),
        );
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        match change {
            0 => e.select_tool(Tool::Eraser),
            1 => e.color = Rgba([70, 80, 90, 255]),
            2 => {
                e.session.undo().unwrap();
            }
            _ => e.cancel_pending_edits(),
        }
        let before = e.session.history_ticket();
        let expected = if change == 1 {
            Rgba([70, 80, 90, 255])
        } else {
            original
        };
        release.send(()).unwrap();
        drain(&mut e);
        service.reap_abandoned();
        assert_eq!(e.color, expected);
        assert_eq!(e.session.history_ticket(), before);
    }
}
#[test]
fn hidden_layer_sampling_preserves_raw_transparent_rgb_and_ignores_selection() {
    let mut e = Editor::new_document(4, 4).unwrap();
    let id = e.target().unwrap();
    e.request(
        9201,
        Action::Stroke(paintcraft_tools::Stroke {
            media: Default::default(),
            layer: id,
            points: vec![(1, 1)],
            point_scale: 1,
            color: Rgba([17, 33, 55, 0]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::Replace,
        }),
    );
    e.run_pending();
    e.request(
        9202,
        Action::Select {
            corners: Some(((3, 3), (3, 3))),
        },
    );
    e.run_pending();
    e.request(
        9204,
        Action::SetVisible {
            layer: id,
            visible: false,
        },
    );
    e.run_pending();
    e.options.picker_source = SampleSource::Layer;
    e.options.picker_size = SampleSize::One;
    let before = e.document_snapshot();
    let ticket = e.session.history_ticket();
    let p = request(&mut e, PointerButton::Primary);
    e.request(9203, Action::PickColor(p));
    e.run_pending();
    assert_eq!(e.color, Rgba([17, 33, 55, 0]));
    assert_eq!(e.session.history_ticket(), ticket);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
}

#[test]
fn explicit_sample_does_not_publish_an_unfinished_text_draft() {
    let mut e = Editor::new_document(16, 16).unwrap();
    e.select_tool(Tool::ColorPicker);
    let layer = e.target().unwrap();
    e.tools.begin_text(layer, (1, 1));
    e.tools.type_text("draft");
    let before = e.document_snapshot();
    let ticket = e.session.history_ticket();
    e.tools.pointer_down(layer, (1, 1));
    let (token, action) = e.tools.pointer_up().unwrap();
    e.request(token, action);
    assert_eq!(e.run_pending(), 0);
    assert!(e.tools.is_editing_text());
    assert_eq!(e.tools.text_content(), Some("draft"));
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.session.history_ticket(), ticket);
}
