use crate::Workspace;
use paintcraft_ui::actions::{self, Context};
use paintcraft_ui::chrome::{Action, Controls, options};
use paintcraft_ui::widgets::Icon;

pub fn toolbar(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button(paintcraft_ui::i18n::tr(ui.ctx(), "File"), |ui| {
            for &action in actions::FILE {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button(paintcraft_ui::i18n::tr(ui.ctx(), "Edit"), |ui| {
            for &action in actions::EDIT {
                if actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button(paintcraft_ui::i18n::tr(ui.ctx(), "View"), |ui| {
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
        ui.menu_button(paintcraft_ui::i18n::tr(ui.ctx(), "Layers"), |ui| {
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
        ui.menu_button(paintcraft_ui::i18n::tr(ui.ctx(), "Window"), |ui| {
            ui.checkbox(
                &mut workspace.tools,
                paintcraft_ui::i18n::tr(ui.ctx(), "Tools"),
            );
            ui.checkbox(
                &mut workspace.colors,
                paintcraft_ui::i18n::tr(ui.ctx(), "Colors"),
            );
            ui.checkbox(
                &mut workspace.layers,
                paintcraft_ui::i18n::tr(ui.ctx(), "Layers"),
            );
            ui.checkbox(
                &mut workspace.history,
                paintcraft_ui::i18n::tr(ui.ctx(), "History"),
            );
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
        ] {
            if paintcraft_ui::actions::toolbar_button(ui, action, icon, context) {
                actions.push(action);
            }
        }
        ui.separator();
        ui.label(state.tool.name());
    });
    ui.separator();
    ui.add_enabled_ui(context.document_open, |ui| {
        options(ui, state, &mut actions);
        paintcraft_ui::chrome::picker_options(ui, state);
    });
    actions
}
