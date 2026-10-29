//! Trusted-host proof construction from actual typed editor results. This does
//! not adopt finger input in App/Projection or authenticate synthetic hardware.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_input::{Modifiers, tablet::*};
use paintcraft_ui::pen_input::{
    Continuation, NativeFingerAdmission, NativeFingerMetadata, NativeTouchOutcome, Owner,
    Predecessor, Source, View,
};
fn context(editor: &Editor) -> View {
    let ticket = editor.session.ticket();
    let owner = editor.session.owner_id().as_u128();
    View {
        owner: Owner {
            session: owner,
            state: ticket.state_id.as_u128(),
            publication: ticket.publication_epoch,
            interface: 1,
        },
        transform: Transform {
            owner,
            epoch: 1,
            origin: (0., 0.),
            points_per_pixel: 1.,
        },
        pixels_per_point: 1.,
        canvas: eframe::egui::Rect::from_min_max(
            eframe::egui::Pos2::ZERO,
            eframe::egui::pos2(8., 8.),
        ),
        exclusions: [None; 64],
        focused: true,
        blocked: false,
        ime: false,
    }
}
fn later_source(
    s: &Source,
    v: View,
) -> (
    paintcraft_ui::pen_input::Batch,
    paintcraft_ui::pen_input::NativeTouchBatch,
) {
    for (ordinal, phase) in [(2, Phase::Down), (3, Phase::Up)] {
        s.submit_native(
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
                pressure: Some(0.5),
                tilt: None,
                twist_degrees: None,
                contact: None,
                provenance: Provenance::Actual,
            },
            |_| Ok(Some((2., 2.))),
        )
        .unwrap();
    }
    let pen = s.take(1, v.owner, false).unwrap();
    let mut touch = None;
    for (ordinal, phase) in [(4, Phase::Down), (5, Phase::Up)] {
        let outcome = s
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
                |_| Ok(Some((2., 2.))),
            )
            .unwrap();
        let NativeTouchOutcome::Admitted(batch) = outcome else {
            panic!("actual-admitted source batch")
        };
        if phase == Phase::Down {
            touch = Some(batch);
        }
    }
    (pen, touch.unwrap())
}
#[test]
fn mixed_source_continuation_uses_actual_authored_and_navigation_results() {
    for (navigation, native_contacts) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut editor = Editor::new_document(8, 8).unwrap();
        if navigation {
            editor.request(77, Action::AddLayer);
            editor.run_pending();
            editor.take_native_publication();
        }
        let v = context(&editor);
        let before = editor.session.ticket();
        let budget = SharedBudget::new(8 << 20);
        let source = if native_contacts {
            Source::new_native_contacts(64, 64, &budget)
        } else {
            Source::new_native_touch(64, 64, &budget)
        }
        .unwrap();
        source.publish(v).unwrap();
        // Synthetic source ordinal1 initiates exactly this real editor action;
        // later ordinal2/4 contacts arrive before its actual publication.
        let activation = Activation::new(v.owner.session, 7, 1);
        editor.begin_native_activation(Scope::new(
            activation,
            source.ui_publication_guard(v.owner).unwrap(),
            None,
        ));
        editor.request(
            1,
            if navigation {
                Action::Undo
            } else {
                Action::AddLayer
            },
        );
        let (mut pen, mut touch) = later_source(&source, v);
        editor.run_pending();
        editor.end_native_activation();
        let after = if navigation {
            assert!(editor.take_native_publication().is_none());
            let result = editor
                .take_native_navigation()
                .expect("actual Undo Navigation");
            assert_eq!(result.activation(), Some(activation));
            assert_eq!(result.before(), before);
            Owner {
                session: result.owner().as_u128(),
                state: result.state().as_u128(),
                publication: result.publication(),
                interface: v.owner.interface,
            }
        } else {
            let result = editor
                .take_native_publication()
                .expect("actual authored Receipt witness");
            assert_eq!(result.activation(), Some(activation));
            assert_eq!(result.before(), before);
            assert!(result.committed());
            assert!(result.history().is_some());
            Owner {
                session: result.owner().as_u128(),
                state: result.state().as_u128(),
                publication: result.publication(),
                interface: v.owner.interface,
            }
        };
        // New head tickets alone are not used to construct this proof.
        let proof = Predecessor {
            before: v.owner,
            after,
            sequence: 1,
            transform: v.transform,
            canvas: v.canvas,
            pixels_per_point: v.pixels_per_point,
            focused: true,
            modal: false,
            ime: false,
        };
        source
            .acknowledge_continuations(
                &mut [
                    Continuation::Pen(&mut pen),
                    Continuation::NativeTouch(&mut touch),
                ],
                proof,
            )
            .unwrap();
        assert!(source.batch_is_current(&pen, after));
        assert!(source.native_touch_batch_is_current(&touch, after));
        assert!(
            source
                .native_touch_publication_guard(&touch, after)
                .unwrap()
                .is_current()
        );
    }
}
#[test]
fn noop_without_typed_publication_cannot_supply_a_predecessor() {
    let mut editor = Editor::new_document(8, 8).unwrap();
    let v = context(&editor);
    let source = Source::new_native_touch(64, 64, &SharedBudget::new(8 << 20)).unwrap();
    source.publish(v).unwrap();
    editor.request(1, Action::Undo);
    let (pen, touch) = later_source(&source, v);
    editor.run_pending();
    assert!(editor.take_native_publication().is_none());
    assert!(editor.take_native_navigation().is_none());
    // There is no typed result from which the trusted host may construct proof.
    assert!(source.batch_is_current(&pen, v.owner));
    assert!(source.native_touch_batch_is_current(&touch, v.owner));
}
#[path = "active_retirement_tests.rs"]
mod active_retirement;
#[path = "native_touch_protocol.rs"]
mod protocol;
