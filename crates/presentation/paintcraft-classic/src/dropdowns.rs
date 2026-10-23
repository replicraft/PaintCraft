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
        super::selection_menu::show(ui, state, context, actions);
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
            popup_style(ui, 220.0);
            let outlined = state.options.style != ShapeStyle::Filled;
            for (solid, label) in [(false, "No outline"), (true, "Solid outline")] {
                if shape_choice(ui, label, outlined == solid).clicked() {
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
            popup_style(ui, 220.0);
            let filled = state.options.style != ShapeStyle::Outline;
            for (solid, label) in [(false, "No fill"), (true, "Solid fill")] {
                if shape_choice(ui, label, filled == solid).clicked() {
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
                egui::Button::new((
                    paintcraft_ui::icons::classic_menu_image(
                        ui.ctx(),
                        icon,
                        22.0,
                        ui.visuals().strong_text_color(),
                    ),
                    "   ",
                ))
                .min_size(Vec2::new(54.0, 32.0))
                .frame_when_inactive(false)
                .frame(true)
            } else {
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().button_padding.y = 0.0;
                egui::Button::new(" ")
                    .min_size(Vec2::new(64.0, 20.0))
                    .frame_when_inactive(false)
                    .frame(true)
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
    open_state(ui, &response, label);
    if ui.is_enabled() {
        response.on_hover_text(label)
    } else {
        response
            .on_disabled_hover_text("Select a rectangle or ellipse to change its outline and fill")
    }
}

/// MenuButton does not mark its trigger selected while its popup is open.
/// Keep the open state visible after the pointer moves into the menu.
pub(super) fn open_state(ui: &egui::Ui, response: &egui::Response, label: &str) {
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::menu(response).get_id());
    if open && ui.is_enabled() {
        selected_outline(ui, response.rect);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), open, label)
    });
}

fn shape_choice(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let response = ui.add(
        egui::Button::new(label)
            .selected(selected)
            .frame_when_inactive(selected)
            .frame(true)
            .min_size(Vec2::new(220.0, 40.0)),
    );
    if selected {
        selected_outline(ui, response.rect);
    }
    response
}

fn selected_outline(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect.shrink(0.5),
        4,
        ui.visuals().selection.stroke,
        egui::StrokeKind::Inside,
    );
}

fn popup_style(ui: &mut egui::Ui, width: f32) {
    ui.set_width(width);
    ui.spacing_mut().interact_size.y = 40.0;
    ui.spacing_mut().item_spacing.y = 0.0;
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::proportional(16.0));
    }
}
