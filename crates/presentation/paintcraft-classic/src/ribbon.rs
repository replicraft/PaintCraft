//! Windows 11 ribbon geometry. Unimplemented controls are disabled prototypes.
use super::{Action, Controls, Workspace};
use egui::Vec2;
use paintcraft_tools::Tool;
use paintcraft_ui::t;
use paintcraft_ui::{actions::Context, widgets::Icon};

mod split_tool;
mod tools;
use tools::tool_button;

pub(super) fn spacious(ui: &egui::Ui) -> bool {
    ui.ctx().content_rect().width() >= 1200.0
}

pub(super) fn roomy_spacing(ui: &egui::Ui) -> bool {
    ui.ctx().content_rect().width() >= 1280.0
}

pub(super) fn ribbon(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.separator();
    egui::ScrollArea::horizontal()
        .id_salt("windows11-ribbon")
        .max_height(if spacious(ui) { 108.0 } else { 96.0 })
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Keep all groups reachable at medium widths before scrolling
                // is necessary; preserve the roomier desktop spacing otherwise.
                if !roomy_spacing(ui) {
                    ui.spacing_mut().item_spacing.x = 4.0;
                }
                group(ui, "Selection", |ui| {
                    split_tool::show(ui, state, Tool::Select, context, actions);
                });
                ui.separator();
                group(ui, "Image", |ui| {
                    super::image_menus::controls(ui, context, actions);
                });
                ui.separator();
                group(ui, "Tools", |ui| {
                    egui::Grid::new("windows11-tools")
                        .spacing(Vec2::splat(1.0))
                        .show(ui, |ui| {
                            for tool in [Tool::Pencil, Tool::Fill, Tool::Text] {
                                if tool_button(ui, state, tool) {
                                    actions.push(Action::SelectTool(tool));
                                }
                            }
                            ui.end_row();
                            if tool_button(ui, state, Tool::Eraser) {
                                actions.push(Action::SelectTool(Tool::Eraser));
                            }
                            if tool_button(ui, state, Tool::ColorPicker) {
                                actions.push(Action::SelectTool(Tool::ColorPicker));
                            }
                            if tool_button(ui, state, Tool::Magnifier) {
                                actions.push(Action::SelectTool(Tool::Magnifier));
                            }
                        });
                });
                ui.separator();
                group(ui, "Brushes", |ui| {
                    split_tool::show(ui, state, Tool::Brush, context, actions);
                });
                ui.separator();
                group(ui, "Shapes", |ui| {
                    ui.horizontal_top(|ui| {
                        super::shapes::gallery(ui, state, actions);
                        ui.vertical(|ui| {
                            if !spacious(ui) {
                                // Three compact targets fit the shared caption baseline.
                                ui.spacing_mut().item_spacing.y = 3.0;
                            }
                            super::dropdowns::shape_paint(ui, state);
                            if !spacious(ui) {
                                super::stroke_size::show(ui, state, context);
                            }
                        });
                        if spacious(ui) {
                            ui.vertical(|ui| {
                                // Center the 40-point button against the 84-point gallery.
                                ui.add_space(22.0);
                                super::stroke_size::show(ui, state, context);
                            });
                        }
                    });
                });
                ui.separator();
                let mut colors_center = ui.cursor().min.y;
                group(ui, "Colors", |ui| {
                    colors_center = super::palette::colors(
                        ui,
                        state,
                        &workspace.custom_colors,
                        context,
                        actions,
                    );
                });
                ui.separator();
                group(ui, "Layers", |ui| {
                    // Follow the measured palette row at either ribbon density.
                    ui.add_space((colors_center - ui.cursor().min.y - 30.0).max(0.0));
                    let response = ui
                        .allocate_ui_with_layout(
                            Vec2::new(60.0, 60.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let response = super::control_surface::button(
                                    ui,
                                    egui::Button::image(paintcraft_ui::icons::classic_image(
                                        ui.ctx(),
                                        Icon::Layers,
                                        44.0,
                                        ui.visuals().dark_mode,
                                    ))
                                    .min_size(Vec2::new(60.0, 60.0))
                                    .frame_when_inactive(workspace.layers)
                                    .frame(true),
                                    workspace.layers,
                                );
                                let accessible_label = t!(ui.ctx(), "Layers");
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        workspace.layers,
                                        &accessible_label,
                                    )
                                });
                                response.on_hover_text(t!(ui.ctx(), "Layers"))
                            },
                        )
                        .inner;
                    if response.clicked() && ui.ctx().current_pass_index() == 0 {
                        workspace.layers = !workspace.layers;
                    }
                });
                let height = if spacious(ui) { 108.0 } else { 96.0 };
                // Follow the caption row at the far edge on desktop. In small
                // windows this stays in the ribbon's horizontal scroller.
                ui.add_space((ui.available_width() - 28.0).max(0.0));
                ui.allocate_ui_with_layout(
                    Vec2::new(28.0, height + 18.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(height - 6.0);
                        super::toolbar_visibility::menu(ui, context, actions);
                    },
                );
            });
        });
    ui.separator();
    // Selection actions live in the Selection popup and Edit menu; they do not
    // need a second toolbar row that shifts the canvas when choosing Select.
    if state.tool == Tool::Fill {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 40.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_height(40.0);
                paintcraft_ui::chrome::options_with_shape_style(ui, state, actions, false)
            },
        );
    }
}

fn group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let content_height = if spacious(ui) { 108.0 } else { 96.0 };
    ui.vertical(|ui| {
        let top = ui.cursor().min.y;
        contents(ui);
        ui.add_space((top + content_height - ui.cursor().min.y).max(0.0));
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 18.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(egui::RichText::new(t!(ui.ctx(), label)).size(14.0).color(
                    egui::Color32::from_gray(if ui.visuals().dark_mode { 208 } else { 68 }),
                ));
            },
        );
    });
}
