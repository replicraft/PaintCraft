use crate::Workspace;
use paintcraft_ui::actions::{self, Context};
use paintcraft_ui::chrome::{Action, Controls, options};
use paintcraft_ui::i18n::tr;
use paintcraft_ui::widgets::Icon;

pub fn toolbar(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button(tr(ui.ctx(), "File"), |ui| {
            for &action in actions::FILE {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button(tr(ui.ctx(), "Edit"), |ui| {
            for &action in actions::EDIT {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        crate::image_menu::show(ui, context, &mut actions);
        ui.menu_button(tr(ui.ctx(), "View"), |ui| {
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
        ui.menu_button(tr(ui.ctx(), "Layers"), |ui| {
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
        ui.menu_button(tr(ui.ctx(), "Window"), |ui| {
            ui.checkbox(&mut workspace.tools, tr(ui.ctx(), "Tools"));
            ui.checkbox(&mut workspace.colors, tr(ui.ctx(), "Colors"));
            ui.checkbox(&mut workspace.layers, tr(ui.ctx(), "Layers"));
            ui.checkbox(&mut workspace.history, tr(ui.ctx(), "History"));
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
        ui.label(tr(ui.ctx(), state.tool.name()));
    });
    ui.separator();
    ui.add_enabled_ui(context.document_open, |ui| {
        crate::shapes::picker(ui, state, &mut actions);
        options(ui, state, &mut actions);
        paintcraft_ui::chrome::picker_options(ui, state);
    });
    actions
}
