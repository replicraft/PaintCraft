//! Classic ribbon popups. Only choices backed by tool operations are enabled.
use super::{Action, Controls};
use egui::Vec2;
use paintcraft_tools::{ShapeStyle, Tool};
use paintcraft_ui::actions::Context;
use paintcraft_ui::icons::Icon;

pub(super) fn selection(
    ui: &mut egui::Ui,
    state: &mut Controls,
    context: Context,
    actions: &mut Vec<Action>,
) {
    dropdown(ui, "Selection options", None, |ui| {
        popup_style(ui, 240.0);
        let rectangle = ui.add(
            egui::Button::image_and_text(
                paintcraft_ui::icons::image(ui.ctx(), Icon::RectangleSelect, 18.0),
                "Rectangle",
            )
            .selected(state.tool == Tool::Select),
        );
        rectangle.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::Button,
                ui.is_enabled(),
                state.tool == Tool::Select,
                "Rectangular selection",
            )
        });
        if rectangle.clicked() {
            state.tool = Tool::Select;
            actions.push(Action::SelectTool(Tool::Select));
            ui.close();
        }
        let free_form = ui.add_enabled(
            false,
            egui::Button::image_and_text(
                paintcraft_ui::icons::image(ui.ctx(), Icon::LassoSelect, 18.0),
                "Free-form",
            ),
        );
        free_form.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, false, "Free-form selection")
        });
        ui.separator();
        for action in [Action::SelectAll, Action::Deselect] {
            if paintcraft_ui::actions::menu_item(ui, action, context) {
                actions.push(action);
                ui.close();
            }
        }
        ui.add_enabled(false, egui::Button::new("Invert selection"));
        ui.separator();
        ui.add_enabled(false, egui::Button::new("Transparent selection"));
        ui.separator();
        ui.add_enabled(
            false,
            egui::Button::image_and_text(
                paintcraft_ui::icons::image(ui.ctx(), Icon::DeleteLayer, 18.0),
                "Delete",
            )
            .shortcut_text("Delete"),
        );
    });
}

pub(super) fn brushes(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    dropdown(ui, "Brushes", None, |ui| {
        super::brush_gallery::gallery(ui, state, actions);
    });
}

pub(super) fn shape_paint(ui: &mut egui::Ui, state: &mut Controls) {
    ui.add_enabled_ui(matches!(state.tool, Tool::Rect | Tool::Ellipse), |ui| {
        dropdown(ui, "Shape outline", Some(Icon::Pencil), |ui| {
            popup_style(ui, 200.0);
            let outlined = state.options.style != ShapeStyle::Filled;
            for (solid, label) in [(false, "No outline"), (true, "Solid outline")] {
                if ui.selectable_label(outlined == solid, label).clicked() {
                    state.options.style = match (solid, state.options.style) {
                        (false, _) => ShapeStyle::Filled,
                        (true, ShapeStyle::Filled) => ShapeStyle::Both,
                        (true, style) => style,
                    };
                    ui.close();
                }
            }
        });
        dropdown(ui, "Shape fill", Some(Icon::Bucket), |ui| {
            popup_style(ui, 200.0);
            let filled = state.options.style != ShapeStyle::Outline;
            for (solid, label) in [(false, "No fill"), (true, "Solid fill")] {
                if ui.selectable_label(filled == solid, label).clicked() {
                    state.options.style = match (solid, state.options.style) {
                        (false, _) => ShapeStyle::Outline,
                        (true, ShapeStyle::Outline) => ShapeStyle::Both,
                        (true, style) => style,
                    };
                    ui.close();
                }
            }
        });
    });
}

fn dropdown(
    ui: &mut egui::Ui,
    label: &str,
    icon: Option<Icon>,
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let response = ui
        .push_id(label, |ui| {
            let button = if let Some(icon) = icon {
                egui::Button::image_and_text(
                    paintcraft_ui::icons::image(ui.ctx(), icon, 20.0),
                    "   ",
                )
                .min_size(Vec2::new(54.0, 32.0))
                .frame(false)
            } else {
                ui.spacing_mut().interact_size.y = 20.0;
                egui::Button::new(" ")
                    .min_size(Vec2::new(64.0, 20.0))
                    .frame(false)
            };
            egui::containers::menu::MenuButton::from_button(button)
                .config(
                    egui::containers::menu::MenuConfig::new()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
                .ui(ui, contents)
                .0
        })
        .inner;
    let center = if icon.is_none() {
        response.rect.center()
    } else {
        response.rect.right_center() - Vec2::new(9.0, 0.0)
    };
    let ink = if ui.is_enabled() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let stroke = egui::Stroke::new(1.0, ink);
    ui.painter().line_segment(
        [center + Vec2::new(-3.0, -1.5), center + Vec2::new(0.0, 1.5)],
        stroke,
    );
    ui.painter().line_segment(
        [center + Vec2::new(0.0, 1.5), center + Vec2::new(3.0, -1.5)],
        stroke,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    if ui.is_enabled() {
        response.on_hover_text(label)
    } else {
        response
            .on_disabled_hover_text("Select a rectangle or ellipse to change its outline and fill")
    }
}

fn popup_style(ui: &mut egui::Ui, width: f32) {
    ui.set_min_width(width);
    ui.spacing_mut().interact_size.y = 32.0;
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::proportional(15.0));
    }
}
