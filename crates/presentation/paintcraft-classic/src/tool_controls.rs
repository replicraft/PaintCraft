//! Palette-style floating tool preferences; no document mutations.
use egui::{Color32, Rect, Vec2, emath::Numeric};
use paintcraft_tools::{SelectionMode, Tool, ToolOptions};
use paintcraft_ui::t;
mod slider;
/// Stable across tools so changing brushes never shifts document pixels.
pub const CANVAS_CLEARANCE: f32 = 144.0;
pub fn supports(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Pencil
            | Tool::Brush
            | Tool::Eraser
            | Tool::Line
            | Tool::Rect
            | Tool::Ellipse
            | Tool::Preset(_)
            | Tool::Polygon
            | Tool::Curve
    )
}
/// Return occupied bounds to fence canvas input, including the first frame.
pub fn show(
    ui: &egui::Ui,
    id: egui::Id,
    viewport: Rect,
    tool: Tool,
    options: &mut ToolOptions,
    enabled: bool,
    sampled_brush: bool,
) -> Option<Rect> {
    let selection = tool == Tool::Select && options.selection_mode == SelectionMode::Brush;
    if (!supports(tool) && !selection) || viewport.width() < 60.0 || viewport.height() < 240.0 {
        return None;
    }
    // Palette's viewport-relative rail heights and 12 / 24 point placement.
    let height = ((viewport.height() - 48.0 - 24.0) / 3.0).max(60.0);
    let rail = (height - 62.0).max(14.0);
    let response = egui::Area::new(id.with("classic-tool-controls"))
        .order(egui::Order::Foreground)
        .movable(false)
        .default_size(Vec2::ZERO)
        .fixed_pos(viewport.min + Vec2::new(12.0, 24.0))
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            ui.set_clip_rect(viewport);
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_enabled_ui(enabled, |ui| {
                let label = if selection {
                    t!(ui.ctx(), "Brush size")
                } else {
                    t!(
                        ui.ctx(),
                        "{ $tool } size",
                        &[("tool", t!(ui.ctx(), tool.name()))]
                    )
                };
                if selection {
                    control(
                        ui,
                        0,
                        &label,
                        "Tool size",
                        "Size",
                        &mut options.selection.diameter,
                        1..=256,
                        256,
                        rail,
                        true,
                    );
                } else {
                    let size = if tool == Tool::Pencil {
                        &mut options.pencil_size
                    } else {
                        &mut options.size
                    };
                    control(
                        ui,
                        0,
                        &label,
                        "Tool size",
                        "Size",
                        size,
                        1..=256,
                        1024,
                        rail,
                        true,
                    );
                }
                ui.add_space(6.0);
                if selection {
                    control(
                        ui,
                        1,
                        "Brush opacity",
                        "Brush opacity",
                        "Opacity",
                        &mut options.selection.strength,
                        0..=100,
                        100,
                        rail,
                        true,
                    );
                    ui.add_space(6.0);
                    control(
                        ui,
                        2,
                        "Brush hardness",
                        "Brush hardness",
                        "Hardness",
                        &mut options.selection.hardness,
                        0..=100,
                        100,
                        rail,
                        true,
                    );
                } else {
                    control(
                        ui,
                        1,
                        "Brush opacity",
                        "Brush opacity",
                        "Opacity",
                        &mut options.brush_opacity,
                        0..=100,
                        100,
                        rail,
                        tool == Tool::Brush,
                    );
                    ui.add_space(6.0);
                    control(
                        ui,
                        2,
                        "Brush hardness",
                        "Brush hardness",
                        "Hardness",
                        &mut options.brush_hardness,
                        0..=100,
                        100,
                        rail,
                        tool == Tool::Brush
                            && options.media == Default::default()
                            && !sampled_brush,
                    );
                }
            });
        });
    Some(response.response.rect)
}
#[allow(clippy::too_many_arguments)]
fn control<N: Numeric>(
    ui: &mut egui::Ui,
    icon: usize,
    header_label: &str,
    slider_label: &str,
    field_label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    field_max: N,
    rail: f32,
    enabled: bool,
) {
    egui::Frame::new()
        .fill(Color32::from_rgb(46, 46, 45))
        .corner_radius(7)
        .inner_margin(4)
        .show(ui, |ui| {
            ui.set_width(32.0);
            ui.spacing_mut().interact_size.x = 28.0;
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                let label = t!(ui.ctx(), header_label);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Image, ui.is_enabled(), &label)
                });
                response.on_hover_text(label);
                header(ui, rect.shrink(5.0), icon);
                ui.add_enabled_ui(enabled, |ui| {
                    let response =
                        slider::show(ui, value, range.clone(), rail, egui::SliderClamping::Edits);
                    let label = t!(ui.ctx(), slider_label);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), &label)
                    });
                    response.on_hover_text(format!(
                        "{label}: {}{}",
                        value.to_f64(),
                        if icon == 0 { " px" } else { "%" }
                    ));
                    ui.visuals_mut().override_text_color = Some(Color32::from_gray(230));
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_gray(48);
                    let response = ui.add(
                        egui::DragValue::new(value)
                            .range(*range.start()..=field_max)
                            .speed(1)
                            .max_decimals(0),
                    );
                    let label = t!(ui.ctx(), field_label);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &label)
                    });
                    response.on_hover_text(format!(
                        "{}: type an exact value",
                        t!(ui.ctx(), header_label)
                    ));
                });
            });
        });
}
fn header(ui: &egui::Ui, r: Rect, icon: usize) {
    let at = |x, y| egui::pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    let p = ui.painter();
    let stroke = egui::Stroke::new(1.4, Color32::WHITE);
    match icon {
        0 => {
            p.rect_stroke(
                Rect::from_min_max(at(0.22, 0.08), at(0.78, 0.42)),
                0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for (a, b) in [
                ((0.22, 0.27), (0.78, 0.27)),
                ((0.3, 0.42), (0.42, 0.92)),
                ((0.7, 0.42), (0.58, 0.92)),
                ((0.42, 0.92), (0.58, 0.92)),
            ] {
                p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
            }
        }
        1 => {
            p.circle_stroke(r.center(), r.width() * 0.4, stroke);
            p.rect_filled(
                Rect::from_min_max(at(0.2, 0.5), at(0.8, 0.65)),
                0,
                Color32::WHITE,
            );
        }
        _ => {
            p.circle_stroke(r.center(), r.width() * 0.42, stroke);
            p.circle_filled(r.center(), r.width() * 0.18, Color32::WHITE);
        }
    }
}
