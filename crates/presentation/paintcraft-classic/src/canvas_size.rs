//! Canvas dimension/default presentation; document and preference writes stay in the host.
use egui::{Color32, Vec2};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupSize {
    pub remember_last: bool,
    pub last: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSize {
    pub size: (u32, u32),
    /// Present when editing New-document defaults rather than the current image.
    pub startup: Option<StartupSize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasResponse {
    Pending,
    Apply,
    Cancel,
}

pub fn canvas_size_dialog(
    ctx: &egui::Context,
    draft: &mut CanvasSize,
    max_dimension: u32,
) -> CanvasResponse {
    let defaults = draft.startup.is_some();
    let title = if defaults {
        "Default canvas size"
    } else {
        "Canvas size"
    };
    let width = (ctx.content_rect().width() - 48.0).clamp(240.0, 520.0);
    let height = (ctx.content_rect().height() - 64.0).clamp(180.0, 520.0);
    let mut result = CanvasResponse::Pending;
    let modal = egui::Modal::new(egui::Id::unique(title)).show(ctx, |ui| {
        ui.set_width(width);
        ui.set_max_height(height);
        ui.spacing_mut().interact_size.y = 36.0;
        ui.spacing_mut().icon_width = 22.0;
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(15.0));
        }
        ui.heading(title);
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .id_salt("canvas-size-body")
            .max_height((height - 104.0).max(60.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::group(ui.style())
                    .corner_radius(6)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.strong("Dimensions");
                        ui.add_space(8.0);
                        for (label, value) in
                            [("Width", &mut draft.size.0), ("Height", &mut draft.size.1)]
                        {
                            ui.horizontal(|ui| {
                                let caption = ui.add_sized([72.0, 36.0], egui::Label::new(label));
                                let field_width = ui.available_width().clamp(80.0, 220.0);
                                ui.add_sized(
                                    [field_width, 36.0],
                                    egui::DragValue::new(value)
                                        .range(1..=max_dimension)
                                        .suffix(" px"),
                                )
                                .labelled_by(caption.id);
                            });
                        }
                    });
                ui.add_space(12.0);
                if let Some(startup) = &mut draft.startup {
                    ui.strong("When you create a new image");
                    ui.radio_value(
                        &mut startup.remember_last,
                        true,
                        "Remember last canvas size",
                    );
                    ui.radio_value(
                        &mut startup.remember_last,
                        false,
                        "Use configured default size",
                    );
                    if let Some((w, h)) = startup.last {
                        ui.weak(format!("Last canvas: {w} × {h} pixels"));
                    }
                    ui.add_space(8.0);
                    ui.label("Applies to new Classic documents. The current image stays as it is.");
                } else {
                    ui.label("Keep the top-left corner. Content outside the canvas is preserved.");
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
            let valid = (1..=max_dimension).contains(&draft.size.0)
                && (1..=max_dimension).contains(&draft.size.1);
            let apply = egui::Button::new(egui::RichText::new("Apply").color(ink))
                .fill(ui.visuals().selection.stroke.color)
                .min_size(Vec2::new(button_width, 36.0));
            if ui.add_enabled(valid, apply).clicked() {
                result = CanvasResponse::Apply;
            }
            if ui
                .add_sized([button_width, 36.0], egui::Button::new("Cancel"))
                .clicked()
            {
                result = CanvasResponse::Cancel;
            }
        });
    });
    if modal.should_close() {
        result = CanvasResponse::Cancel;
    }
    result
}
