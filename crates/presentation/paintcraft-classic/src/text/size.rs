//! Point-size entry and presets for the text toolbar.
use super::*;

const FONT_SIZES: [u16; 16] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];

pub(super) fn font_size(ui: &mut Ui, points: &mut u16) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let size = ui.add_sized(
            [66.0, CONTROL_HEIGHT],
            egui::DragValue::new(points).range(1..=512),
        );
        size.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), "Font size")
        });
        size.on_hover_text("Font size in points · type a value or choose a preset");
        let (response, _) = egui::containers::menu::MenuButton::from_button(
            Button::new(" ").min_size(Vec2::new(24.0, CONTROL_HEIGHT)),
        )
        .ui(ui, |ui| {
            ui.set_min_width(90.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical()
                .id_salt("classic-font-size-presets")
                .max_height((ui.ctx().content_rect().height() * 0.5 - 24.0).clamp(72.0, 280.0))
                .show(ui, |ui| {
                    for size in FONT_SIZES {
                        let selected = *points == size;
                        let choice = ui.add_sized(
                            [ui.available_width(), CONTROL_HEIGHT],
                            Button::new(size.to_string())
                                .selected(selected)
                                .frame_when_inactive(selected),
                        );
                        selected_outline(ui, &choice, selected);
                        choice.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                ui.is_enabled(),
                                *points == size,
                                format!("{size} pt"),
                            )
                        });
                        if choice.clicked() {
                            *points = size;
                            ui.close();
                        }
                    }
                });
        });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Font size presets")
        });
        let center = response.rect.center();
        ui.painter().add(egui::Shape::line(
            vec![
                center + Vec2::new(-3.0, -1.5),
                center + Vec2::new(0.0, 1.5),
                center + Vec2::new(3.0, -1.5),
            ],
            egui::Stroke::new(1.0, ui.visuals().text_color()),
        ));
        response.on_hover_text("Font size presets");
    });
}
