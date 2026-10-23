use super::*;
use eframe::egui::{Role, ViewportCommand, ViewportId};
use paintcraft::settings::{ClassicVariant, Experience, Settings};

fn view_menu(h: &mut Harness<App>) {
    h.get_by_role_and_label(Role::Button, "View").click();
    h.run();
}

fn fullscreen_command(h: &Harness<App>, expected: bool) -> bool {
    h.output().viewport_output[&ViewportId::ROOT]
        .commands
        .iter()
        .any(|command| matches!(command, ViewportCommand::Fullscreen(value) if *value == expected))
}

// Queue key-down/up in one input frame so the viewport command is still in
// the inspected output (Harness::key_press otherwise runs each event separately).
fn tap(h: &mut Harness<App>, code: Key) {
    h.input_mut()
        .events
        .extend([key(code, Modifiers::NONE), key_up(code, Modifiers::NONE)]);
}

#[test]
fn classic_status_visibility_persists_without_changing_document_and_studio_keeps_its_status() {
    let dir = scratch("workspace-status");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    let base = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let pixels = h.state().rendered().to_vec();
    let height = h.state().canvas_viewport().unwrap().height();
    view_menu(&mut h);
    assert_eq!(
        h.get_by_label("Status bar").accesskit_node().toggled(),
        Some(eframe::egui::accesskit::Toggled::True)
    );
    h.get_by_label("Status bar").click();
    h.run();
    assert!(h.query_by_label("Zoom percentage").is_none());
    assert!(h.state().canvas_viewport().unwrap().height() > height + 10.0);
    assert!(!Settings::load(&dir).0.classic_status_bar);
    assert!(h.state().editor().document().same_semantics(&base));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert_eq!(h.state().rendered(), pixels);
    drop(h);
    let mut restarted = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(restarted.query_by_label("Zoom percentage").is_none());
    view_menu(&mut restarted);
    assert_eq!(
        restarted
            .get_by_label("Status bar")
            .accesskit_node()
            .toggled(),
        Some(eframe::egui::accesskit::Toggled::False)
    );
    drop(restarted);
    let mut settings = Settings::load(&dir).0;
    settings.experience = Experience::Studio;
    settings.save(&dir).unwrap();
    let studio = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(studio.query_by_label("320 × 320 px").is_some());
    drop(studio);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fullscreen_menu_f11_and_escape_emit_window_commands_without_history() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let base = h.state().editor().document_snapshot();
    view_menu(&mut h);
    h.get_by_label_contains("Full screen").click();
    h.step();
    assert!(fullscreen_command(&h, true));
    h.run();
    tap(&mut h, Key::F11);
    h.step();
    assert!(fullscreen_command(&h, false));
    h.run();
    tap(&mut h, Key::F11);
    h.step();
    assert!(fullscreen_command(&h, true));
    h.run();
    tap(&mut h, Key::Escape);
    h.step();
    assert!(fullscreen_command(&h, false));
    h.run();
    assert!(h.state().editor().document().same_semantics(&base));
    assert!(!h.state().editor().is_dirty());
}

#[test]
fn classic_view_labels_align_across_icons_and_toggle_states() {
    use eframe::egui::{accesskit::Toggled, epaint::Shape};
    let dir = scratch("view-marker-alignment");
    Settings {
        classic_variant: Some(ClassicVariant::V11),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, true);
    let base = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    for checked in [false, true] {
        view_menu(&mut h);
        let mut positions = Vec::new();
        for label in [
            "Zoom",
            "Rulers",
            "Gridlines",
            "Status bar",
            "Full screen",
            "Thumbnail",
            "Default canvas size…",
            "Change Classic skin…",
        ] {
            let x = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Text(text) if text.galley.job.text.trim() == label => {
                        let row = text.galley.rows.first()?;
                        let glyph = row.glyphs.iter().find(|glyph| !glyph.chr.is_whitespace())?;
                        Some(text.pos.x + row.pos.x + glyph.pos.x)
                    }
                    _ => None,
                })
                .expect("menu label is painted");
            positions.push(x);
        }
        assert!(
            positions.iter().all(|x| (x - positions[0]).abs() <= 1.0),
            "{positions:?}"
        );
        let full = h.get_by_label_contains("Full screen");
        assert_eq!(
            full.accesskit_node().toggled(),
            Some(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
        );
        if !checked {
            let rect = full.rect();
            assert!(
                h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Rect(image) if image.brush.is_some()
                        && rect.contains_rect(image.rect)
                        && (image.rect.width() - 22.0).abs() < 0.1
                )),
                "unchecked Full screen must paint its glyph"
            );
            full.click();
            h.step();
            assert!(fullscreen_command(&h, true));
            h.run();
        } else {
            h.key_press(Key::Escape);
            h.run();
        }
    }
    assert!(h.state().editor().document().same_semantics(&base));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn escape_cancels_pending_text_before_leaving_fullscreen() {
    let mut h = harness();
    h.get_by_label("Text").click();
    h.run();
    drag(&mut h, (50.0, 50.0), (50.0, 50.0));
    h.event(Event::Text("draft".into()));
    h.run();
    assert!(h.state().editor().is_editing_text());
    tap(&mut h, Key::F11);
    h.step();
    assert!(fullscreen_command(&h, true));
    h.run();
    tap(&mut h, Key::Escape);
    h.step();
    assert!(!h.state().editor().is_editing_text());
    assert!(!fullscreen_command(&h, false));
    h.run();
    tap(&mut h, Key::Escape);
    h.step();
    assert!(fullscreen_command(&h, false));
}

#[test]
#[ignore = "requires a native GPU; exports Classic View menu and status visibility"]
fn render_classic_workspace_view_for_review() {
    use paintcraft::settings::Appearance;
    let output = PathBuf::from("/tmp/paintcraft-workspace-view");
    std::fs::create_dir_all(&output).unwrap();
    for (name, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        let dir = scratch(&format!("workspace-view-{name}"));
        Settings {
            classic_variant: Some(ClassicVariant::V11),
            appearance,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        view_menu(&mut h);
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-view-menu.png")))
            .unwrap();
        h.get_by_role_and_label(Role::Button, "Zoom").hover();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-zoom-menu.png")))
            .unwrap();
        h.key_press(Key::ArrowLeft);
        h.run();
        h.get_by_label("Status bar").click();
        h.run();
        h.render()
            .unwrap()
            .save(output.join(format!("{name}-status-hidden.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_gridlines_menu_and_shortcut_share_state_without_changing_document() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let base = h.state().editor().document_snapshot();
    view_menu(&mut h);
    assert!(h.query_by_label_contains("Gridlines").is_some());
    assert!(h.query_by_label("Pixel grid").is_none());
    h.get_by_label_contains("Gridlines").click();
    h.run();
    view_menu(&mut h);
    assert_eq!(
        h.get_by_label_contains("Gridlines")
            .accesskit_node()
            .toggled(),
        Some(eframe::egui::accesskit::Toggled::True)
    );
    h.key_press(Key::Escape);
    h.run();
    h.input_mut().events.extend([
        key(Key::G, Modifiers::COMMAND),
        key_up(Key::G, Modifiers::COMMAND),
    ]);
    h.run();
    view_menu(&mut h);
    assert_eq!(
        h.get_by_label_contains("Gridlines")
            .accesskit_node()
            .toggled(),
        Some(eframe::egui::accesskit::Toggled::False)
    );
    assert!(h.state().editor().document().same_semantics(&base));
}
