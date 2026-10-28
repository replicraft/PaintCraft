use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_input::{
    Modifiers,
    tablet::{Buttons, Clock, DeviceKind, Identity, Phase, Provenance, RawSample, Time},
};
use paintcraft_platform::tablet::{
    CaptureEvent, MouseKind, MouseRecord, NativePacket, WindowPosition,
};
use std::time::{Duration, Instant};
fn input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 1000.0))),
        focused: true,
        ..Default::default()
    }
}
fn tick(host: &mut Host, ctx: &egui::Context) -> Result<Progress> {
    host.snapshot(DeliveryHealth::Ready)?;
    let mut step = host.step(ctx, input())?;
    step.output.textures_delta.clear();
    if let Some(error) = step.error {
        return Err(error);
    }
    Ok(step.progress)
}
fn finish(host: &mut Host, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "owned semantic pump stalled");
        if tick(host, ctx).unwrap() == Progress::Idle {
            break;
        }
        std::thread::yield_now();
    }
}
fn setup(
    experience: Experience,
) -> (
    Host,
    egui::Context,
    Pos2,
    Arc<paintcraft_document::Document>,
) {
    let mut app = App::new(None);
    app.settings.experience = experience;
    app.settings.language = paintcraft_ui::i18n::Language::English;
    app.install(controller::Editor::new_document(48, 32).unwrap())
        .unwrap();
    app.initial_fit = false;
    app.zoom = 6.0;
    app.editor.select_tool(paintcraft_tools::Tool::Brush);
    app.editor.options.size = 5;
    app.editor.color = paintcraft_core::types::Rgba([221, 31, 19, 255]);
    app.editor.pointer_down((3, 3));
    app.editor.pointer_up();
    app.editor.run_pending();
    let older = app.editor.document_snapshot();
    app.editor.color = paintcraft_core::types::Rgba([17, 67, 221, 255]);
    let mut harness = Harness::builder()
        .with_max_steps(64)
        .with_size(Vec2::new(1100.0, 1000.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    harness.run();
    let undo = harness
        .get_all_by_label("Undo")
        .next()
        .expect("real visible Undo toolbar")
        .rect()
        .center();
    let ctx = harness.ctx.clone();
    let mut app = harness.into_state();
    app.initialize_editing(ctx.clone());
    let mut host = Host::new(app, 64, &SharedBudget::new(16 << 20)).unwrap();
    for _ in 0..4 {
        tick(&mut host, &ctx).unwrap();
    }
    (host, ctx, undo, older)
}
fn time(sequence: u64) -> Time {
    Time {
        epoch: 1,
        micros: (sequence + 1) * 100,
        clock: Clock::AppKitUptime,
    }
}
fn pen(host: &Host, sequence: u64, phase: Phase, at: (f64, f64)) {
    let view = host.ingress.lock().unwrap().view().unwrap();
    let x = view.transform.origin.0 + at.0 * view.transform.points_per_pixel;
    let y = view.transform.origin.1 + at.1 * view.transform.points_per_pixel;
    host.capture_endpoint()(CaptureRecord {
        installation: 1,
        ordinal: sequence,
        event: CaptureEvent::Tablet(NativePacket {
            button_edge: None,
            raw: RawSample {
                identity: Identity {
                    device: 7,
                    pointer: 9,
                    kind: DeviceKind::Pen,
                },
                sequence,
                time: time(sequence),
                phase,
                logical_position: None,
                buttons: Buttons(if phase == Phase::Up { 0 } else { 1 }),
                modifiers: Modifiers::default(),
                pressure: Some(0.7),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            position: Some(WindowPosition::PhysicalPixels(
                x * view.pixels_per_point,
                y * view.pixels_per_point,
            )),
            delivery: paintcraft_platform::tablet::Delivery::ReceiverPointer,
        }),
    })
    .unwrap();
}
fn mouse(host: &Host, sequence: u64, pressed: bool, at: Pos2) {
    let ppp = host
        .ingress
        .lock()
        .unwrap()
        .view()
        .unwrap()
        .pixels_per_point;
    host.capture_endpoint()(CaptureRecord {
        installation: 1,
        ordinal: sequence,
        event: CaptureEvent::Mouse(MouseRecord {
            kind: MouseKind::Button { number: 0, pressed },
            position: WindowPosition::PhysicalPixels(f64::from(at.x) * ppp, f64::from(at.y) * ppp),
            client: true,
            modifiers: Modifiers::default(),
            time: time(sequence),
        }),
    })
    .unwrap();
}
#[test]
fn completed_pen_then_real_undo_in_one_snapshot_preserves_the_older_stroke() {
    for experience in [Experience::Studio, Experience::Classic] {
        let (mut host, ctx, undo, older) = setup(experience);
        let depth = host.app.editor.undo_depth();
        pen(&host, 0, Phase::Down, (22.0, 18.0));
        pen(&host, 1, Phase::Up, (22.0, 18.0));
        mouse(&host, 2, true, undo);
        mouse(&host, 3, false, undo);
        assert!(host.snapshot(DeliveryHealth::Ready).unwrap());
        finish(&mut host, &ctx);
        assert!(
            host.app.editor.document().same_semantics(&older),
            "{experience:?}"
        );
        assert_eq!(host.app.editor.undo_depth(), depth);
        assert_eq!(host.app.editor.redo_depth(), 1);
    }
}
#[test]
fn real_undo_then_queued_pen_uses_navigation_witness_and_retains_correct_history() {
    let (mut host, ctx, undo, older) = setup(Experience::Studio);
    let depth = host.app.editor.undo_depth();
    pen(&host, 0, Phase::Down, (20.0, 18.0));
    pen(&host, 1, Phase::Up, (20.0, 18.0));
    mouse(&host, 2, true, undo);
    mouse(&host, 3, false, undo);
    pen(&host, 4, Phase::Down, (34.0, 18.0));
    pen(&host, 5, Phase::Up, (34.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.undo_depth(), depth + 1);
    assert_eq!(host.app.editor.redo_depth(), 0);
    let target = host.app.editor.target().unwrap();
    assert_eq!(
        host.app.editor.document().layers[&target]
            .raster
            .get(20, 18),
        older.layers[&target].raster.get(20, 18)
    );
    assert_ne!(
        host.app.editor.document().layers[&target]
            .raster
            .get(34, 18),
        older.layers[&target].raster.get(34, 18)
    );
    host.app.editor.request(999, paintcraft_tools::Action::Undo);
    let deadline = Instant::now() + Duration::from_secs(10);
    while host.app.editor.has_pending_edits() {
        host.app.editor.run_pending();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(host.app.editor.document().same_semantics(&older));
}
#[test]
fn paused_own_publication_keeps_later_admitted_contact_current() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let service = host.app.editing.clone().unwrap();
    let release = service.pause();
    let before = host.app.editor.publications();
    pen(&host, 0, Phase::Down, (20.0, 18.0));
    pen(&host, 1, Phase::Up, (20.0, 18.0));
    pen(&host, 2, Phase::Down, (34.0, 18.0));
    pen(&host, 3, Phase::Up, (34.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    assert_eq!(host.app.editor.publications(), before);
    let frame = host.pending.as_ref().unwrap();
    assert!(
        host.ingress
            .lock()
            .unwrap()
            .is_current(&frame.entries[host.cursor], host.app.owned_owner())
    );
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 2);
}
#[test]
fn unrelated_preexisting_completion_never_authorizes_queued_pen() {
    let (mut host, ctx, _, older) = setup(Experience::Studio);
    pen(&host, 0, Phase::Down, (22.0, 18.0));
    pen(&host, 1, Phase::Up, (22.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    let service = host.app.editing.clone().unwrap();
    let release = service.pause();
    host.app
        .editor
        .request(777, paintcraft_tools::Action::AddLayer);
    assert_eq!(
        tick(&mut host, &ctx).unwrap(),
        Progress::WaitingForPriorEdit
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let error = loop {
        match tick(&mut host, &ctx) {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
    assert_eq!(
        host.app.editor.document().layers.len(),
        older.layers.len() + 1
    );
    let old_target = *older.roots.last().unwrap();
    assert_eq!(
        host.app.editor.document().layers[&old_target]
            .raster
            .get(22, 18),
        older.layers[&old_target].raster.get(22, 18)
    );
}

#[test]
fn next_contact_acquired_during_own_wait_keeps_partial_down_preview_and_real_up() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let release = host.app.editing.clone().unwrap().pause();
    let before = host.app.editor.publications();
    pen(&host, 0, Phase::Down, (20.0, 18.0));
    pen(&host, 1, Phase::Up, (20.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    // This partial contact arrives while the exact own service is paused.
    pen(&host, 2, Phase::Down, (34.0, 18.0));
    assert!(!host.snapshot(DeliveryHealth::Ready).unwrap());
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 1);
    // The pump now drains already-acquired input before reporting Idle, so the
    // partial contact is already private/live after finishing the prior barrier.
    assert!(host.app.editor.gesture_in_progress());
    assert!(!host.snapshot(DeliveryHealth::Ready).unwrap());
    finish(&mut host, &ctx);
    assert!(host.app.editor.gesture_in_progress());
    assert_eq!(host.app.editor.publications(), before + 1);
    pen(&host, 3, Phase::Up, (34.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 2);
}
#[test]
fn a_noop_pen_does_not_add_history_before_the_real_undo_widget() {
    let (mut host, ctx, undo, _) = setup(Experience::Studio);
    host.app.editor.color = paintcraft_core::types::Rgba([255; 4]);
    let before = host.app.editor.publications();
    let depth = host.app.editor.undo_depth();
    pen(&host, 0, Phase::Down, (22.0, 18.0));
    pen(&host, 1, Phase::Up, (22.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before);
    assert_eq!(host.app.editor.undo_depth(), depth);
    mouse(&host, 2, true, undo);
    mouse(&host, 3, false, undo);
    host.snapshot(DeliveryHealth::Ready).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 1);
    assert_eq!(host.app.editor.undo_depth(), depth - 1);
}
#[test]
fn a_new_settings_modal_retires_continuation_before_receipt_ack_without_losing_frame_output() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let release = host.app.editing.clone().unwrap().pause();
    pen(&host, 0, Phase::Down, (20.0, 18.0));
    pen(&host, 1, Phase::Up, (20.0, 18.0));
    pen(&host, 2, Phase::Down, (34.0, 18.0));
    pen(&host, 3, Phase::Up, (34.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    let before = host.app.editor.publications();
    let depth = host.app.editor.undo_depth();
    host.app.open_settings(&ctx);
    assert!(tick(&mut host, &ctx).is_err());
    release.send(()).unwrap();
    assert_no_resurrection(&mut host, &ctx, before, depth);
    assert!(matches!(
        host.app.confirm,
        Some(crate::Confirm::Settings(_))
    ));
}

#[test]
fn ordinary_ime_is_delivered_during_own_wait_and_queued_native_work_is_retired() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let release = host.app.editing.clone().unwrap().pause();
    let before = host.app.editor.publications();
    pen(&host, 0, Phase::Down, (20.0, 18.0));
    pen(&host, 1, Phase::Up, (20.0, 18.0));
    pen(&host, 2, Phase::Down, (34.0, 18.0));
    pen(&host, 3, Phase::Up, (34.0, 18.0));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    let mut raw = input();
    raw.events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "字".into(),
        active_range_chars: None,
    }));
    host.snapshot(DeliveryHealth::Ready).unwrap();
    let mut step = host.step(&ctx, raw).unwrap();
    step.output.textures_delta.clear();
    assert_eq!(step.progress, Progress::Retired);
    assert!(
        host.app.ime_composing,
        "normal IME path receives the original event"
    );
    assert!(host.pending.is_none());
    assert!(host.waiting.is_none());
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    // Retirement revokes the precisely owned private result before polling.
    assert_eq!(host.app.editor.publications(), before);
}

#[test]
fn owned_activation_requires_the_existing_service_instead_of_inline_editing() {
    let app = App::new(None);
    let result = Host::new(app, 64, &SharedBudget::new(16 << 20));
    assert!(matches!(
        result,
        Err(Error {
            code: ErrorCode::TargetInvalid,
            ..
        })
    ));
}

fn assert_no_resurrection(host: &mut Host, ctx: &egui::Context, before: u64, depth: usize) {
    assert!(
        !host.app.any_pending_edits(),
        "owned private job must already be abandoned"
    );
    for _ in 0..12 {
        host.app.editing.as_ref().unwrap().reap_abandoned();
        let mut output = ctx.run_ui(input(), |ui| host.app.ui(ui));
        output.textures_delta.clear();
        std::thread::yield_now();
    }
    assert_eq!(host.app.editor.publications(), before);
    assert_eq!(host.app.editor.undo_depth(), depth);
}
#[test]
fn paused_owned_preparation_is_retired_by_failure_focus_and_invalid_bridge_input() {
    for reason in 0..4 {
        let (mut host, ctx, _, _) = setup(Experience::Studio);
        let release = host.app.editing.clone().unwrap().pause();
        let before = host.app.editor.publications();
        let depth = host.app.editor.undo_depth();
        pen(&host, 0, Phase::Down, (20.0, 18.0));
        pen(&host, 1, Phase::Up, (20.0, 18.0));
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
        let mut raw = input();
        match reason {
            0 => {
                host.snapshot(DeliveryHealth::Failed).unwrap();
            }
            1 => {
                host.snapshot(DeliveryHealth::Ready).unwrap();
                raw.focused = false;
            }
            2 => {
                host.snapshot(DeliveryHealth::Ready).unwrap();
                raw.events.push(egui::Event::PointerMoved(Pos2::ZERO));
            }
            _ => {} // No fresh health observation must retire, not retain authority.
        }
        match host.step(&ctx, raw) {
            Ok(mut step) => {
                assert_eq!(step.progress, Progress::Retired);
                step.output.textures_delta.clear();
            }
            Err(_) => assert!(reason >= 2),
        }
        release.send(()).unwrap();
        assert_no_resurrection(&mut host, &ctx, before, depth);
    }
}
#[test]
fn failure_does_not_abandon_a_preexisting_unrelated_edit() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let release = host.app.editing.clone().unwrap().pause();
    let before = host.app.editor.publications();
    let layers = host.app.editor.document().layers.len();
    host.app
        .editor
        .request(777, paintcraft_tools::Action::AddLayer);
    assert_eq!(
        tick(&mut host, &ctx).unwrap(),
        Progress::WaitingForPriorEdit
    );
    host.snapshot(DeliveryHealth::Failed).unwrap();
    let mut step = host.step(&ctx, input()).unwrap();
    step.output.textures_delta.clear();
    assert!(host.app.any_pending_edits());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while host.app.any_pending_edits() {
        assert!(Instant::now() < deadline);
        let mut output = ctx.run_ui(input(), |ui| host.app.ui(ui));
        output.textures_delta.clear();
        std::thread::yield_now();
    }
    assert_eq!(host.app.editor.publications(), before + 1);
    assert_eq!(host.app.editor.document().layers.len(), layers + 1);
}

#[test]
fn prepared_owned_pen_result_cannot_publish_after_failure_focus_modal_or_ime_retirement() {
    for reason in 0..4 {
        let (mut host, ctx, _, _) = setup(Experience::Studio);
        let (prepared, release) = host.app.editing.as_ref().unwrap().pause_next_completion();
        let before = host.app.editor.publications();
        let depth = host.app.editor.undo_depth();
        pen(&host, 0, Phase::Down, (20.0, 18.0));
        pen(&host, 1, Phase::Up, (20.0, 18.0));
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
        prepared.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut raw = input();
        if reason == 0 {
            host.snapshot(DeliveryHealth::Failed).unwrap();
        } else {
            host.snapshot(DeliveryHealth::Ready).unwrap();
        }
        match reason {
            1 => raw.focused = false,
            2 => host.app.open_settings(&ctx),
            3 => raw.events.push(egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "字".into(),
                active_range_chars: None,
            })),
            _ => {}
        }
        let mut step = host.step(&ctx, raw).unwrap();
        step.output.textures_delta.clear();
        assert_eq!(step.progress, Progress::Retired);
        if reason == 3 {
            assert!(host.app.ime_composing);
        }
        if reason == 2 {
            assert!(matches!(
                host.app.confirm,
                Some(crate::Confirm::Settings(_))
            ));
        }
        release.send(()).unwrap();
        assert_no_resurrection(&mut host, &ctx, before, depth);
    }
}

#[test]
fn typed_backing_cancel_is_not_mistaken_for_an_authored_activation() {
    let (mut host, ctx, _, _) = setup(Experience::Studio);
    let before = host.app.editor.publications();
    host.capture_endpoint()(CaptureRecord {
        installation: 1,
        ordinal: 0,
        event: CaptureEvent::Boundary {
            kind: paintcraft_platform::tablet::Boundary::ViewInvalidated,
            time: None,
        },
    })
    .unwrap();
    assert!(host.ingress.lock().unwrap().view().is_none());
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before);
    pen(&host, 1, Phase::Down, (20.0, 18.0));
    pen(&host, 2, Phase::Up, (20.0, 18.0));
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 1);
}

#[test]
fn replaying_acquired_backing_boundary_keeps_newer_completed_contact_and_cancels_old_preview() {
    for experience in [Experience::Classic, Experience::Studio] {
        let (mut host, ctx, _, older) = setup(experience);
        let before = host.app.editor.publications();
        let depth = host.app.editor.undo_depth();
        pen(&host, 0, Phase::Down, (20.0, 18.0));
        assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
        assert_eq!(host.app.editor.publications(), before);
        let view = host.ingress.lock().unwrap().view().unwrap();
        host.capture_endpoint()(CaptureRecord {
            installation: 1,
            ordinal: 1,
            event: CaptureEvent::Boundary {
                kind: paintcraft_platform::tablet::Boundary::ViewInvalidated,
                time: None,
            },
        })
        .unwrap();
        // A completed fresh layout may be followed by several real source
        // records before the GUI replays its already-acquired cancellation.
        host.ingress.lock().unwrap().publish(view).unwrap();
        pen(&host, 2, Phase::Up, (20.0, 18.0));
        pen(&host, 3, Phase::Down, (34.0, 18.0));
        pen(&host, 4, Phase::Up, (34.0, 18.0));
        finish(&mut host, &ctx);
        assert_eq!(host.app.editor.publications(), before + 1);
        assert_eq!(host.app.editor.undo_depth(), depth + 1);
        let layer = older.roots[0];
        assert_eq!(
            host.app.editor.document().layers[&layer].raster.get(20, 18),
            older.layers[&layer].raster.get(20, 18)
        );
        assert_ne!(
            host.app.editor.document().layers[&layer].raster.get(34, 18),
            older.layers[&layer].raster.get(34, 18)
        );
    }
}

#[test]
fn pen_only_retirement_does_not_abandon_a_paused_genuine_gui_navigation_job() {
    let (mut host, ctx, undo, _) = setup(Experience::Studio);
    let before = host.app.editor.publications();
    let depth = host.app.editor.undo_depth();
    let release = host.app.editing.clone().unwrap().pause();
    mouse(&host, 0, true, undo);
    mouse(&host, 1, false, undo);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::Advanced);
    assert_eq!(tick(&mut host, &ctx).unwrap(), Progress::WaitingForOwnEdit);
    assert!(!host.ingress.lock().unwrap().source().retire_pen());
    release.send(()).unwrap();
    finish(&mut host, &ctx);
    assert_eq!(host.app.editor.publications(), before + 1);
    assert_eq!(host.app.editor.undo_depth(), depth - 1);
}

#[test]
fn actual_undo_widget_requires_the_exact_healthy_native_installation() {
    for experience in [Experience::Classic, Experience::Studio] {
        for matching in [true, false] {
            let (mut host, ctx, undo, _) = setup(experience);
            let source = paintcraft_tablet_native::TestInstallation::default();
            let other = paintcraft_tablet_native::TestInstallation::default();
            let epoch = source.source_epoch();
            assert_ne!(epoch, other.source_epoch());
            let before = host.app.editor.document_snapshot();
            let depth = host.app.editor.undo_depth();
            let ppp = host
                .ingress
                .lock()
                .unwrap()
                .view()
                .unwrap()
                .pixels_per_point;
            host.bind_delivery(if matching {
                source.delivery_check()
            } else {
                other.delivery_check()
            });
            for (ordinal, pressed) in [(0, true), (1, false)] {
                host.capture_endpoint()(CaptureRecord {
                    installation: epoch,
                    ordinal,
                    event: CaptureEvent::Mouse(MouseRecord {
                        kind: MouseKind::Button { number: 0, pressed },
                        position: WindowPosition::PhysicalPixels(
                            f64::from(undo.x) * ppp,
                            f64::from(undo.y) * ppp,
                        ),
                        client: true,
                        modifiers: Modifiers::default(),
                        time: Time {
                            epoch,
                            ..time(ordinal)
                        },
                    }),
                })
                .unwrap();
            }
            if matching {
                finish(&mut host, &ctx);
                assert_eq!(host.app.editor.undo_depth(), depth - 1);
                assert_eq!(host.app.editor.redo_depth(), 1);
                assert!(!host.app.editor.document().same_semantics(&before));
            } else {
                assert!(host.snapshot(DeliveryHealth::Ready).unwrap());
                let mut result = host.step(&ctx, input()).unwrap();
                result.output.textures_delta.clear();
                assert_eq!(result.progress, Progress::Retired);
                assert!(result.error.is_some());
                assert_eq!(host.app.editor.undo_depth(), depth);
                assert_eq!(host.app.editor.redo_depth(), 0);
                assert!(host.app.editor.document().same_semantics(&before));
            }
            let check = source.delivery_check();
            drop(source);
            assert!(!check.authorize_installation(epoch, || panic!("retired fixture")));
        }
    }
}
