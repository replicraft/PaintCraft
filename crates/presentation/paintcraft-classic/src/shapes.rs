//! The five-column Classic gallery follows the supplied reference ordering.
use super::{Action, Controls};
use egui::Vec2;
use paintcraft_tools::{ShapePreset, Tool};
use paintcraft_ui::icons::Icon;

const SHAPES: [(Icon, &str, Option<Tool>); 23] = [
    (Icon::Line, "Line", Some(Tool::Line)),
    (Icon::Curve, "Curve", Some(Tool::Curve)),
    (Icon::Ellipse, "Ellipse", Some(Tool::Ellipse)),
    (Icon::Rectangle, "Rectangle", Some(Tool::Rect)),
    (
        Icon::RoundedRectangle,
        "Rounded rectangle",
        Some(Tool::Preset(ShapePreset::RoundedRectangle)),
    ),
    (Icon::Polygon, "Polygon", Some(Tool::Polygon)),
    (
        Icon::Triangle,
        "Triangle",
        Some(Tool::Preset(ShapePreset::Triangle)),
    ),
    (
        Icon::RightTriangle,
        "Right triangle",
        Some(Tool::Preset(ShapePreset::RightTriangle)),
    ),
    (
        Icon::Diamond,
        "Diamond",
        Some(Tool::Preset(ShapePreset::Diamond)),
    ),
    (
        Icon::Pentagon,
        "Pentagon",
        Some(Tool::Preset(ShapePreset::Pentagon)),
    ),
    (
        Icon::Hexagon,
        "Hexagon",
        Some(Tool::Preset(ShapePreset::Hexagon)),
    ),
    (
        Icon::RightArrow,
        "Right arrow",
        Some(Tool::Preset(ShapePreset::RightArrow)),
    ),
    (
        Icon::LeftArrow,
        "Left arrow",
        Some(Tool::Preset(ShapePreset::LeftArrow)),
    ),
    (
        Icon::UpArrow,
        "Up arrow",
        Some(Tool::Preset(ShapePreset::UpArrow)),
    ),
    (
        Icon::DownArrow,
        "Down arrow",
        Some(Tool::Preset(ShapePreset::DownArrow)),
    ),
    (
        Icon::FourPointStar,
        "Four-point star",
        Some(Tool::Preset(ShapePreset::FourPointStar)),
    ),
    (
        Icon::Star,
        "Five-point star",
        Some(Tool::Preset(ShapePreset::FivePointStar)),
    ),
    (
        Icon::SixPointStar,
        "Six-point star",
        Some(Tool::Preset(ShapePreset::SixPointStar)),
    ),
    (
        Icon::RoundedSpeechBubble,
        "Rectangular callout",
        Some(Tool::Preset(ShapePreset::RectangularCallout)),
    ),
    (
        Icon::SpeechBubble,
        "Oval callout",
        Some(Tool::Preset(ShapePreset::OvalCallout)),
    ),
    (
        Icon::CloudCallout,
        "Cloud callout",
        Some(Tool::Preset(ShapePreset::CloudCallout)),
    ),
    (Icon::Heart, "Heart", Some(Tool::Preset(ShapePreset::Heart))),
    (
        Icon::Lightning,
        "Lightning",
        Some(Tool::Preset(ShapePreset::Lightning)),
    ),
];

pub(super) fn gallery(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    let spacious = super::ribbon::spacious(ui);
    let cell = if spacious { 28.0 } else { 24.0 };
    let glyph = if spacious { 22.0 } else { 19.0 };
    egui::Frame::new()
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(2)
        .show(ui, |ui| {
            ui.spacing_mut().button_padding = Vec2::splat(2.0);
            egui::ScrollArea::vertical()
                .id_salt("classic-shapes-gallery")
                .max_height(if spacious { 90.0 } else { 78.0 })
                .max_width(cell * 5.0 + 12.0)
                .show(ui, |ui| {
                    egui::Grid::new("windows11-shapes")
                        .min_row_height(cell)
                        .min_col_width(cell)
                        .spacing(Vec2::ZERO)
                        .show(ui, |ui| {
                            for (index, (icon, label, tool)) in SHAPES.into_iter().enumerate() {
                                let selected = tool.is_some_and(|tool| state.tool == tool);
                                let response = ui
                                    .add_enabled_ui(tool.is_some(), |ui| {
                                        button(ui, label, icon, selected, cell, glyph)
                                    })
                                    .inner;
                                if let Some(tool) = tool {
                                    if response.clicked() {
                                        state.tool = tool;
                                        actions.push(Action::SelectTool(tool));
                                    }
                                } else {
                                    response.on_disabled_hover_text(format!(
                                        "{label} is not available yet."
                                    ));
                                }
                                if index % 5 == 4 {
                                    ui.end_row();
                                }
                            }
                        });
                });
        });
}

fn button(
    ui: &mut egui::Ui,
    label: &str,
    icon: Icon,
    selected: bool,
    cell: f32,
    glyph: f32,
) -> egui::Response {
    let localized_label = paintcraft_ui::i18n::tr(ui.ctx(), label);
    let label = localized_label.as_str();
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size = Vec2::splat(cell);
            crate::control_surface::button(
                ui,
                egui::Button::new(paintcraft_ui::icons::classic_shape_image(
                    ui.ctx(),
                    icon,
                    glyph,
                    ui.visuals().strong_text_color(),
                ))
                .min_size(Vec2::splat(cell))
                .frame_when_inactive(selected)
                .frame(true),
                selected,
            )
        })
        .inner;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}
