//! Studio selection shape is a tool preference captured at pointer-down.
use paintcraft_tools::{SelectionMode, Tool};
use paintcraft_ui::{chrome::Controls, t};

pub(super) fn options(ui: &mut egui::Ui, state: &mut Controls) {
    if state.tool != Tool::Select {
        return;
    }
    egui::ComboBox::from_label(t!(ui.ctx(), "Selection shapes"))
        .selected_text(t!(
            ui.ctx(),
            match state.options.selection_mode {
                SelectionMode::Rectangle => "Rectangular selection",
                SelectionMode::FreeForm => "Free-form selection",
            }
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut state.options.selection_mode,
                SelectionMode::Rectangle,
                t!(ui.ctx(), "Rectangular selection"),
            );
            ui.selectable_value(
                &mut state.options.selection_mode,
                SelectionMode::FreeForm,
                t!(ui.ctx(), "Free-form selection"),
            );
        });
    let label = t!(ui.ctx(), "Transparent selection");
    let help = t!(
        ui.ctx(),
        "When moving a selection, skip pixels whose RGB matches Color 2; alpha is ignored."
    );
    ui.checkbox(&mut state.options.transparent_selection, label)
        .on_hover_text(help);
}
