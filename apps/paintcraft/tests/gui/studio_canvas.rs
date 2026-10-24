use super::*;
use eframe::egui::Role;
use paintcraft::settings::{Appearance, Experience, Settings};
use paintcraft_core::types::canvas::Anchor;
use paintcraft_ui::i18n::{Language, tr};

fn open(h: &mut Harness<App>) {
    let image = tr(&h.ctx, "Image");
    h.get_by_role_and_label(Role::Button, &image).click();
    h.run();
    let label = tr(&h.ctx, "Canvas size…");
    h.get_by_label(&label).click();
    h.run();
}
#[test]
fn studio_canvas_size_has_nine_anchors_one_publication_exact_undo_and_no_classic_preference_writes()
{
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1360.0, 900.0));
    h.run();
    let source = h.state().editor().document_snapshot();
    let depth = h.state().editor().undo_depth();
    let saved = std::fs::read(dir.join("settings.json")).unwrap();
    open(&mut h);
    assert!(h.query_by_label("Remember last canvas size").is_none());
    let width = h.get_by_label("Width").rect();
    let height = h.get_by_label("Height").rect();
    assert_eq!(width.left(), height.left());
    assert_eq!(width.width(), height.width());
    assert_eq!(width.height(), 32.0);
    h.get_by_label("Width")
        .set_value_accesskit(f64::from(source.canvas.0 - 3));
    h.run();
    h.get_by_label("Height")
        .set_value_accesskit(f64::from(source.canvas.1 + 3));
    h.run();
    for anchor in Anchor::ALL {
        let label = format!("Anchor: {}", anchor.name());
        h.get_by_role_and_label(Role::Button, &label).click();
        h.run();
        assert_eq!(
            h.get_by_role_and_label(Role::Button, &label)
                .accesskit_node()
                .toggled(),
            Some(eframe::egui::accesskit::Toggled::True)
        );
        assert!(h.state().editor().document().same_semantics(&source));
    }
    h.get_by_role_and_label(Role::Button, "Anchor: Center")
        .click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    let changed = h.state().editor().document_snapshot();
    assert_eq!(changed.canvas, (source.canvas.0 - 3, source.canvas.1 + 3));
    assert_eq!(h.state().editor().undo_depth(), depth + 1);
    for id in &source.roots {
        assert_eq!(
            changed.layers[id].origin,
            (
                source.layers[id].origin.0 - 2,
                source.layers[id].origin.1 + 1
            )
        );
        assert_eq!(
            changed.layers[id].raster.bounds(),
            source.layers[id].raster.bounds()
        );
    }
    assert_eq!(std::fs::read(dir.join("settings.json")).unwrap(), saved);
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&source));
    h.get_by_label("Redo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&changed));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn studio_canvas_cancel_and_unchanged_apply_leave_pixels_selection_and_history_intact() {
    let (mut h, dir) = super::studio::studio_harness(true);
    let source = h.state().editor().document_snapshot();
    let selection = h.state().editor().selection();
    let ticket = h.state().editor().history_view().ticket;
    open(&mut h);
    h.get_by_label("Width").set_value_accesskit(20.0);
    h.run();
    h.get_by_role_and_label(Role::Button, "Anchor: Bottom right")
        .click();
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().editor().document().same_semantics(&source));
    assert_eq!(h.state().editor().selection(), selection);
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    open(&mut h);
    h.get_by_role_and_label(Role::Button, "Anchor: Bottom right")
        .click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(h.state().editor().document().same_semantics(&source));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn studio_canvas_size_keeps_actions_fixed_while_anchor_grid_scrolls_in_small_windows() {
    for (size, ppp, appearance, language) in [
        (
            Vec2::new(360.0, 260.0),
            1.0,
            Appearance::Light,
            Language::English,
        ),
        (
            Vec2::new(480.0, 360.0),
            2.0,
            Appearance::Dark,
            Language::Japanese,
        ),
    ] {
        let dir = scratch("studio-canvas-responsive");
        Settings {
            experience: Experience::Studio,
            appearance,
            language,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        open(&mut h);
        h.set_size(size);
        h.run();
        let apply = tr(&h.ctx, "Apply");
        let cancel = tr(&h.ctx, "Cancel");
        let anchor_name = format!("{}: {}", tr(&h.ctx, "Anchor"), tr(&h.ctx, "Bottom right"));
        let footer = h.get_by_label(&apply).rect();
        assert!(h.ctx.content_rect().contains_rect(footer));
        assert!(
            h.ctx
                .content_rect()
                .contains_rect(h.get_by_label(&cancel).rect())
        );
        h.get_by_role_and_label(Role::Button, &anchor_name)
            .scroll_to_me();
        h.run();
        let anchor = h.get_by_role_and_label(Role::Button, &anchor_name).rect();
        assert!(anchor.bottom() < footer.top(), "{size:?} / {ppp}");
        assert_eq!(h.get_by_label(&apply).rect(), footer);
        h.get_by_label(&cancel).click();
        h.run();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
#[ignore = "Explicit offscreen Studio canvas dialog review"]
fn export_studio_canvas_size_for_review() {
    let output = std::env::temp_dir().join("paintcraft-studio-canvas-size");
    std::fs::create_dir_all(&output).unwrap();
    for (appearance, language, name) in [
        (Appearance::Light, Language::English, "english-light"),
        (Appearance::Dark, Language::Japanese, "japanese-dark"),
    ] {
        let dir = scratch("studio-canvas-review");
        Settings {
            experience: Experience::Studio,
            appearance,
            language,
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        h.set_size(Vec2::new(1360.0, 900.0));
        h.run();
        open(&mut h);
        h.render()
            .unwrap()
            .save(output.join(format!("{name}.png")))
            .unwrap();
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
