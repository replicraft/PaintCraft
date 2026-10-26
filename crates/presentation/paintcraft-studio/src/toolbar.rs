use crate::Workspace;
use paintcraft_ui::actions::{self, Context};
use paintcraft_ui::chrome::{Action, Controls, options};
use paintcraft_ui::t;
use paintcraft_ui::widgets::Icon;

pub fn toolbar(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button(t!(ui.ctx(), "File"), |ui| {
            for &action in actions::STUDIO_FILE {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button(t!(ui.ctx(), "Edit"), |ui| {
            for &action in actions::EDIT {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        crate::image_menu::show(ui, context, &mut actions);
        ui.menu_button(t!(ui.ctx(), "View"), |ui| {
            for (index, &action) in actions::VIEW.iter().enumerate() {
                if index == 5 {
                    ui.separator();
                }
                if actions::menu_item(ui, Action::View(action), context) {
                    actions.push(Action::View(action));
                    ui.close();
                }
            }
        });
        ui.menu_button(t!(ui.ctx(), "Layers"), |ui| {
            for (index, &action) in actions::LAYERS.iter().enumerate() {
                if matches!(index, 3 | 4 | 8 | 12) {
                    ui.separator();
                }
                if actions::menu_item(ui, Action::Layer(action), context) {
                    actions.push(Action::Layer(action));
                    ui.close();
                }
            }
        });
        ui.menu_button(t!(ui.ctx(), "Window"), |ui| {
            ui.checkbox(&mut workspace.tools, t!(ui.ctx(), "Tools"));
            ui.checkbox(&mut workspace.colors, t!(ui.ctx(), "Colors"));
            ui.checkbox(&mut workspace.layers, t!(ui.ctx(), "Layers"));
            ui.checkbox(&mut workspace.history, t!(ui.ctx(), "History"));
        });
    });
    ui.horizontal(|ui| {
        for (icon, action) in [
            (Icon::New, Action::New),
            (Icon::Open, Action::Open),
            (Icon::Save, Action::Save),
            (Icon::Export, Action::Export),
            (Icon::Undo, Action::Undo),
            (Icon::Redo, Action::Redo),
            (Icon::ClearCanvas, Action::ClearCanvas),
        ] {
            if paintcraft_ui::actions::toolbar_button(ui, action, icon, context) {
                actions.push(action);
            }
        }
        ui.separator();
        if let Some(name) = &workspace.resource_shape {
            ui.label(name);
        } else {
            ui.label(t!(
                ui.ctx(),
                Action::SelectTool(state.tool)
                    .descriptor_for(context.frontend)
                    .label,
            ));
        }
    });
    ui.separator();
    ui.add_enabled_ui(context.document_open, |ui| {
        if workspace.resource_shape.is_none() {
            crate::shapes::picker(ui, state, &mut actions);
        }
        crate::selection::options(ui, state);
        if state.tool == paintcraft_tools::Tool::Rect
            && let Some(name) = &workspace.resource_shape
        {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!(
                    ui.ctx(),
                    "Selected custom shape: { $name }",
                    name = name
                ));
                ui.label(t!(ui.ctx(), "Drag to size; hold Shift for a square box."));
                if ui.button(t!(ui.ctx(), "Shape resources…")).clicked() {
                    actions.push(Action::ShapeResources);
                }
            });
        } else if state.tool == paintcraft_tools::Tool::Brush
            && let Some((name, (width, height))) = &workspace.sampled_brush
        {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!(ui.ctx(), "Selected brush: { $name }", name = name));
                ui.label(t!(
                    ui.ctx(),
                    "Tip size: { $width } × { $height } px",
                    width = *width,
                    height = *height
                ));
                ui.add(
                    egui::Slider::new(&mut state.options.brush_opacity, 0..=100)
                        .text(t!(ui.ctx(), "Brush opacity"))
                        .suffix("%"),
                );
                if ui.button(t!(ui.ctx(), "Brush resources…")).clicked() {
                    actions.push(Action::BrushResources);
                }
            });
        } else {
            options(ui, state, &mut actions);
        }
        paintcraft_ui::chrome::picker_options(ui, state);
    });
    actions
}
