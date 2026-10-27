use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Provenance, RawSample, Time},
};

fn setup() -> (App, egui::Context) {
    let mut app = App::new(None);
    app.install(Editor::new_document(48, 32).unwrap()).unwrap();
    app.editor.select_tool(Tool::Brush);
    app.editor.options.size = 9;
    app.editor.color = Rgba([31, 79, 151, 255]);
    app.editor.secondary = Rgba([171, 37, 91, 255]);
    app.canvas_rect = Some(Rect::from_min_size(
        Pos2::new(10.0, 20.0),
        Vec2::new(48.0, 32.0),
    ));
    app.canvas_viewport = app.canvas_rect;
    app.zoom = 1.0;
    app.tablet = Some(
        Receiver::new(
            Source::new(128, 384, &SharedBudget::new(8 << 20)).unwrap(),
            None,
            &SharedBudget::new(8 << 20),
        )
        .unwrap(),
    );
    let ctx = egui::Context::default();
    publish(&app);
    (app, ctx)
}
fn publish(app: &App) {
    app.tablet
        .as_ref()
        .unwrap()
        .source
        .publish(CaptureView {
            owner: app.tablet_owner(),
            transform: app.tablet_transform().unwrap(),
            pixels_per_point: 1.0,
            canvas: app.canvas_rect.unwrap(),
            exclusions: [None; 64],
            focused: true,
            blocked: false,
            ime: false,
        })
        .unwrap();
}
fn raw(sequence: u64, phase: Phase, buttons: u16) -> RawSample {
    RawSample {
        identity: Identity {
            device: 1,
            pointer: 2,
            kind: DeviceKind::Pen,
        },
        sequence,
        time: Time {
            epoch: 1,
            micros: 100 * sequence,
            clock: Clock::AppKitUptime,
        },
        phase,
        logical_position: Some((20.0 + sequence as f64, 30.0)),
        buttons: Buttons(buttons),
        modifiers: Modifiers::default(),
        pressure: Some(0.4),
        tilt: None,
        twist_degrees: None,
        contact: None,
        provenance: Provenance::Actual,
    }
}
fn push(app: &App, sequence: u64, phase: Phase, buttons: u16) {
    app.tablet
        .as_ref()
        .unwrap()
        .source
        .submit_with(Delivery::ReceiverPointer, |_| {
            Ok(raw(sequence, phase, buttons))
        })
        .unwrap();
}
fn drain(app: &mut App, ctx: &egui::Context) {
    app.tablet_raw_input(
        ctx,
        &mut egui::RawInput {
            focused: true,
            ..Default::default()
        },
    );
}
fn canvas(app: &mut App) -> Vec<egui::Event> {
    let mut events = Vec::new();
    app.tablet_canvas_input(&mut events, true, egui::LayerId::background(), None);
    events
}
fn execute_fallback(app: &mut App, events: Vec<egui::Event>) {
    for event in events {
        match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                let at = (
                    ((pos.x - 10.0) * 256.0) as i64,
                    ((pos.y - 20.0) * 256.0) as i64,
                );
                let button = if button == egui::PointerButton::Primary {
                    paintcraft_input::PointerButton::Primary
                } else {
                    paintcraft_input::PointerButton::Secondary
                };
                if pressed {
                    app.editor.pointer_down_subpixel(at, button);
                } else {
                    app.editor.pointer_move_subpixel(at);
                    app.editor.pointer_up_button(button);
                }
            }
            egui::Event::PointerMoved(pos) => app.editor.pointer_move_subpixel((
                ((pos.x - 10.0) * 256.0) as i64,
                ((pos.y - 20.0) * 256.0) as i64,
            )),
            _ => {}
        }
    }
    app.editor.run_pending();
}
#[test]
fn ordinary_headless_startup_does_not_install_capture_or_change_mouse_ime_keys() {
    let mut app = App::new(None);
    assert!(app.tablet.is_none());
    let mut input = egui::RawInput {
        events: vec![
            egui::Event::Text("hello".into()),
            egui::Event::PointerMoved(Pos2::new(3.0, 4.0)),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "字".into(),
                active_range_chars: None,
            }),
        ],
        ..Default::default()
    };
    let events = input.events.clone();
    app.tablet_raw_input(&egui::Context::default(), &mut input);
    assert_eq!(input.events, events);
}
#[test]
fn native_pressure_batch_publishes_once_and_stale_owner_cannot_publish() {
    let (mut app, ctx) = setup();
    let depth = app.editor.undo_depth();
    push(&app, 1, Phase::Down, 1);
    push(&app, 2, Phase::Up, 0);
    drain(&mut app, &ctx);
    assert!(canvas(&mut app).is_empty());
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), depth + 1);
    assert!(canvas(&mut app).is_empty());
    app.editor.run_pending();
    assert_eq!(app.editor.undo_depth(), depth + 1);
    publish(&app);
    push(&app, 3, Phase::Down, 1);
    push(&app, 4, Phase::Up, 0);
    drain(&mut app, &ctx);
    app.install(Editor::new_document(48, 32).unwrap()).unwrap();
    let before = app.editor.document_snapshot();
    canvas(&mut app);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
}
#[test]
fn skipped_canvas_modal_discards_drained_up_and_private_prefix_before_dismissal() {
    let (mut app, ctx) = setup();
    let before = app.editor.document_snapshot();
    push(&app, 1, Phase::Down, 1);
    drain(&mut app, &ctx);
    canvas(&mut app);
    assert!(app.editor.preview_batch().is_some());
    push(&app, 2, Phase::Up, 0);
    drain(&mut app, &ctx);
    // The payload has left Source; modal bypasses canvas for this entire pass.
    app.confirm = Some(Confirm::QuitUnsaved);
    app.publish_tablet_view(&ctx);
    assert!(app.tablet.as_ref().unwrap().pending.is_none());
    assert!(app.editor.preview_batch().is_none());
    app.confirm = None;
    drain(&mut app, &ctx);
    canvas(&mut app);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
    // Also retire a full unpublished Down+Up if the entire canvas path is skipped.
    publish(&app);
    push(&app, 3, Phase::Down, 1);
    push(&app, 4, Phase::Up, 0);
    drain(&mut app, &ctx);
    drain(&mut app, &ctx);
    canvas(&mut app);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
}
#[test]
fn failed_delivery_discards_even_completed_unpublished_contact_and_rejects_later_input() {
    let (mut app, _) = setup();
    let before = app.editor.document_snapshot();
    push(&app, 1, Phase::Down, 1);
    push(&app, 2, Phase::Up, 0);
    let owner = app.tablet_owner();
    app.tablet.as_mut().unwrap().take(owner, true).unwrap();
    canvas(&mut app);
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
    assert_eq!(
        app.tablet
            .as_ref()
            .unwrap()
            .source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(3, Phase::Down, 1)))
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
}
#[test]
fn fallback_latches_secondary_or_primary_through_both_barrel_change_directions() {
    for (down, up, expected) in [
        (1 | 2, 0, egui::PointerButton::Secondary),
        (1, 2, egui::PointerButton::Primary),
    ] {
        let (mut app, ctx) = setup();
        app.editor.options.round = false;
        push(&app, 1, Phase::Down, down);
        push(&app, 2, Phase::Up, up);
        drain(&mut app, &ctx);
        let events = canvas(&mut app);
        let buttons: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => Some((*button, *pressed)),
                _ => None,
            })
            .collect();
        assert_eq!(buttons, [(expected, true), (expected, false)]);
        let depth = app.editor.undo_depth();
        execute_fallback(&mut app, events);
        assert!(!app.editor.pointer_is_down());
        assert_eq!(app.editor.undo_depth(), depth + 1);
        assert!(app.tablet.as_ref().unwrap().fallback_button.is_none());
    }
}
#[test]
fn genuine_mouse_multibutton_capture_stays_owned_until_last_release() {
    let (app, _) = setup();
    let source = &app.tablet.as_ref().unwrap().source;
    let mut mouse = MouseCapture::default();
    let event = |button, pressed| egui::Event::PointerButton {
        pos: Pos2::new(20.0, 30.0),
        button,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    assert!(mouse.retain(source, &event(egui::PointerButton::Primary, true)));
    assert!(mouse.retain(source, &event(egui::PointerButton::Secondary, true)));
    assert!(mouse.retain(source, &event(egui::PointerButton::Primary, false)));
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(1, Phase::Down, 1)))
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
    assert!(mouse.retain(source, &event(egui::PointerButton::Secondary, false)));
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(2, Phase::Down, 1)))
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Canvas
    );
}

