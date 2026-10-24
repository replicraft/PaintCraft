//! Compact historical selection rows and section captions.
use egui::{Align2, Color32, FontId, Vec2};
use paintcraft_ui::t;
pub(super) fn heading(ui: &mut egui::Ui, label: &str) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        0,
        if ui.visuals().dark_mode {
            Color32::from_gray(48)
        } else {
            Color32::from_rgb(235, 238, 243)
        },
    );
    let label = t!(ui.ctx(), label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), &label));
    ui.painter().text(
        rect.left_center() + Vec2::new(6.0, 0.0),
        Align2::LEFT_CENTER,
        &label,
        FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
}

pub(super) use crate::legacy_surface::menu_row as row;
