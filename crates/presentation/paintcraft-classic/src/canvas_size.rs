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
    let width = (ctx.content_rect().width() - 56.0).clamp(240.0, 520.0);
    let height = (ctx.content_rect().height() - 72.0).clamp(180.0, 520.0);
    let mut result = CanvasResponse::Pending;
    let frame = egui::Frame::window(&ctx.style_of(ctx.theme()))
        .inner_margin(16)
        .corner_radius(12);
    let modal =
        egui::Modal::new(egui::Id::unique(title))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.set_max_height(height);
                ui.spacing_mut().interact_size.y = 40.0;
                ui.spacing_mut().icon_width = 22.0;
                for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    ui.style_mut()
                        .text_styles
                        .insert(style, egui::FontId::proportional(16.0));
                }
                ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
                ui.visuals_mut().weak_text_color =
                    Some(Color32::from_gray(if ui.visuals().dark_mode {
                        184
                    } else {
                        88
                    }));
                ui.label(egui::RichText::new(title).size(24.0).strong());
                ui.add_space(12.0);
                // A visible reserved scrollbar makes lower policy choices discoverable
                // on short displays without covering dimension fields.
                ui.spacing_mut().scroll.floating = false;
                egui::ScrollArea::vertical()
            .id_salt("canvas-size-body")
            .max_height((height - 124.0).max(60.0))
                .auto_shrink([false, true])
            .show(ui, |ui| {
                crate::form_controls::style(ui);
                egui::Frame::group(ui.style())
                    .corner_radius(6)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(0.0));
                        ui.strong("Dimensions");
                        ui.add_space(8.0);
                        for (label, value) in
                            [("Width", &mut draft.size.0), ("Height", &mut draft.size.1)]
                        {
                            ui.horizontal(|ui| {
                                let caption = ui.add_sized([72.0, 40.0], egui::Label::new(label));
                                let field_width = ui.available_width().clamp(80.0, 220.0);
                                ui.add_sized(
                                    [field_width, 40.0],
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
                        .min_size(Vec2::new(button_width, 40.0));
                    if ui.add_enabled(valid, apply).clicked() {
                        result = CanvasResponse::Apply;
                    }
                    if ui
                        .add_sized([button_width, 40.0], egui::Button::new("Cancel"))
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
