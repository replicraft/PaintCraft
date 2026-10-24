//! Windows 11 ribbon geometry. Unimplemented controls are disabled prototypes.
use super::{Action, Controls, Workspace};
use egui::Vec2;
use paintcraft_tools::Tool;
use paintcraft_ui::{actions::Context, widgets::Icon};

mod split_tool;
mod tools;
use tools::{prototype, tool_button};

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
                    super::image_menus::controls(ui);
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
                            prototype(ui, "Color picker", Icon::Eyedropper,
                                "Picking colors from the canvas is not available yet.");
                            prototype(ui, "Magnifier", Icon::ZoomIn,
                                "The magnifier tool is not available yet. Use the status bar or the View menu's Zoom controls to change magnification.");
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
                                ui.add_space(24.0);
                                super::stroke_size::show(ui, state, context);
                            });
                        }
                    });
                });
                ui.separator();
                group(ui, "Colors", |ui| {
                    super::palette::colors(ui, state, &workspace.custom_colors, context, actions)
                });
                ui.separator();
                group(ui, "Layers", |ui| {
                    let response = ui
                        .allocate_ui_with_layout(
                            Vec2::new(60.0, 60.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let response = super::control_surface::button(ui,
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
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        workspace.layers,
                                        "Layers",
                                    )
                                });
                                response.on_hover_text("Layers")
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
                ui.label(
                    egui::RichText::new(label)
                        .size(14.0)
                        .color(egui::Color32::from_gray(if ui.visuals().dark_mode {
                            208
                        } else {
                            68
                        })),
                );
            },
        );
    });
}
