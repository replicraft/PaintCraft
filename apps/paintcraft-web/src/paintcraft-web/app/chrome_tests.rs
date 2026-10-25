use super::*;
use egui::{Color32, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};

pub(super) fn harness(
    frontend: Frontend,
    variant: ClassicVariant,
    width: f32,
) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(width, 900.0))
        .build_eframe(move |cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.frontend = frontend;
            app.variant = variant;
            app.preferences.variant = variant;
            app.preferences.theme = Some(egui::Theme::Dark);
            app
        });
    h.run();
    h
}

fn fill(h: &Harness<WebApp>, rect: egui::Rect) -> Color32 {
    h.output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(shape) if shape.rect == rect => Some(shape.fill),
            _ => None,
        })
        .expect("painted download target")
}

#[test]
fn browser_download_is_centered_hoverable_and_exclusive_to_paint11() {
    for frontend in [Frontend::Classic, Frontend::Studio] {
        for variant in ClassicVariant::ALL {
            let mut h = harness(frontend, variant, 1360.0);
            let allowed = frontend == Frontend::Classic && variant == ClassicVariant::V11;
            assert_eq!(h.query_by_label("Download for desktop").is_some(), allowed);
            if allowed {
                let rect = h.get_by_label("Download for desktop").rect();
                assert!(
                    (rect.center().y - h.get_by_label("Settings…").rect().center().y).abs() < 1.0
                );
                let before = fill(&h, rect);
                h.get_by_label("Download for desktop").hover();
                h.run();
                assert_ne!(fill(&h, rect), before);
                h.get_by_label("Download for desktop").click();
                h.step();
                assert!(h.output().platform_output.commands.iter().any(|command| {
                    matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == "/download" && url.new_tab)
                }));
            }
            h.get_by_role_and_label(Role::Button, "File").click();
            h.run();
            assert!(
                h.query_by_label("Exit").is_none(),
                "{frontend:?} {variant:?}"
            );
        }
    }
}
