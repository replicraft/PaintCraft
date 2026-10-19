use crate::Workspace;
use paintcraft_ui::chrome::{Action, Controls, options};
use paintcraft_ui::widgets::{Icon, icon_button};

pub fn toolbar(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    document_open: bool,
) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            for (label, action) in [
                ("New", Action::New),
                ("Open…", Action::Open),
                ("Save", Action::Save),
                ("Save as…", Action::SaveAs),
                ("Export…", Action::Export),
            ] {
                if ui
                    .add_enabled(
                        document_open || matches!(action, Action::New | Action::Open),
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button("Edit", |ui| {
            for (label, action) in [
                ("Undo", Action::Undo),
                ("Redo", Action::Redo),
                ("Select all", Action::SelectAll),
                ("Deselect", Action::Deselect),
            ] {
                if ui
                    .add_enabled(
                        document_open || matches!(action, Action::New | Action::Open),
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button("Window", |ui| {
            ui.checkbox(&mut workspace.tools, "Tools");
            ui.checkbox(&mut workspace.colors, "Colors");
            ui.checkbox(&mut workspace.layers, "Layers");
            ui.checkbox(&mut workspace.history, "History");
        });
    });
    ui.horizontal(|ui| {
        for (label, icon, action) in [
            ("New", Icon::New, Action::New),
            ("Open", Icon::Open, Action::Open),
            ("Save", Icon::Save, Action::Save),
            ("Export", Icon::Export, Action::Export),
            ("Undo", Icon::Undo, Action::Undo),
            ("Redo", Icon::Redo, Action::Redo),
        ] {
            if ui
                .add_enabled_ui(
                    document_open || matches!(action, Action::New | Action::Open),
                    |ui| icon_button(ui, label, icon, false, false),
                )
                .inner
                .clicked()
            {
                actions.push(action);
            }
        }
        ui.separator();
        ui.label(state.tool.name());
    });
    ui.separator();
    ui.add_enabled_ui(document_open, |ui| options(ui, state, &mut actions));
    actions
}
