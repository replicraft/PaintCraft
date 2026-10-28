use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_ui::pen_input::{Owner, Source, View};

fn view() -> View {
    View {
        owner: Owner {
            session: 1,
            state: 2,
            publication: 0,
            interface: 1,
        },
        transform: Transform {
            owner: 1,
            epoch: 1,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        },
        pixels_per_point: 2.0,
        canvas: eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(100.0, 100.0),
        ),
        exclusions: [None; 64],
        focused: true,
        blocked: false,
        ime: false,
    }
}
fn source() -> Source {
    Source::new(16, 16, &SharedBudget::new(1 << 20)).unwrap()
}
fn send(
    source: &Source,
    normalizer: &mut Normalizer,
    mut event: Pointer,
    convert: impl FnOnce(f64) -> Result<Option<(f64, f64)>, Invalid>,
) -> Result<receiver::Route, Invalid> {
    normalizer.observe_cancellation(source.cancellation_epoch());
    event.at = None;
    let fenced = normalizer.contact_fenced(event.id);
    let raw = normalizer.sample(event, Provenance::Actual)?;
    assert_eq!(raw.logical_position, None);
    let result = deliver(source, raw, fenced, convert);
    match result {
        Ok(receiver::Route::Suppressed) if raw.buttons.0 != 0 => {
            normalizer.deny_contact(raw.identity.pointer)
        }
        Err(_) => {
            source.cancel();
            normalizer.clear();
        }
        _ => {}
    }
    result
}
fn no_geometry(_: f64) -> Result<Option<(f64, f64)>, Invalid> {
    panic!("suppressed/no-view record must not request geometry")
}
fn mapped(scale: f64) -> Result<Option<(f64, f64)>, Invalid> {
    assert_eq!(scale, 2.0);
    Ok(Some((20.0, 20.0)))
}
fn mapped_ui(scale: f64) -> Result<Option<(f64, f64)>, Invalid> {
    assert_eq!(scale, 2.0);
    Ok(Some((120.0, 20.0)))
}

#[test]
fn denied_down_without_layout_and_fenced_terminal_recover_the_same_real_pointer_identity() {
    for buttons in [
        Buttons::TIP,
        Buttons::ERASER,
        Buttons::BARREL,
        Buttons::BARREL_UPPER,
        Buttons::TIP | Buttons::BARREL,
    ] {
        let source = source();
        let mut n = Normalizer::new(3);
        assert_eq!(
            send(&source, &mut n, event(Kind::Down, buttons), no_geometry).unwrap(),
            receiver::Route::Suppressed
        );
        source.publish(view()).unwrap();
        assert_eq!(
            send(&source, &mut n, event(Kind::Move, buttons), no_geometry).unwrap(),
            receiver::Route::Suppressed
        );
        assert_eq!(
            send(&source, &mut n, event(Kind::Up, 0), no_geometry).unwrap(),
            receiver::Route::Suppressed
        );
        let expected = if buttons & (Buttons::TIP | Buttons::ERASER) != 0 {
            receiver::Route::Canvas
        } else {
            receiver::Route::Ui
        };
        let convert = if expected == receiver::Route::Canvas {
            mapped
        } else {
            mapped_ui
        };
        assert_eq!(
            send(&source, &mut n, event(Kind::Down, buttons), convert).unwrap(),
            expected
        );
        let batch = source.take(1, view().owner, false).unwrap();
        if expected == receiver::Route::Canvas {
            assert_eq!(batch.frame.samples.len(), 1);
        } else {
            assert_eq!(
                batch
                    .frame
                    .ui
                    .iter()
                    .filter(|record| matches!(
                        record.event,
                        receiver::UiEvent::Button { pressed: true, .. }
                    ))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn invalid_canvas_mapping_does_not_require_an_invented_position_for_terminal_recovery() {
    let source = source();
    source.publish(view()).unwrap();
    let mut n = Normalizer::new(3);
    assert_eq!(
        send(&source, &mut n, event(Kind::Down, Buttons::TIP), |_| Err(
            Invalid::Coordinates
        )),
        Err(Invalid::Coordinates)
    );
    assert_eq!(
        send(
            &source,
            &mut n,
            event(Kind::Move, Buttons::TIP),
            no_geometry
        )
        .unwrap(),
        receiver::Route::Suppressed
    );
    assert_eq!(
        send(&source, &mut n, event(Kind::Up, 0), no_geometry).unwrap(),
        receiver::Route::Suppressed
    );
    assert_eq!(
        send(&source, &mut n, event(Kind::Down, Buttons::TIP), mapped).unwrap(),
        receiver::Route::Canvas
    );
    let batch = source.take(1, view().owner, false).unwrap();
    assert_eq!(batch.frame.samples.len(), 1);
    assert_eq!(batch.frame.samples[0].sequence, 4);
}

#[test]
fn active_capture_losing_view_retires_real_up_without_coordinates_before_fresh_stroke() {
    let source = source();
    source.publish(view()).unwrap();
    let mut n = Normalizer::new(3);
    assert_eq!(
        send(&source, &mut n, event(Kind::Down, Buttons::TIP), mapped).unwrap(),
        receiver::Route::Canvas
    );
    source.invalidate_view();
    assert_eq!(
        send(&source, &mut n, event(Kind::Up, 0), no_geometry).unwrap(),
        receiver::Route::Suppressed
    );
    source.publish(view()).unwrap();
    assert_eq!(
        send(&source, &mut n, event(Kind::Down, Buttons::TIP), mapped).unwrap(),
        receiver::Route::Canvas
    );
    assert_eq!(
        send(&source, &mut n, event(Kind::Up, 0), mapped).unwrap(),
        receiver::Route::Canvas
    );
    let batch = source.take(1, view().owner, false).unwrap();
    assert_eq!(batch.frame.samples.len(), 2);
    assert_eq!(batch.frame.samples[0].sequence, 3);
}

#[test]
fn predicted_positionless_terminal_cannot_retire_or_reclassify_actual_contact() {
    let mut n = Normalizer::new(3);
    let mut down = event(Kind::Down, Buttons::ERASER);
    down.at = None;
    assert_eq!(
        n.sample(down, Provenance::Actual).unwrap().phase,
        Phase::Down
    );
    let mut up = event(Kind::Up, 0);
    up.at = None;
    assert_eq!(n.sample(up, Provenance::Predicted), Err(Invalid::Lifecycle));
    let mut moved = event(Kind::Move, Buttons::ERASER);
    moved.at = None;
    let raw = n.sample(moved, Provenance::Actual).unwrap();
    assert_eq!(raw.phase, Phase::Move);
    assert_eq!(raw.identity.kind, DeviceKind::Eraser);
}
