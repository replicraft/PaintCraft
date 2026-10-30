//! Shared copy-consent presentation. Hosts retain the frozen source and authority.
use crate::t;

pub const LABEL: &str = "Bake editable filters into this exported copy";
pub const EXPLANATION: &str = "The exported copy keeps the rendered appearance but does not retain editable filter settings, including disabled filters. Save a PaintCraft project to keep them.";
pub const BAKED_DESCRIPTION: &str = "Editable filters are baked into this exported copy.";

/// The owner starts unchecked for a fresh copy and revokes its prepared output
/// when this response changes. This widget never constructs export authority.
pub fn controls(ui: &mut egui::Ui, checked: &mut bool) -> egui::Response {
    ui.vertical(|ui| {
        let response = ui.checkbox(checked, t!(ui.ctx(), LABEL));
        ui.add(egui::Label::new(egui::RichText::new(t!(ui.ctx(), EXPLANATION)).small()).wrap());
        response
    })
    .inner
}
