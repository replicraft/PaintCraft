//! Responsive color dialog presentation, with a fixed acceptance footer.
use super::*;

pub fn edit_colors_dialog(ctx: &egui::Context, editor: &mut ColorEditor) -> ColorResponse {
    let mut result = ColorResponse::Pending;
    let width = (ctx.content_rect().width() - 48.0).clamp(240.0, 760.0);
    let height = (ctx.content_rect().height() - 64.0).clamp(180.0, 640.0);
    let body_height = (height - 100.0).max(60.0);
    let modal = egui::Modal::new(egui::Id::unique("classic-edit-colors")).show(ctx, |ui| {
        ui.set_width(width);
        ui.set_max_height(height);
        ui.spacing_mut().interact_size.y = 36.0;
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(15.0));
        }
        ui.heading("Edit colors");
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .id_salt("classic-color-dialog-body")
            .max_height(body_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                picker_and_fields(ui, editor, width);
                ui.add_space(24.0);
                if width >= 600.0 {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            basic_heading(ui);
                            palette(ui, editor, 12);
                        });
                        ui.add_space(24.0);
                        ui.vertical(|ui| custom(ui, editor));
                    });
                } else {
                    basic_heading(ui);
                    palette(ui, editor, ((width / 25.0) as usize).clamp(4, 12));
                    ui.add_space(12.0);
                    custom(ui, editor);
                }
            });
        ui.add_space(12.0);
        ui.separator();
        let button_width = (width - ui.spacing().item_spacing.x) * 0.5;
        ui.horizontal(|ui| {
            let ink = if ui.visuals().dark_mode {
                Color32::from_gray(16)
            } else {
                Color32::WHITE
            };
            let accept = egui::Button::new(egui::RichText::new("OK").color(ink))
                .fill(ui.visuals().selection.stroke.color)
                .min_size(Vec2::new(button_width, 36.0));
            if ui.add_enabled(editor.valid(), accept).clicked() {
                result = ColorResponse::Accept;
            }
            if ui
                .add_sized([button_width, 36.0], egui::Button::new("Cancel"))
                .clicked()
            {
                result = ColorResponse::Cancel;
            }
        });
    });
    if modal.should_close() {
        result = ColorResponse::Cancel;
    }
    result
}

fn picker_and_fields(ui: &mut egui::Ui, editor: &mut ColorEditor, width: f32) {
    let wide = width >= 600.0;
    let side = if wide {
        (width - 300.0).clamp(180.0, 320.0)
    } else {
        (width - 84.0).clamp(120.0, 300.0)
    };
    ui.horizontal_top(|ui| {
        picker::square(ui, editor, side);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, side), egui::Sense::hover());
        ui.painter().rect(
            rect,
            4.0,
            Color32::from_rgb(editor.color.0[0], editor.color.0[1], editor.color.0[2]),
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::ColorWell,
                ui.is_enabled(),
                "Current color preview",
            )
        });
        picker::brightness(ui, editor, side);
        if wide {
            ui.add_space(8.0);
            let fields_width = ui.available_width();
            ui.vertical(|ui| fields::controls(ui, editor, fields_width));
        }
    });
    if !wide {
        ui.add_space(12.0);
        fields::controls(ui, editor, width);
    }
}

fn custom(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    ui.horizontal(|ui| {
        ui.label("Custom colors");
        let add = ui.add_enabled(editor.valid(), egui::Button::new("+"));
        add.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                ui.is_enabled() && editor.valid(),
                "Add custom color",
            )
        });
        if add
            .on_hover_text("Add current color to the selected custom slot")
            .clicked()
            && ui.ctx().current_pass_index() == 0
        {
            let mut color = editor.color;
            color.0[3] = 255;
            editor.custom[editor.selected_slot] = Some(color);
            editor.selected_slot = editor
                .custom
                .iter()
                .position(Option::is_none)
                .unwrap_or((editor.selected_slot + 1) % CUSTOM_COLOR_COUNT);
        }
    });
    custom_palette(ui, editor);
}

fn basic_heading(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        ui.label("Basic colors");
    });
}
