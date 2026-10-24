use eframe::egui::{self, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_web::WebApp;

#[test]
fn preview_uses_classic_chrome_and_exposes_unsupported_actions_honestly() {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    harness.run();
    let layers = harness.get_by_role_and_label(Role::Button, "Layers").rect();
    let colors = harness.get_by_label("Edit colors…").rect();
    assert!((layers.center().y - colors.center().y).abs() < 1.0);
    harness.get_by_label("Save").click();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Save is not connected")
            .is_some()
    );
    harness
        .get_by_role_and_label(Role::Button, "Layers")
        .click();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Document layers are not connected")
            .is_some()
    );
    harness.get_by_label("Settings…").click();
    harness.run();
    harness.get_by_label("Dark mode").click();
    harness.run();
    assert_eq!(harness.ctx.theme(), egui::Theme::Dark);
}
