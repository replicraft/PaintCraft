use crate::Workspace;
use paintcraft_tools::Tool;
use paintcraft_ui::chrome::{Action, Controls, tool_button};

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
    let mut colors_open = workspace.colors;
    egui::Window::new("Colors")
        .id(egui::Id::unique("studio-colors"))
        .open(&mut colors_open)
        .enabled(enabled)
        .resizable(false)
        .default_width(310.0)
        .max_width(340.0)
        .default_pos(egui::pos2(12.0, 500.0))
        .show(ctx, |ui| {
            ui.push_id(state_id, |ui| {
                ui.set_width(310.0);
                crate::colors::controls(ui, state, workspace);
            });
        });
    workspace.colors = colors_open;
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
