//! The five-column Classic gallery follows the supplied reference ordering.
use super::{Action, Controls};
use egui::Vec2;
use paintcraft_tools::Tool;
use paintcraft_ui::icons::Icon;

const SHAPES: [(Icon, &str, Option<Tool>); 23] = [
    (Icon::Line, "Line", Some(Tool::Line)),
    (Icon::Curve, "Curve", None),
    (Icon::Ellipse, "Ellipse", Some(Tool::Ellipse)),
    (Icon::Rectangle, "Rectangle", Some(Tool::Rect)),
    (Icon::RoundedRectangle, "Rounded rectangle", None),
    (Icon::Polygon, "Polygon", None),
    (Icon::Triangle, "Triangle", None),
    (Icon::RightTriangle, "Right triangle", None),
    (Icon::Diamond, "Diamond", None),
    (Icon::Pentagon, "Pentagon", None),
    (Icon::Hexagon, "Hexagon", None),
    (Icon::RightArrow, "Right arrow", None),
    (Icon::LeftArrow, "Left arrow", None),
    (Icon::UpArrow, "Up arrow", None),
    (Icon::DownArrow, "Down arrow", None),
    (Icon::FourPointStar, "Four-point star", None),
    (Icon::Star, "Five-point star", None),
    (Icon::SixPointStar, "Six-point star", None),
    (Icon::RoundedSpeechBubble, "Rectangular callout", None),
    (Icon::SpeechBubble, "Oval callout", None),
    (Icon::CloudCallout, "Cloud callout", None),
    (Icon::Heart, "Heart", None),
    (Icon::Lightning, "Lightning", None),
];

pub(super) fn gallery(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    egui::Frame::new()
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(2)
        .show(ui, |ui| {
            ui.spacing_mut().button_padding = Vec2::splat(2.0);
            egui::ScrollArea::vertical()
                .id_salt("classic-shapes-gallery")
                .max_height(78.0)
                .max_width(132.0)
                .show(ui, |ui| {
                    egui::Grid::new("windows11-shapes")
                        .min_row_height(24.0)
                        .min_col_width(24.0)
                        .spacing(Vec2::ZERO)
                        .show(ui, |ui| {
                            for (index, (icon, label, tool)) in SHAPES.into_iter().enumerate() {
                                let selected = tool.is_some_and(|tool| state.tool == tool);
                                let response = ui
                                    .add_enabled_ui(tool.is_some(), |ui| {
                                        button(ui, label, icon, selected)
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

fn button(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size = Vec2::splat(24.0);
            ui.add(
                egui::Button::new(paintcraft_ui::icons::image(ui.ctx(), icon, 19.0))
                    .selected(selected)
                    .min_size(Vec2::splat(24.0))
                    .frame_when_inactive(selected)
                    .frame(true),
            )
        })
        .inner;
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            4,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}
