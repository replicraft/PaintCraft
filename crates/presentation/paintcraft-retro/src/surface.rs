//! Local classic controls; no global theme or font definition changes.
use egui::{Color32, Stroke, Vec2};

pub fn workspace_color() -> Color32 {
    Color32::from_gray(128)
}
pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_gray(192))
        .inner_margin(egui::Margin::symmetric(3, 2))
}

pub fn with_style<R>(ui: &mut egui::Ui, show: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::from_gray(192);
        style.visuals.window_fill = Color32::from_gray(192);
        style.visuals.override_text_color = Some(Color32::BLACK);
        style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
        style.spacing.item_spacing = Vec2::new(3.0, 3.0);
        style.spacing.button_padding = Vec2::new(4.0, 2.0);
        for font in style.text_styles.values_mut() {
            font.family = paintcraft_ui::fonts::retro_family();
            font.size = 12.0;
        }
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            widget.corner_radius = egui::CornerRadius::ZERO;
            widget.weak_bg_fill = Color32::from_gray(192);
        }
        show(ui)
    })
    .inner
}

pub(super) fn bevel(ui: &egui::Ui, rect: egui::Rect, inset: bool) {
    let (light, dark) = if inset {
        (Color32::from_gray(96), Color32::WHITE)
    } else {
        (Color32::WHITE, Color32::from_gray(96))
    };
    for (a, b, color) in [
        (rect.left_bottom(), rect.left_top(), light),
        (rect.left_top(), rect.right_top(), light),
        (rect.right_top(), rect.right_bottom(), dark),
        (rect.right_bottom(), rect.left_bottom(), dark),
    ] {
        ui.painter().line_segment([a, b], Stroke::new(1.0, color));
    }
}
