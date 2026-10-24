//! Studio owns shape selection presentation and emits the normal tool action.
use paintcraft_tools::{ShapePreset, Tool};
use paintcraft_ui::{actions::Action, chrome::Controls, i18n::tr};

pub(super) fn picker(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    if !matches!(
        state.tool,
        Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Preset(_)
    ) {
        return;
    }
    egui::ComboBox::from_label(tr(ui.ctx(), "Shape"))
        .selected_text(tr(ui.ctx(), state.tool.name()))
        .height(320.0)
        .show_ui(ui, |ui| {
            for tool in [Tool::Line, Tool::Rect, Tool::Ellipse]
                .into_iter()
                .chain(ShapePreset::ALL.map(Tool::Preset))
            {
                if ui
                    .selectable_label(state.tool == tool, tr(ui.ctx(), tool.name()))
                    .clicked()
                {
                    state.tool = tool;
                    actions.push(Action::SelectTool(tool));
                }
            }
        });
}
