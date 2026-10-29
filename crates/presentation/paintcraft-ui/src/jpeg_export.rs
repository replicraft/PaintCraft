//! Shared JPEG alpha backdrop control for desktop and browser exports.
use crate::t;

pub fn controls(ui: &mut egui::Ui, rgb: &mut [u8; 3]) {
    ui.horizontal(|ui| {
        let label = ui.label(t!(ui.ctx(), "JPEG background color"));
        ui.color_edit_button_srgb(rgb).labelled_by(label.id);
    });
    ui.label(t!(
        ui.ctx(),
        "Transparent pixels use the selected background color."
    ));
}
