use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, translate};

fn harness(skin: ClassicVariant, language: Language) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.variant = skin;
            app.preferences.variant = skin;
            app.preferences.language = language;
            set_language(&cc.egui_ctx, language);
            app
        });
    h.run();
    h
}
fn fire(h: &mut Harness<WebApp>, view: ViewAction) {
    let ctx = h.ctx.clone();
    h.state_mut().action(&ctx, Action::View(view));
    h.run();
}

#[test]
fn rulers_grid_status_and_thumbnail_are_presentation_only_in_every_classic_skin() {
    for skin in ClassicVariant::ALL {
        for language in [Language::English, Language::Japanese] {
            let mut h = harness(skin, language);
            let ticket = h.state().editor.session.ticket();
            let original =
                paintcraft_formats::pcnd::write(h.state().editor.session.document()).unwrap();
            fire(&mut h, ViewAction::Quadruple);
            fire(&mut h, ViewAction::PixelGrid);
            assert!(h.state().context().pixel_grid);
            assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { stroke, .. } if stroke.color == egui::Color32::from_black_alpha(72))));
            fire(&mut h, ViewAction::Rulers);
            assert!(h.state().context().rulers);
            assert!(
                h.query_by_label(&translate(language, "Horizontal ruler (pixels)"))
                    .is_some()
            );
            let viewport = h.state().canvas.viewport;
            fire(&mut h, ViewAction::StatusBar);
            assert!(!h.state().context().status_bar);
            assert!(h.state().canvas.viewport.y > viewport.y);
            assert!(
                h.query_by_label(&translate(language, "Canvas zoom"))
                    .is_none()
            );
            fire(&mut h, ViewAction::StatusBar);
            assert!(
                h.query_by_label(&translate(language, "Canvas zoom"))
                    .is_some()
            );
            fire(&mut h, ViewAction::Thumbnail);
            assert!(
                h.query_by_label(&translate(language, "Canvas thumbnail"))
                    .is_some()
            );
            assert!(
                h.query_by_label(&translate(language, "Preview unavailable"))
                    .is_none()
            );
            h.get_by_label(&translate(language, "Close thumbnail"))
                .click();
            h.run();
            assert!(!h.state().context().thumbnail);
            assert_eq!(h.state().editor.session.ticket(), ticket);
            assert_eq!(
                paintcraft_formats::pcnd::write(h.state().editor.session.document()).unwrap(),
                original
            );
            assert_eq!(h.state().editor.session.undo_depth(), 0);
        }
    }
}

#[test]
fn actual_view_menu_hides_reveals_and_restores_toolbar_without_publishing() {
    let mut h = harness(ClassicVariant::V11, Language::English);
    let ticket = h.state().editor.session.ticket();
    let initial = h.state().canvas.viewport.y;
    h.get_by_role_and_label(egui::Role::Button, "View").click();
    h.run();
    h.get_by_label("Automatically hide toolbar").click();
    h.run();
    h.event(egui::Event::PointerMoved(
        h.state().canvas.rect.unwrap().center(),
    ));
    h.run();
    assert!(h.state().context().auto_hide_toolbar);
    assert!(h.state().canvas.viewport.y > initial + 80.0);
    assert!(
        h.query_by_role_and_label(egui::Role::Button, "Pencil")
            .is_none()
    );
    h.get_by_role_and_label(egui::Role::Button, "View").hover();
    h.run();
    assert!(
        h.query_by_role_and_label(egui::Role::Button, "Pencil")
            .is_some()
    );
    h.get_by_role_and_label(egui::Role::Button, "View").click();
    h.run();
    h.get_by_label("Always show toolbar").click();
    h.run();
    h.event(egui::Event::PointerMoved(
        h.state().canvas.rect.unwrap().center(),
    ));
    h.run();
    assert!(!h.state().context().auto_hide_toolbar);
    assert!(
        h.query_by_role_and_label(egui::Role::Button, "Pencil")
            .is_some()
    );
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

#[test]
fn grid_and_thumbnail_state_are_owned_by_each_frontend_workspace() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.action(&ctx, Action::View(ViewAction::PixelGrid));
    app.action(&ctx, Action::View(ViewAction::Thumbnail));
    app.action(&ctx, Action::ChooseFrontend(Frontend::Studio));
    assert!(!app.context().pixel_grid && !app.context().thumbnail);
    app.action(&ctx, Action::ChooseFrontend(Frontend::Classic));
    assert!(app.context().pixel_grid && app.context().thumbnail);
}
