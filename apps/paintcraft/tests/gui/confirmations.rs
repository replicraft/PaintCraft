use super::*;
use paintcraft_ui::confirmation::{Choice, Prompt};

#[test]
fn cancelling_new_with_escape_or_backdrop_preserves_pixels_and_history() {
    let mut h = harness_with(App::new(None), 1.0, true);
    with_layer(&mut h);
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    for escape in [true, false] {
        h.event(key_up(Key::N, Modifiers::COMMAND));
        h.event(key(Key::N, Modifiers::COMMAND));
        h.run();
        assert!(h.query_by_label("Discard changes?").is_some());
        if escape {
            h.key_press(Key::Escape);
        } else {
            let outside = Pos2::new(4.0, 4.0);
            h.input_mut().events.extend([
                Event::PointerMoved(outside),
                press(outside),
                release(outside),
            ]);
        }
        h.run();
        assert!(h.query_by_label("Discard changes?").is_none());
        assert_eq!(h.state().document_count(), 1);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
    }
}

#[test]
fn confirmation_buttons_stay_reachable_for_long_messages_and_each_choice_fires_once() {
    for (size, ppp, dark) in [
        (Vec2::new(1360.0, 900.0), 1.0, false),
        (Vec2::new(560.0, 500.0), 2.0, true),
        (Vec2::new(360.0, 360.0), 1.0, true),
        (Vec2::new(480.0, 360.0), 2.0, false),
    ] {
        for discard in [None, Some("Discard and switch")] {
            let message = format!(
                "{}\n{}",
                "An unsaved document needs your decision.\n".repeat(60),
                "very-long-path-component/".repeat(40)
            );
            let mut h = Harness::builder()
                .with_size(size)
                .with_pixels_per_point(ppp)
                .build_ui_state(
                    move |ui, choices: &mut Vec<Choice>| {
                        ui.ctx().set_visuals(if dark {
                            eframe::egui::Visuals::dark()
                        } else {
                            eframe::egui::Visuals::light()
                        });
                        let choice = paintcraft_ui::confirmation::show(
                            ui.ctx(),
                            eframe::egui::Id::unique("test-confirmation"),
                            Prompt {
                                title: "Save changes before closing this document?",
                                message: &message,
                                accept: "Save and switch",
                                discard,
                            },
                        );
                        if ui.ctx().current_pass_index() == 0 {
                            if let Some(choice) = choice {
                                choices.push(choice);
                            }
                            ui.ctx()
                                .request_discard("test: repeated confirmation layout");
                        }
                    },
                    Vec::new(),
                );
            h.run();
            let footer = h.get_by_label("Cancel").rect();
            h.run_steps(8);
            assert_eq!(h.get_by_label("Cancel").rect(), footer);
            for label in [Some("Save and switch"), discard, Some("Cancel")]
                .into_iter()
                .flatten()
            {
                let rect = h.get_by_label(label).rect();
                assert!(
                    rect.min.x >= 0.0
                        && rect.min.y >= 0.0
                        && rect.max.x <= size.x
                        && rect.max.y <= size.y,
                    "{label}: {rect:?} outside {size:?}"
                );
            }
            // Scroll the long explanation; the action area is outside its scroll region.
            let text_before = h.get_by_label_contains("An unsaved document").rect();
            let body = text_before.min + Vec2::new(10.0, 5.0);
            h.input_mut().events.extend([
                Event::PointerMoved(body),
                Event::MouseWheel {
                    unit: eframe::egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0),
                    modifiers: Modifiers::NONE,
                    phase: eframe::egui::TouchPhase::Move,
                    source: eframe::egui::MouseWheelSource::Unknown,
                },
            ]);
            h.run();
            assert!(
                h.get_by_label_contains("An unsaved document").rect().min.y < text_before.min.y,
                "the explanation must actually scroll at {size:?}"
            );
            assert_eq!(h.get_by_label("Cancel").rect(), footer);
            for (label, expected) in [
                ("Save and switch", Choice::Accept),
                ("Cancel", Choice::Cancel),
            ] {
                h.state_mut().clear();
                h.get_by_label(label).click();
                h.run();
                assert_eq!(h.state(), &[expected]);
            }
            if let Some(label) = discard {
                h.state_mut().clear();
                h.get_by_label(label).click();
                h.run();
                assert_eq!(h.state(), &[Choice::Discard]);
            }
            h.state_mut().clear();
            h.key_press(Key::Escape);
            h.run();
            assert_eq!(h.state(), &[Choice::Cancel]);
        }
    }
}

#[test]
fn classic_close_prompt_escape_keeps_the_document_and_discard_closes_only_on_click() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(480.0, 360.0))
        .with_pixels_per_point(2.0)
        .build_ui_state(
            |ui, app: &mut App| {
                app.ui(ui);
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("test: repeated close layout");
                }
            },
            App::new(None),
        );
    h.run();
    let point = h.state().canvas_rect().unwrap().min + Vec2::new(10.0, 10.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(point), press(point), release(point)]);
    h.run();
    assert!(h.state().editor().is_dirty());
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    h.event(key(Key::W, Modifiers::COMMAND));
    h.run();
    for label in ["Save and close", "Don't save", "Cancel"] {
        let rect = h.get_by_label(label).rect();
        assert!(
            rect.min.x >= 0.0 && rect.min.y >= 0.0 && rect.max.x <= 480.0 && rect.max.y <= 360.0,
            "{rect:?}"
        );
    }
    h.key_press(Key::Escape);
    h.run();
    assert!(h.query_by_label("Save and close").is_none());
    assert_eq!(h.state().document_count(), 1);
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    h.event(key_up(Key::W, Modifiers::COMMAND));
    h.event(key(Key::W, Modifiers::COMMAND));
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(h.state().document_count(), 0);
}

#[test]
#[ignore = "requires GPU; exports real save/discard/cancel confirmation contexts"]
fn export_confirmation_surfaces() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = std::env::temp_dir().join("paintcraft-confirmation-surfaces");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, theme) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
        for (size, name) in [
            (Vec2::new(1360.0, 900.0), "desktop"),
            (Vec2::new(360.0, 360.0), "compact"),
        ] {
            for kind in ["new", "close", "switch"] {
                let dir = scratch("confirmation-surface-review");
                Settings {
                    appearance,
                    classic_variant: Some(ClassicVariant::V11),
                    ..Default::default()
                }
                .save(&dir)
                .unwrap();
                let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
                h.set_size(size);
                h.run();
                let p = at(&h, 10.0, 10.0);
                h.input_mut()
                    .events
                    .extend([Event::PointerMoved(p), press(p), release(p)]);
                h.run();
                assert!(h.state().editor().is_dirty());
                match kind {
                    "new" => h.key_combination_modifiers(Modifiers::COMMAND, &[Key::N]),
                    "close" => h.key_combination_modifiers(Modifiers::COMMAND, &[Key::W]),
                    "switch" => {
                        h.get_by_label("Interface").click();
                        h.run();
                        h.get_by_label("Studio").click();
                    }
                    _ => unreachable!(),
                }
                h.run();
                assert!(h.query_by_label("Cancel").is_some());
                h.render()
                    .unwrap()
                    .save(output.join(format!("{theme}-{name}-{kind}.png")))
                    .unwrap();
                h.get_by_label("Cancel").click();
                h.run();
                assert!(h.state().editor().is_dirty());
                drop(h);
                std::fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
