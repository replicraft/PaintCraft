//! Actual paused editor jobs; synthetic source records, no OS or touch tool claim.
use super::*;
use paintcraft_platform::tablet as native;
use paintcraft_ui::pen_input::PublicationGuard;
#[derive(Clone, Copy, Debug)]
enum Contact {
    Pen,
    Touch,
    Mouse,
}
fn native_event(contact: Contact, ordinal: u64, phase: Phase) -> native::CaptureEvent {
    let time = Time {
        epoch: 7,
        micros: 100 + ordinal,
        clock: Clock::WindowsPerformance,
    };
    let position = native::WindowPosition::PhysicalPixels(3., 3.);
    match contact {
        Contact::Pen => native::CaptureEvent::Tablet(native::NativePacket {
            raw: RawSample {
                identity: Identity {
                    device: 2,
                    pointer: 3,
                    kind: DeviceKind::Pen,
                },
                sequence: ordinal,
                time,
                phase,
                logical_position: None,
                buttons: Buttons(if phase == Phase::Down {
                    Buttons::TIP
                } else {
                    0
                }),
                modifiers: Modifiers::default(),
                pressure: Some(0.8),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            button_edge: None,
            position: Some(position),
            delivery: native::Delivery::ReceiverPointer,
        }),
        Contact::Touch => native::CaptureEvent::Finger {
            identity: Identity {
                device: 30,
                pointer: 1,
                kind: DeviceKind::Finger,
            },
            phase,
            time,
            provenance: Provenance::Actual,
            position: Some(position),
            confidence: FingerConfidence::High,
            in_contact: phase == Phase::Down,
            primary: true,
        },
        Contact::Mouse => native::CaptureEvent::Mouse(native::MouseRecord {
            kind: native::MouseKind::Button {
                number: 0,
                pressed: phase == Phase::Down,
            },
            position,
            client: true,
            modifiers: Modifiers::default(),
            time,
        }),
    }
}
fn source_control(older: Contact, later: Contact, keyboard: bool, budget: &SharedBudget) {
    let mut collector = native::Collector::new(8, budget).unwrap();
    for (ordinal, contact, phase) in [
        (0, older, Phase::Down),
        (1, older, Phase::Up),
        (2, later, Phase::Down),
    ] {
        collector
            .accept(native::CaptureRecord {
                installation: 7,
                ordinal,
                event: native_event(contact, ordinal, phase),
            })
            .unwrap();
    }
    let class = collector.capture_class();
    assert!(matches!(
        (later, class),
        (Contact::Pen, native::CaptureClass::Pen(_))
            | (Contact::Touch, native::CaptureClass::Touch)
    ));
    let event = if keyboard {
        native::CaptureEvent::Boundary {
            kind: native::Boundary::Keyboard,
            time: None,
        }
    } else {
        native::CaptureEvent::Mouse(native::MouseRecord {
            kind: native::MouseKind::Wheel {
                delta: (0., 1.),
                precise: true,
                physical_scale: 1.,
                phase: native::WheelPhase::Move,
                momentum: false,
            },
            position: native::WindowPosition::PhysicalPixels(3., 3.),
            client: true,
            modifiers: Modifiers::default(),
            time: Time {
                epoch: 7,
                micros: 103,
                clock: Clock::WindowsPerformance,
            },
        })
    };
    collector
        .accept(native::CaptureRecord {
            installation: 7,
            ordinal: 3,
            event,
        })
        .unwrap();
    let frame = collector.take(native::DeliveryHealth::Ready).unwrap();
    match frame.records.last().unwrap() {
        native::Collected::Mouse { retire_capture, .. } if !keyboard => {
            assert_eq!(*retire_capture, Some(class))
        }
        native::Collected::Boundary(_) if keyboard => {}
        other => panic!("wrong active-contact control {other:?}"),
    }
}
fn pen(source: &Source, ordinal: u64, phase: Phase) {
    source
        .submit_native(
            receiver::Delivery::ReceiverPointer,
            RawSample {
                identity: Identity {
                    device: 2,
                    pointer: 3,
                    kind: DeviceKind::Pen,
                },
                sequence: ordinal,
                time: Time {
                    epoch: 7,
                    micros: 100 + ordinal,
                    clock: Clock::WindowsPerformance,
                },
                phase,
                logical_position: None,
                buttons: Buttons(if phase == Phase::Down {
                    Buttons::TIP
                } else {
                    0
                }),
                modifiers: Modifiers::default(),
                pressure: Some(0.8),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            |_| Ok(Some((3., 3.))),
        )
        .unwrap();
}
fn touch(
    source: &Source,
    ordinal: u64,
    phase: Phase,
) -> paintcraft_ui::pen_input::NativeTouchBatch {
    let result = source
        .submit_native_finger(
            NativeFingerMetadata {
                installation: 7,
                ordinal,
                identity: Identity {
                    device: 30,
                    pointer: 1,
                    kind: DeviceKind::Finger,
                },
                phase,
                time: Time {
                    epoch: 7,
                    micros: 100 + ordinal,
                    clock: Clock::WindowsPerformance,
                },
                provenance: Provenance::Actual,
                confidence: FingerConfidence::High,
                in_contact: phase == Phase::Down,
                primary: true,
            },
            NativeFingerAdmission::Candidate,
            |_| Ok(Some((3., 3.))),
        )
        .unwrap();
    let NativeTouchOutcome::Admitted(batch) = result else {
        panic!("expected admitted actual touch metadata");
    };
    batch
}
fn finish(editor: &mut Editor) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while editor.has_pending_edits() {
        assert!(
            std::time::Instant::now() < deadline,
            "paused actual job did not complete"
        );
        editor.run_pending();
        std::thread::yield_now();
    }
}
#[test]
fn active_control_retirement_preserves_actual_older_paused_jobs() {
    for older in [Contact::Pen, Contact::Touch, Contact::Mouse] {
        for (later, keyboard) in [
            (Contact::Pen, false),
            (Contact::Touch, false),
            (Contact::Pen, true),
            (Contact::Touch, true),
        ] {
            let budget = SharedBudget::new(16 << 20);
            let mut editor = Editor::new_document(8, 8).unwrap();
            let service = background::Service::new(eframe::egui::Context::default()).unwrap();
            editor.connect_editing(service.clone());
            let view = context(&editor);
            let source = Source::new_native_contacts(64, 64, &budget).unwrap();
            source.publish(view).unwrap();
            let mut old_pen = None;
            let guard: PublicationGuard = match older {
                Contact::Pen => {
                    pen(&source, 0, Phase::Down);
                    pen(&source, 1, Phase::Up);
                    let batch = source.take(1, view.owner, false).unwrap();
                    let guard = source.publication_guard(&batch, view.owner).unwrap();
                    old_pen = Some(batch);
                    guard
                }
                Contact::Touch => {
                    let batch = touch(&source, 0, Phase::Down);
                    drop(touch(&source, 1, Phase::Up));
                    source
                        .native_touch_publication_guard(&batch, view.owner)
                        .unwrap()
                }
                Contact::Mouse => source.ui_publication_guard(view.owner).unwrap(),
            };
            let before = editor.session.ticket();
            let depth = editor.undo_depth();
            let activation = Activation::new(view.owner.session, 7, 1);
            editor.begin_native_activation(Scope::new(activation, guard.clone(), None));
            let release = service.pause();
            if let Some(batch) = &old_pen {
                editor.select_tool(Tool::Brush);
                editor.options.size = 3;
                editor.color = Rgba([17, 91, 151, 255]);
                for sample in &batch.frame.samples {
                    assert!(editor.pen_sample(*sample, &budget).unwrap());
                }
            } else {
                // Touch remains a source/publication guard; no finger canvas
                // action is being enabled by this test. Real Edit service runs.
                editor.request(1, Action::AddLayer);
            }
            editor.run_pending();
            editor.end_native_activation();
            assert!(editor.has_pending_edits());
            assert!(editor.take_native_publication().is_none());
            // Source-observed control metadata selects active-only retirement;
            // this test handler is not production keyboard/touch GUI adoption.
            source_control(older, later, keyboard, &budget);
            let later_guard = match later {
                Contact::Pen => {
                    pen(&source, 2, Phase::Down);
                    let batch = source.take(2, view.owner, false).unwrap();
                    let guard = source.publication_guard(&batch, view.owner).unwrap();
                    let cancellations = source.retire_active_native_pen().unwrap();
                    assert!(
                        cancellations
                            .iter()
                            .flatten()
                            .any(|c| c.matches_batch(&batch))
                    );
                    guard
                }
                Contact::Touch => {
                    let batch = touch(&source, 2, Phase::Down);
                    let guard = source
                        .native_touch_publication_guard(&batch, view.owner)
                        .unwrap();
                    let cancellations = source.retire_active_native_touches().unwrap();
                    assert!(
                        cancellations
                            .iter()
                            .flatten()
                            .any(|c| c.matches_batch(&batch))
                    );
                    guard
                }
                Contact::Mouse => unreachable!(),
            };
            assert!(!later_guard.is_current());
            assert!(
                guard.is_current(),
                "{older:?} older guard retired by later {later:?}"
            );
            assert_eq!(editor.session.ticket(), before);
            assert_eq!(editor.undo_depth(), depth);
            release.send(()).unwrap();
            finish(&mut editor);
            let result = editor
                .take_native_publication()
                .expect("actual paused job receipt");
            assert_eq!(result.activation(), Some(activation));
            assert_eq!(result.before(), before);
            assert!(result.committed());
            assert!(result.history().is_some());
            assert_eq!(result.publication(), before.publication_epoch + 1);
            assert_eq!(editor.undo_depth(), depth + 1);
            assert!(!later_guard.is_current());
        }
    }
}
