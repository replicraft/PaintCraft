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
    dropdown(ui, "Selection options", "", |ui| {
        ui.set_min_width(240.0);
        ui.spacing_mut().interact_size.y = 30.0;
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
    dropdown(ui, "Brushes", "", |ui| {
        super::brush_gallery::gallery(ui, state, actions);
    });
}

pub(super) fn shape_paint(ui: &mut egui::Ui, state: &mut Controls) {
    ui.add_enabled_ui(matches!(state.tool, Tool::Rect | Tool::Ellipse), |ui| {
        dropdown(ui, "Shape outline", "Outline", |ui| {
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
        dropdown(ui, "Shape fill", "Fill", |ui| {
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
    text: &str,
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let response = ui
        .push_id(label, |ui| {
            let button = egui::Button::new(format!("{text}    ")).min_size(Vec2::new(64.0, 18.0));
            egui::containers::menu::MenuButton::from_button(button)
                .config(
                    egui::containers::menu::MenuConfig::new()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
                .ui(ui, contents)
                .0
        })
        .inner;
    let center = if text.is_empty() {
        response.rect.center()
    } else {
        response.rect.right_center() - Vec2::new(9.0, 0.0)
    };
    let stroke = egui::Stroke::new(1.0, ui.visuals().text_color());
    ui.painter().line_segment(
        [center + Vec2::new(-3.0, -1.5), center + Vec2::new(0.0, 1.5)],
        stroke,
    );
    ui.painter().line_segment(
        [center + Vec2::new(0.0, 1.5), center + Vec2::new(3.0, -1.5)],
        stroke,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response
}
