//! Compact historical properties. Values and publication remain host-owned.
use super::{CanvasResponse, CanvasSize, fields, properties_info};
use egui::{Color32, Vec2};
use paintcraft_ui::i18n::{tr, tr_format};

pub(super) fn show(
    ctx: &egui::Context,
    draft: &mut CanvasSize,
    maximum: u32,
    max_pixels: Option<u64>,
    name: Option<&str>,
) -> CanvasResponse {
    let title = if name.is_some() {
        "Image properties"
    } else {
        "Canvas size"
    };
    let width = (ctx.content_rect().width() - 40.0).clamp(240.0, 340.0);
    let height = (ctx.content_rect().height() - 110.0).clamp(48.0, 400.0);
    let mut result = CanvasResponse::Pending;
    let modal = egui::Modal::new(egui::Id::unique("historical-canvas-properties"))
        .frame(
            egui::Frame::window(&ctx.style_of(ctx.theme()))
                .corner_radius(0)
                .inner_margin(10)
                .fill(Color32::from_gray(if ctx.theme() == egui::Theme::Dark { 43 } else { 240 })),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            crate::form_controls::style(ui);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut().text_styles.insert(style, egui::FontId::proportional(12.0));
            }
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
                state.corner_radius = 0.into();
            }
            ui.horizontal(|ui| {
                ui.strong(tr(ui.ctx(), title));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = tr_format(ui.ctx(), "Close { $title }", &[("title", tr(ui.ctx(), title).to_lowercase())]);
                    let close = ui.add(egui::Button::new("×").min_size(Vec2::splat(22.0))).on_hover_text(&label);
                    close.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
                    if close.clicked() {
                        result = CanvasResponse::Cancel;
                    }
                });
            });
            ui.separator();
            ui.spacing_mut().scroll.floating = false;
            egui::ScrollArea::vertical().id_salt("historical-properties-body")
                .max_height(height).min_scrolled_height(48.0).auto_shrink([false, true])
                .show(ui, |ui| {
                    if let Some(name) = name {
                        properties_info(ui, name);
                        ui.add_space(6.0);
                        group(ui, "Dimensions", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for unit in ["Inches", "Centimeters"] {
                                    ui.add_enabled(false, egui::RadioButton::new(false, tr(ui.ctx(), unit)))
                                        .on_disabled_hover_text(tr(ui.ctx(), "Physical units require image resolution metadata, which is not available for this document."));
                                }
                                let _ = ui.radio(true, tr(ui.ctx(), "Pixels"));
                            });
                        });
                        group(ui, "Colors", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.add_enabled(false, egui::RadioButton::new(false, tr(ui.ctx(), "Black and white")))
                                    .on_disabled_hover_text(tr(ui.ctx(), "Monochrome conversion is not available yet."));
                                let _ = ui.radio(true, tr(ui.ctx(), "Color"));
                            });
                        });
                    }
                    fields::rows(ui, &mut draft.size, maximum, ["Width", "Height"], 24.0, 12.0);
                    if let Some(limit) = max_pixels {
                        let label = tr_format(ui.ctx(), "Image limit: { $maximum } pixels", &[("maximum", limit.to_string())]);
                        if u64::from(draft.size.0) * u64::from(draft.size.1) > limit {
                            ui.colored_label(ui.visuals().error_fg_color, label);
                        } else { ui.label(label); }
                    }
                    ui.label(tr(ui.ctx(), "Keep the top-left corner. Content outside the canvas is preserved."));
                });
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_sized([80.0, 24.0], egui::Button::new(tr(ui.ctx(), "Cancel"))).clicked() {
                    result = CanvasResponse::Cancel;
                }
                let valid = (1..=maximum).contains(&draft.size.0)
                    && (1..=maximum).contains(&draft.size.1)
                    && max_pixels.is_none_or(|limit| u64::from(draft.size.0) * u64::from(draft.size.1) <= limit);
                if ui.add_enabled(valid, egui::Button::new(tr(ui.ctx(), if name.is_some() { "OK" } else { "Apply" })).min_size(Vec2::new(80.0, 24.0))).clicked() {
                    result = CanvasResponse::Apply;
                }
            });
        });
    if modal.should_close() {
        CanvasResponse::Cancel
    } else {
        result
    }
}

fn group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .corner_radius(0)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 1.0).max(0.0));
            ui.label(tr(ui.ctx(), title));
            body(ui);
        });
}