#[test]
fn pressure_refusal_keeps_an_unrelated_existing_text_draft_private() {
    let (mut app, ctx) = setup();
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((4, 4));
    app.editor.pointer_up();
    app.editor.type_text("Keep");
    assert!(app.editor.is_editing_text());
    let before = app.editor.export_snapshot().unwrap();
    let ticket = app.editor.history_view().ticket;
    app.tablet
        .as_ref()
        .unwrap()
        .source
        .submit_with(Delivery::ReceiverPointer, |_| {
            let mut value = raw(1, Phase::Down, 1);
            value.identity.kind = DeviceKind::Eraser;
            Ok(value)
        })
        .unwrap();
    drain(&mut app, &ctx);
    assert!(canvas(&mut app).is_empty());
    assert!(app.editor.is_editing_text());
    assert!(
        app.editor
            .export_snapshot()
            .unwrap()
            .same_semantics(&before)
    );
    assert_eq!(app.editor.history_view().ticket, ticket);
}

#[test]
fn real_canvas_fallback_matches_genuine_pointer_for_all_nine_media() {
    use paintcraft_core::types::brush::BrushMedia;
    for media in BrushMedia::ALL {
        let mut outputs = Vec::new();
        for pen in [false, true] {
            let (mut app, _) = setup();
            app.settings.experience = Experience::Studio;
            app = app.with_initial_zoom(1.0);
            app.editor.options.media = media;
            let mut h = egui_kittest::Harness::builder()
                .with_size(Vec2::new(1100.0, 850.0))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            let rect = h.state().canvas_rect.unwrap();
            let zoom = h.state().zoom;
            let at = |x, y| rect.min + Vec2::new(x, y) * zoom;
            let points = [at(8.25, 10.5), at(19.25, 17.5), at(29.25, 20.5)];
            let depth = h.state().editor.undo_depth();
            if pen {
                for (index, phase) in [Phase::Down, Phase::Move, Phase::Up]
                    .into_iter()
                    .enumerate()
                {
                    h.state()
                        .tablet
                        .as_ref()
                        .unwrap()
                        .source
                        .submit_with(Delivery::ReceiverPointer, |_| {
                            let mut value = raw(
                                index as u64 + 1,
                                phase,
                                if phase == Phase::Up { 0 } else { 1 },
                            );
                            value.logical_position =
                                Some((f64::from(points[index].x), f64::from(points[index].y)));
                            value.pressure = None; // genuinely unavailable, preserving selected fallback media
                            Ok(value)
                        })
                        .unwrap();
                }
                let ctx = h.ctx.clone();
                let mut input = egui::RawInput {
                    focused: true,
                    ..Default::default()
                };
                h.state_mut().tablet_raw_input(&ctx, &mut input);
                h.input_mut().events.extend(input.events);
            } else {
                h.input_mut().events.extend([
                    egui::Event::PointerButton {
                        pos: points[0],
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::default(),
                    },
                    egui::Event::PointerMoved(points[1]),
                    egui::Event::PointerButton {
                        pos: points[2],
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::default(),
                    },
                ]);
            }
            h.run();
            assert!(!h.state().editor.pointer_is_down(), "{media:?}, pen={pen}");
            assert_eq!(h.state().editor.options.media, media);
            assert_eq!(
                h.state().editor.undo_depth(),
                depth + 1,
                "{media:?}, pen={pen}"
            );
            outputs.push(paintcraft_render::render_flat(h.state().editor.document()).unwrap());
        }
        assert_eq!(outputs[0], outputs[1], "{media:?}");
    }
}

