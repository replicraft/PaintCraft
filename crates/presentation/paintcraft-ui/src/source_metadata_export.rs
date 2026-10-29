//! A copy-only privacy choice; actual authority is captured from the frozen
//! export document and target by the host, never by this checkbox or loss label.
use crate::t;

/// Caller shows this only for an attachment-bearing document. The draft starts
/// false for each new export and must not be reused as a prepared-policy witness.
pub fn controls(ui: &mut egui::Ui, omit: &mut bool) {
    ui.checkbox(
        omit,
        t!(
            ui.ctx(),
            "Omit original source metadata from this exported copy"
        ),
    );
    ui.label(t!(ui.ctx(), "Any retained original source profile, resolution and orientation information will be omitted from the copy. The open document and PCND saves keep it."));
}
