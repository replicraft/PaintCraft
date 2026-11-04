//! Studio selection shape is a tool preference captured at pointer-down.
use paintcraft_tools::{SelectionMode, Tool};
use paintcraft_ui::{chrome::Controls, t};

pub(super) fn options(ui: &mut egui::Ui, state: &mut Controls) {
    if state.tool != Tool::Select {
        return;
    }
    egui::ComboBox::from_label(t!(ui.ctx(), "Selection shapes"))
        .selected_text(t!(ui.ctx(), state.options.selection_mode.label()))
        .show_ui(ui, |ui| {
            for mode in SelectionMode::ALL {
                ui.selectable_value(
                    &mut state.options.selection_mode,
                    mode,
                    t!(ui.ctx(), mode.label()),
                );
            }
        });
    let label = t!(ui.ctx(), "Transparent selection");
    let help = t!(
        ui.ctx(),
        "When moving a selection, skip pixels whose RGB matches Color 2; alpha is ignored."
    );
    ui.checkbox(&mut state.options.transparent_selection, label)
        .on_hover_text(help);
}