#[test]
fn widget_owned_pen_contact_cannot_paint_when_popup_dismisses_mid_contact() {
    let (mut app, ctx) = setup();
    let before = app.editor.document_snapshot();
    let source = app.tablet.as_ref().unwrap().source.clone();
    source
        .publish(CaptureView {
            owner: app.tablet_owner(),
            transform: app.tablet_transform().unwrap(),
            pixels_per_point: 1.0,
            canvas: app.canvas_rect.unwrap(),
            exclusions: [None; 64],
            focused: true,
            blocked: true,
            ime: false,
        })
        .unwrap();
    push(&app, 1, Phase::Down, 1);
    let mut input = egui::RawInput {
        focused: true,
        ..Default::default()
    };
    app.tablet_raw_input(&ctx, &mut input);
    assert!(
        input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. }))
    );
    // The same pass dismisses a popup before canvas dispatch. Widget events
    // remain available to egui but never re-enter the legacy canvas loop.
    app.tablet_canvas_input(&mut input.events, true, egui::LayerId::background(), None);
    assert!(input.events.is_empty());
    publish(&app);
    push(&app, 2, Phase::Move, 1);
    push(&app, 3, Phase::Up, 0);
    let mut input = egui::RawInput {
        focused: true,
        ..Default::default()
    };
    app.tablet_raw_input(&ctx, &mut input);
    assert!(
        input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerButton { pressed: false, .. }))
    );
    app.tablet_canvas_input(&mut input.events, true, egui::LayerId::background(), None);
    assert!(input.events.is_empty());
    app.editor.run_pending();
    assert!(app.editor.document().same_semantics(&before));
}

