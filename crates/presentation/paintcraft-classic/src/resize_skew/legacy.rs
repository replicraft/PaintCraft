//! Historical two-group layout; the host retains the preview draft.
use super::{Draft, numeric_info, size_field};
use egui::{Color32, Ui, Vec2};
use paintcraft_ui::i18n::tr;

pub(super) fn show(ctx: &egui::Context, draft: &mut Draft, maximum: u32) -> bool {
    let width = (ctx.content_rect().width() - 40.0).clamp(240.0, 300.0);
    let body_height = (ctx.content_rect().height() - 130.0).clamp(48.0, 360.0);
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("historical-resize-skew-preview"))
        .frame(
            egui::Frame::window(&ctx.style_of(ctx.theme()))
                .corner_radius(0)
                .inner_margin(10)
                .fill(Color32::from_gray(if ctx.theme() == egui::Theme::Dark {
                    43
                } else {
                    240
                })),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            crate::form_controls::style(ui);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            ui.style_mut().drag_value_text_style = egui::TextStyle::Body;
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, egui::FontId::proportional(12.0));
            }
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
            ] {
                state.corner_radius = 0.into();
            }
            ui.horizontal(|ui| {
                ui.strong(tr(ui.ctx(), "Resize and skew"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = tr(ui.ctx(), "Close resize and skew");
                    let response = ui.add_sized([22.0, 22.0], egui::Button::new("×"));
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, &label)
                    });
                    close |= response.on_hover_text(label).clicked();
                });
            });
            ui.separator();
            ui.spacing_mut().scroll.floating = false;
            egui::ScrollArea::vertical()
                .id_salt("historical-resize-body")
                .max_height(body_height)
                .min_scrolled_height(48.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    group(ui, "Resize", |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut draft.pixels, false, tr(ui.ctx(), "Percentage"));
                            ui.radio_value(&mut draft.pixels, true, tr(ui.ctx(), "Pixels"));
                        });
                        for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                            row(ui, label, |ui, width| {
                                ui.push_id(("resize-field", axis, draft.pixels), |ui| {
                                    size_field(ui, draft, axis, maximum, width, 24.0, label);
                                });
                            });
                        }
                        ui.checkbox(&mut draft.linked, tr(ui.ctx(), "Maintain aspect ratio"));
                    });
                    group(ui, "Skew", |ui| {
                        for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                            row(ui, label, |ui, width| {
                                let response = ui.add(
                                    egui::DragValue::new(&mut draft.skew[axis])
                                        .range(-89..=89)
                                        .suffix("°")
                                        .min_size(Vec2::new(width, 24.0)),
                                );
                                numeric_info(
                                    ui,
                                    &response,
                                    &format!("Skew {}", label.to_lowercase()),
                                    f64::from(draft.skew[axis]),
                                );
                            });
                        }
                    });
                });
            ui.label(tr(ui.ctx(), "Preview only")).on_hover_text(tr(
                ui.ctx(),
                "Resize and skew changes cannot be applied yet.",
            ));
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close |= ui
                    .add_sized([80.0, 24.0], egui::Button::new(tr(ui.ctx(), "Cancel")))
                    .clicked();
                ui.add_enabled(
                    false,
                    egui::Button::new(tr(ui.ctx(), "OK")).min_size(Vec2::new(80.0, 24.0)),
                )
                .on_disabled_hover_text(tr(
                    ui.ctx(),
                    "Applying resize and skew is not available yet.",
                ));
            });
        });
    close || modal.should_close()
}

fn group(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .corner_radius(0)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 1.0).max(0.0));
            ui.label(tr(ui.ctx(), title));
            body(ui);
        });
}

fn row(ui: &mut Ui, label: &str, body: impl FnOnce(&mut Ui, f32)) {
    ui.horizontal(|ui| {
        ui.add_sized([90.0, 24.0], egui::Label::new(tr(ui.ctx(), label)));
        let width = ui.available_width().max(60.0);
        body(ui, width);
    });
}
