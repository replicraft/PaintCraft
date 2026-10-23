use super::*;
use eframe::egui::{Role, TextureId, epaint::Shape};
use paintcraft::settings::{Appearance, ClassicVariant, Settings};

fn preview_texture(h: &Harness<App>, label: &str) -> Option<TextureId> {
    let row = h.get_by_role_and_label(Role::Button, label).rect();
    h.output().shapes.iter().find_map(|shape| {
        let Shape::Mesh(mesh) = &shape.shape else {
            return None;
        };
        let bounds = mesh.calc_bounds();
        (row.contains_rect(bounds)
            && bounds.width() > 30.0
            && bounds.width() <= 32.1
            && bounds.center().x < row.left() + 40.0)
            .then_some(mesh.texture_id)
    })
}

fn await_preview(h: &mut Harness<App>, label: &str, old: Option<TextureId>) -> TextureId {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        h.step();
        if let Some(id) = preview_texture(h, label)
            && Some(id) != old
        {
            return id;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "preview did not update for {label}"
        );
        std::thread::yield_now();
    }
}

#[test]
fn classic_layer_previews_follow_commits_and_undo_without_publishing_or_moving_properties() {
    for (ppp, appearance) in [(1.0, Appearance::Light), (2.0, Appearance::Dark)] {
        let dir = scratch("layer-previews");
        Settings {
            appearance,
            classic_variant: Some(ClassicVariant::V11),
            ..Default::default()
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), ppp, true);
        with_layer(&mut h);
        let snapshot = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let published = publications(&h);
        let field = h.get_by_label("Layer name").rect();
        let initial = await_preview(&mut h, "Layer 2", None);
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(publications(&h), published);
        assert_eq!(h.get_by_label("Layer name").rect(), field);
        drag(&mut h, (30.0, 30.0), (80.0, 60.0));
        let painted = await_preview(&mut h, "Layer 2", Some(initial));
        assert_eq!(publications(&h), published + 1);
        h.get_by_role_and_label(Role::Button, "Undo").click();
        h.run();
        await_preview(&mut h, "Layer 2", Some(painted));
        assert!(h.state().editor().document().same_semantics(&snapshot));
        assert_eq!(publications(&h), published + 2);
        assert_eq!(h.get_by_label("Layer name").rect(), field);
        drop(h);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