#[test]
fn focus_loss_and_ime_preedit_retire_private_stroke_before_a_queued_release() {
    for event in [
        egui::Event::WindowFocused(false),
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "字".into(),
            active_range_chars: None,
        }),
    ] {
        let (mut app, ctx) = setup();
        let before = app.editor.document_snapshot();
        push(&app, 1, Phase::Down, 1);
        drain(&mut app, &ctx);
        canvas(&mut app);
        assert!(app.editor.preview_batch().is_some());
        push(&app, 2, Phase::Up, 0);
        let mut input = egui::RawInput {
            focused: true,
            events: vec![event.clone(), egui::Event::Text("keep".into())],
            ..Default::default()
        };
        app.tablet_raw_input(&ctx, &mut input);
        assert!(input.events.contains(&event));
        assert!(input.events.contains(&egui::Event::Text("keep".into())));
        canvas(&mut app);
        app.editor.run_pending();
        assert!(app.editor.preview_batch().is_none());
        assert!(app.editor.document().same_semantics(&before));
    }
}

#[test]
fn translated_gui_allowance_denial_is_typed_before_any_native_installation() {
    let source = Source::new(16, 16, &SharedBudget::new(1 << 20)).unwrap();
    match Receiver::new(source, None, &SharedBudget::new(1)) {
        Err(error) => assert_eq!(error.code, paintcraft_core::types::ErrorCode::ResourceLimit),
        Ok(_) => panic!("Unadmitted GUI storage must not reach native installation"),
    }
}

#[test]
fn held_widget_pen_blocks_original_mouse_motion_even_on_an_idle_source_frame() {
    let (mut app, ctx) = setup();
    let source = app.tablet.as_ref().unwrap().source.clone();
    source
        .publish(CaptureView {
            owner: app.tablet_owner(),
            transform: app.tablet_transform().unwrap(),
            pixels_per_point: 1.0,
            canvas: Rect::ZERO,
            exclusions: [None; 64],
            focused: true,
            blocked: true,
            ime: false,
        })
        .unwrap();
    push(&app, 1, Phase::Down, 1);
    drain(&mut app, &ctx);
    canvas(&mut app);
    let keyboard = egui::Event::Text("keep".into());
    let mut input = egui::RawInput {
        focused: true,
        events: vec![
            keyboard.clone(),
            egui::Event::PointerMoved(Pos2::new(41.0, 31.0)),
        ],
        ..Default::default()
    };
    app.tablet_raw_input(&ctx, &mut input);
    assert_eq!(input.events, [keyboard]);
}

#[test]
fn over_bound_widget_burst_retires_without_a_translated_prefix_or_stuck_button() {
    let (mut app, ctx) = setup();
    let budget = SharedBudget::new(64 << 20);
    // Deliberately supply a valid, larger Source than the production wrapper
    // accepts, to exercise the real pre-append boundary rather than force OOM.
    let source = Source::new(16, UI_RELEASE_SLOTS + 2, &budget).unwrap();
    app.tablet = Some(Receiver::new(source.clone(), None, &budget).unwrap());
    source
        .publish(CaptureView {
            owner: app.tablet_owner(),
            transform: app.tablet_transform().unwrap(),
            pixels_per_point: 1.0,
            canvas: Rect::ZERO,
            exclusions: [None; 64],
            focused: true,
            blocked: true,
            ime: false,
        })
        .unwrap();
    push(&app, 1, Phase::Down, 1);
    let mut first = egui::RawInput {
        focused: true,
        ..Default::default()
    };
    app.tablet_raw_input(&ctx, &mut first);
    let mut output = ctx.run_ui(first, |_| {});
    output.textures_delta.clear();
    assert!(ctx.input(|i| i.pointer.any_down()));
    canvas(&mut app);
    for sequence in 2..=(UI_RELEASE_SLOTS as u64 + 2) {
        source
            .submit_with(Delivery::ReceiverPointer, |_| {
                Ok(raw(sequence, Phase::Move, 1))
            })
            .unwrap();
    }
    let keyboard = egui::Event::Text("keep".into());
    let mut input = egui::RawInput {
        focused: true,
        events: vec![keyboard.clone()],
        ..Default::default()
    };
    app.tablet_raw_input(&ctx, &mut input);
    assert_eq!(input.events, [keyboard]);
    assert!(!ctx.input(|i| i.pointer.any_down()));
    assert!(app.tablet.as_ref().unwrap().pending.is_none());
    assert_eq!(
        source
            .submit_with(Delivery::ReceiverPointer, |_| Ok(raw(
                UI_RELEASE_SLOTS as u64 + 3,
                Phase::Up,
                0
            )))
            .unwrap(),
        paintcraft_input::tablet::receiver::Route::Suppressed
    );
}
