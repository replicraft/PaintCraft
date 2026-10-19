use crate::Workspace;
use paintcraft_core::types::Rgba;
use paintcraft_tools::Tool;
use paintcraft_ui::chrome::{Action, Controls, color_edit, tool_button};

pub fn floating_controls(
    ctx: &egui::Context,
    state: &mut Controls,
    workspace: &mut Workspace,
    enabled: bool,
    state_id: egui::Id,
) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::Window::new("Tools")
        .id(egui::Id::unique("studio-tools"))
        .open(&mut workspace.tools)
        .enabled(enabled)
        .resizable(false)
        .auto_sized()
        .default_pos(egui::pos2(12.0, 150.0))
        .show(ctx, |ui| {
            ui.push_id(state_id, |ui| {
                egui::Grid::new("studio-tool-grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (i, tool) in [
                            Tool::Select,
                            Tool::Pencil,
                            Tool::Brush,
                            Tool::Eraser,
                            Tool::Fill,
                            Tool::Text,
                            Tool::Line,
                            Tool::Rect,
                            Tool::Ellipse,
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            if tool_button(ui, state, tool, false) {
                                actions.push(Action::SelectTool(tool));
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            });
        });
    egui::Window::new("Colors")
        .id(egui::Id::unique("studio-colors"))
        .open(&mut workspace.colors)
        .enabled(enabled)
        .resizable(false)
        .auto_sized()
        .default_pos(egui::pos2(12.0, 500.0))
        .show(ctx, |ui| {
            ui.push_id(state_id, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Primary");
                    color_edit(ui, "Primary color", &mut state.primary);
                    ui.label("Secondary");
                    color_edit(ui, "Secondary color", &mut state.secondary);
                });
                egui::Grid::new("studio-palette")
                    .num_columns(8)
                    .spacing(egui::vec2(2.0, 2.0))
                    .show(ui, |ui| {
                        for (i, rgb) in PALETTE.into_iter().enumerate() {
                            let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let label =
                                format!("Color #{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                            let r = ui.add(
                                egui::Button::new("")
                                    .fill(color)
                                    .min_size(egui::vec2(20.0, 20.0)),
                            );
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    ui.is_enabled(),
                                    &label,
                                )
                            });
                            if r.clicked() {
                                state.primary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                            }
                            if r.secondary_clicked() {
                                state.secondary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                            }
                            if i % 8 == 7 {
                                ui.end_row();
                            }
                        }
                    });
            });
        });
    actions
}

pub fn layers_window(ctx: &egui::Context, open: &mut bool, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Window::new("Layers")
        .id(egui::Id::unique("studio-layers"))
        .open(open)
        .default_width(240.0)
        .default_pos(egui::pos2(
            (ctx.viewport_rect().right() - 260.0).max(170.0),
            360.0,
        ))
        .show(ctx, contents);
}

const PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [127, 127, 127],
    [128, 0, 0],
    [255, 0, 0],
    [255, 128, 0],
    [255, 255, 0],
    [0, 128, 0],
    [0, 128, 255],
    [255, 255, 255],
    [192, 192, 192],
    [128, 0, 128],
    [255, 128, 192],
    [128, 64, 0],
    [128, 255, 128],
    [0, 255, 255],
    [0, 0, 128],
];
