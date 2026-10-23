//! Classic v11 tool preferences float beside the canvas; no document mutations.
use egui::{Rect, Vec2, Widget};
use paintcraft_tools::{Tool, ToolOptions};
use paintcraft_ui::icons::Icon;

/// Only clamp the canvas to this inset when centering would cross the controls.
/// Keep it stable across tools so brush/eraser changes never shift document pixels.
pub const CANVAS_CLEARANCE: f32 = 144.0;

pub fn supports(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Pencil | Tool::Brush | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse
    )
}

/// Return the occupied screen rectangle so the host can exclude it from paint
/// input even on the first frame, before egui's previous-frame hit map settles.
pub fn show(
    ui: &egui::Ui,
    id: egui::Id,
    viewport: Rect,
    tool: Tool,
    options: &mut ToolOptions,
    enabled: bool,
) -> Option<Rect> {
    if !supports(tool) || viewport.width() < 48.0 || viewport.height() < 120.0 {
        return None;
    }
    let height = (viewport.height() - 152.0).clamp(40.0, 360.0);
    let top = (viewport.height() * 0.1).clamp(8.0, 64.0);
    let width = if tool == Tool::Brush { 106.0 } else { 48.0 };
    let response = egui::Area::new(id.with("classic-tool-controls"))
        .order(egui::Order::Foreground)
        .movable(false)
        .default_width(width)
        .fixed_pos(viewport.min + Vec2::new(12.0, top))
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            ui.set_clip_rect(viewport);
            ui.add_enabled_ui(enabled, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    panel(ui, |ui| {
                        let label = format!("{} size", tool.name());
                        header(ui, Icon::StrokeSize, &label);
                        ui.add_space(6.0);
                        ui.spacing_mut().slider_width = height;
                        let size = if tool == Tool::Pencil {
                            &mut options.pencil_size
                        } else {
                            &mut options.size
                        };
                        let response = slider(
                            ui,
                            egui::Slider::new(size, 1..=64)
                                .vertical()
                                .show_value(false)
                                .clamping(egui::SliderClamping::Edits),
                            height,
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Slider,
                                ui.is_enabled(),
                                "Tool size",
                            )
                        });
                        response.on_hover_text(format!("Size: {size} px"));
                        let value = ui
                            .add(egui::DragValue::new(size).range(1..=1024))
                            .on_hover_text("Size in pixels");
                        value.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::SpinButton,
                                ui.is_enabled(),
                                "Size",
                            )
                        });
                    });
                    if tool == Tool::Brush {
                        panel(ui, |ui| {
                            header(ui, Icon::Opacity, "Brush opacity");
                            ui.add_space(6.0);
                            ui.spacing_mut().slider_width = height;
                            let response = slider(
                                ui,
                                egui::Slider::new(&mut options.brush_opacity, 0..=100)
                                    .vertical()
                                    .show_value(false),
                                height,
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Slider,
                                    ui.is_enabled(),
                                    "Brush opacity",
                                )
                            });
                            response.on_hover_text(format!("Opacity: {}%", options.brush_opacity));
                            let value = ui.add(
                                egui::DragValue::new(&mut options.brush_opacity)
                                    .range(0..=100)
                                    .suffix("%"),
                            );
                            value.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::SpinButton,
                                    ui.is_enabled(),
                                    "Opacity",
                                )
                            });
                        });
                    }
                });
            });
        });
    Some(response.response.rect)
}

fn panel(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().window_fill())
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(10)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(36.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), body);
        });
}

fn slider(ui: &mut egui::Ui, slider: egui::Slider<'_>, height: f32) -> egui::Response {
    ui.scope(|ui| {
        let accent = ui.visuals().selection.stroke.color;
        ui.visuals_mut().selection.bg_fill = accent;
        ui.visuals_mut().widgets.inactive.bg_fill = if ui.visuals().dark_mode {
            egui::Color32::from_gray(76)
        } else {
            egui::Color32::from_gray(224)
        };
        ui.visuals_mut().widgets.inactive.fg_stroke = egui::Stroke::new(1.5, accent);
        ui.spacing_mut().slider_rail_height = 4.0;
        ui.add_sized(
            Vec2::new(20.0, height),
            slider
                .handle_shape(egui::style::HandleShape::Circle)
                .trailing_fill(true),
        )
    })
    .inner
}

fn header(ui: &mut egui::Ui, icon: Icon, label: &str) {
    let response = paintcraft_ui::icons::image(ui.ctx(), icon, 22.0).ui(ui);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, ui.is_enabled(), label));
    response.on_hover_text(label);
}
