//! Ribbon access to existing shape thickness; samples are UI illustrations.
use super::Controls;
use egui::{Button, Rect, Vec2};
use paintcraft_tools::Tool;
use paintcraft_ui::{actions::Context, icons::Icon};

pub(super) fn show(ui: &mut egui::Ui, state: &mut Controls, context: Context) {
    let supported = matches!(
        state.tool,
        Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Preset(_) | Tool::Polygon | Tool::Curve
    );
    let enabled = supported && context.document_open && !context.blocked;
    ui.add_enabled_ui(enabled, |ui| {
        let target = if super::ribbon::spacious(ui) {
            Vec2::splat(40.0)
        } else {
            ui.spacing_mut().interact_size.y = 28.0;
            ui.spacing_mut().button_padding.y = 2.0;
            Vec2::new(54.0, 28.0)
        };
        let button = Button::new("")
            .min_size(target)
            .frame_when_inactive(false)
            .frame(true);
        let (response, _) = egui::containers::menu::MenuButton::from_button(button)
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                let height = super::popup_layout::height(ui, 40.0, 240.0);
                ui.set_width(220.0);
                ui.spacing_mut().button_padding.x = 12.0;
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.visuals_mut().weak_text_color =
                    Some(egui::Color32::from_gray(if ui.visuals().dark_mode {
                        184
                    } else {
                        88
                    }));
                for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    ui.style_mut()
                        .text_styles
                        .insert(style, egui::FontId::proportional(16.0));
                }
                let custom = ![1, 3, 5, 8].contains(&state.options.size);
                egui::ScrollArea::vertical()
                    .id_salt("classic-stroke-size-presets")
                    .max_height((height - if custom { 24.0 } else { 0.0 }).max(40.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for size in [1, 3, 5, 8] {
                            let selected = state.options.size == size;
                            let row = super::control_surface::button(
                                ui,
                                Button::new(())
                                    .left_text(format!("{size} px"))
                                    .frame_when_inactive(selected)
                                    .min_size(Vec2::new(220.0, 40.0)),
                                selected,
                            );
                            ui.painter().rect_filled(
                                Rect::from_center_size(
                                    row.rect.center() + Vec2::new(24.0, 0.0),
                                    Vec2::new(112.0, size as f32),
                                ),
                                0,
                                ui.visuals().strong_text_color(),
                            );
                            if row.clicked() && ui.ctx().current_pass_index() == 0 {
                                state.options.size = size;
                                ui.close();
                            }
                        }
                    });
                if custom {
                    ui.add_sized(
                        [220.0, 24.0],
                        egui::Label::new(
                            egui::RichText::new(format!("Current: {} px", state.options.size))
                                .weak(),
                        ),
                    );
                }
            });
        paintcraft_ui::icons::classic_menu_image(
            ui.ctx(),
            Icon::StrokeSize,
            22.0,
            ui.visuals().strong_text_color(),
        )
        .paint_at(
            ui,
            Rect::from_center_size(response.rect.center(), Vec2::splat(22.0)),
        );
        super::dropdowns::open_state(ui, &response, "Stroke size");
        if enabled {
            response.on_hover_text(paintcraft_ui::i18n::tr(ui.ctx(), "Stroke size"));
        } else {
            response.on_disabled_hover_text(paintcraft_ui::i18n::tr(
                ui.ctx(),
                "Select a line, rectangle or ellipse to change stroke size.",
            ));
        }
    });
}
