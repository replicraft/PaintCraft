use super::*;
use eframe::egui::{Role, ViewportCommand, ViewportEvent, ViewportId};

fn file(h: &mut Harness<App>) {
    h.get_by_role_and_label(Role::Button, "File").click();
    h.run();
}

#[test]
fn classic_quick_access_follows_the_reference_and_keeps_file_actions_in_the_menu() {
    let mut h = harness_with(App::new(None), 2.0, true);
    h.ctx.style_mut_of(h.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    let snapshot = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    let mut previous_right = 0.0;
    for label in ["Save", "Share", "Undo", "Redo", "Clear Canvas"] {
        let button = h.get_by_role_and_label(Role::Button, label);
        assert!(
            button.rect().left() > previous_right,
            "{label} must follow the preceding quick action"
        );
        previous_right = button.rect().right();
    }
    for label in ["New", "Open", "Export"] {
        assert!(h.query_by_label(label).is_none());
    }
    assert!(
        h.get_by_role_and_label(Role::Button, "Share")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(Role::Button, "Share").hover();
    h.run();
    assert_eq!(
        h.query_all_by_role_and_label(Role::Label, "System sharing is not available yet.")
            .count(),
        1
    );
    h.get_by_role_and_label(Role::Button, "Share").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&snapshot));
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    file(&mut h);
    for label in ["New", "Open"] {
        assert!(
            !h.get_by_role_and_label(Role::Button, "File")
                .get_by_label_contains(label)
                .accesskit_node()
                .is_disabled()
        );
    }
    h.key_press(Key::Escape);
    h.run();
}

#[test]
fn classic_file_exposes_native_print_and_keeps_unimplemented_actions_disabled() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let before = h.state().editor().document_snapshot();
    let history = h.state().editor().history_view().ticket;
    file(&mut h);
    for label in ["Share", "Set as desktop background"] {
        assert!(
            h.get_by_role_and_label(Role::Button, "File")
                .get_by_label(label)
                .accesskit_node()
                .is_disabled(),
            "{label}"
        );
    }
    h.get_by_label("Import to canvas").hover();
    h.run();
    for label in ["From file…", "From scanner or camera…"] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.get_by_label("Print").hover();
    h.run();
    let print = h.get_by_label_contains("Print…");
    assert_eq!(
        print.accesskit_node().is_disabled(),
        !cfg!(target_os = "macos")
    );
    // Inspect availability without invoking a real print dialog or printer job.
    for label in ["Page setup…", "Print preview"] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.get_by_role_and_label(Role::Button, "Edit").hover();
    h.run();
    for label in ["Cut", "Copy", "Paste", "Copy visible layers"] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
    assert!(h.state().editor().document().same_semantics(&before));
    assert_eq!(h.state().editor().history_view().ticket, history);
    h.key_press(Key::Escape);
    h.run();
}

#[test]
fn classic_file_rows_keep_distinct_icons_and_unavailable_submenu_markers() {
    use eframe::egui::epaint::Shape;
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("file-preview-icons");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        let base = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        file(&mut h);
        let mut textures = std::collections::HashSet::new();
        for label in [
            "Open",
            "Import to canvas",
            "Recent",
            "Set as desktop background",
            "Image properties",
        ] {
            let row = h
                .get_by_role_and_label(Role::Button, "File")
                .get_by_label_contains(label);
            let rect = row.rect();
            let texture = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Rect(image) if rect.contains_rect(image.rect) => {
                        image.brush.as_ref().map(|brush| brush.fill_texture_id)
                    }
                    _ => None,
                })
                .expect("menu row paints an image");
            assert!(
                textures.insert(texture),
                "{label} must have its own artwork"
            );
            if label == "Recent" {
                assert!(!row.accesskit_node().is_disabled());
            }
            if label == "Set as desktop background" {
                assert!(row.accesskit_node().is_disabled());
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    Shape::Path(path) if path.points.len() == 3
                        && path.points.iter().all(|point| rect.contains(*point) && point.x > rect.right() - 24.0)
                )), "{label} must retain a submenu arrow");
                row.click();
                h.run();
                assert!(h.query_by_label("From file…").is_none());
                // Menu dismissal on a disabled-row click is owned by egui.
                if !eframe::egui::Popup::is_any_open(&h.ctx) {
                    file(&mut h);
                }
            }
        }
        assert!(h.state().editor().document().same_semantics(&base));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn classic_exit_requests_window_close_and_the_unsaved_guard_can_cancel_it() {
    let mut h = harness_with(App::new(None), 1.0, true);
    with_layer(&mut h);
    let before = h.state().editor().document_snapshot();
    file(&mut h);
    h.get_by_label("Exit").click();
    h.step();
    assert!(
        h.output().viewport_output[&ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, ViewportCommand::Close))
    );
    // eframe delivers the close request as a viewport event. Exercise the same
    // acknowledgement that the OS/native menu close path receives.
    h.input_mut()
        .viewports
        .entry(ViewportId::ROOT)
        .or_default()
        .events
        .push(ViewportEvent::Close);
    h.step();
    h.run();
    assert!(h.query_by_label("Quit anyway").is_some());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().editor().is_dirty());
    assert!(h.state().editor().document().same_semantics(&before));
}

#[test]
fn classic_file_menu_keeps_exit_reachable_in_a_short_window() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1100.0, 500.0))
        .with_pixels_per_point(2.0)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), App::new(None));
    h.run();
    file(&mut h);
    h.get_by_label("Exit").scroll_to_me();
    h.run();
    let rect = h.get_by_label("Exit").rect();
    assert!(rect.min.y >= 0.0 && rect.max.y <= 500.0, "{rect:?}");
}

#[test]
#[ignore = "requires native GPU; exports actual Classic File/Edit menu scenes"]
fn export_classic_file_and_edit_menus() {
    use paintcraft::settings::{Appearance, ClassicVariant, Settings};
    let output = PathBuf::from("/tmp/paintcraft-classic-file-edit");
    std::fs::create_dir_all(&output).unwrap();
    for (theme, appearance) in [("light", Appearance::Light), ("dark", Appearance::Dark)] {
        for scene in ["file", "edit", "save-as", "print"] {
            let dir = scratch("classic-file-edit-preview");
            Settings {
                appearance,
                classic_variant: Some(ClassicVariant::V11),
                ..Default::default()
            }
            .save(&dir)
            .unwrap();
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_ui_state(
                    |ui, app: &mut App| app.ui(ui),
                    App::new(None).with_settings(dir.clone()),
                );
            h.run();
            file(&mut h);
            match scene {
                "edit" => h.get_by_role_and_label(Role::Button, "Edit").hover(),
                "save-as" => h.get_by_label("Save as").hover(),
                "print" => h.get_by_label("Print").hover(),
                _ => {}
            }
            h.run();
            h.render()
                .unwrap()
                .save(output.join(format!("classic-{theme}-{scene}.png")))
                .unwrap();
            drop(h);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
