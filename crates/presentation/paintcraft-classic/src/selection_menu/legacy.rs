//! Compact historical selection rows and section captions.
use crate::ClassicVariant;
use egui::{Align2, Color32, FontId, Rect, Vec2};
use paintcraft_ui::{
    i18n::tr,
    icons::{self, Icon},
};

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
    let label = tr(ui.ctx(), label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), &label));
    ui.painter().text(
        rect.left_center() + Vec2::new(6.0, 0.0),
        Align2::LEFT_CENTER,
        &label,
        FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
}

pub(super) fn row(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    label: &str,
    accessible: &str,
    icon: Option<Icon>,
    selected: Option<bool>,
    shortcut: Option<&str>,
) -> egui::Response {
    let response = crate::legacy_surface::button(
        ui,
        variant,
        egui::Button::new(" ").min_size(Vec2::new(ui.available_width().max(220.0), 24.0)),
        false,
    );
    let accessible = tr(ui.ctx(), accessible);
    response.widget_info(|| match selected {
        Some(selected) => {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &accessible)
        }
        None => egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &accessible),
    });
    let icon_rect = Rect::from_center_size(
        response.rect.left_center() + Vec2::new(14.0, 0.0),
        Vec2::splat(16.0),
    );
    if selected == Some(true) {
        ui.painter()
            .rect_filled(icon_rect.expand(2.0), 1, ui.visuals().selection.bg_fill);
        ui.painter().rect_stroke(
            icon_rect.expand(2.0),
            1,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    if let Some(icon) = icon {
        if matches!(icon, Icon::RectangleSelect | Icon::DeleteLayer) {
            icons::legacy_image(ui.ctx(), icon, 16.0).paint_at(ui, icon_rect);
        } else {
            let ink = if ui.visuals().dark_mode {
                Color32::from_rgb(150, 205, 240)
            } else {
                Color32::from_rgb(46, 91, 130)
            };
            icons::classic_menu_image(ui.ctx(), icon, 16.0, ink).paint_at(ui, icon_rect);
        }
    }
    ui.painter().text(
        response.rect.left_center() + Vec2::new(32.0, 0.0),
        Align2::LEFT_CENTER,
        tr(ui.ctx(), label),
        FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            response.rect.right_center() - Vec2::new(8.0, 0.0),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
    response
}
