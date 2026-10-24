//! Staged layer properties. The desktop validates ownership and publishes edits.
use paintcraft_ui::t;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerProperties {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertiesResponse {
    Pending,
    Apply,
    Cancel,
}

pub fn layer_properties_dialog(
    ctx: &egui::Context,
    values: &mut LayerProperties,
) -> PropertiesResponse {
    let mut response = PropertiesResponse::Pending;
    let modal = egui::Modal::new(egui::Id::unique("studio-layer-properties")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.heading(t!(ui.ctx(), "Layer Properties"));
        ui.horizontal(|ui| {
            let label = ui.label(t!(ui.ctx(), "Name"));
            ui.text_edit_singleline(&mut values.name)
                .labelled_by(label.id);
        });
        ui.checkbox(&mut values.visible, t!(ui.ctx(), "Visible"));
        ui.horizontal(|ui| {
            let label = ui.label(t!(ui.ctx(), "Layer opacity"));
            ui.add(egui::Slider::new(&mut values.opacity, 0..=255))
                .labelled_by(label.id);
        });
        let valid = values.name.chars().count() <= 1024;
        if !valid {
            ui.label(t!(
                ui.ctx(),
                "Layer names can contain up to 1024 characters.",
            ));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(valid, egui::Button::new(t!(ui.ctx(), "Apply")))
                .clicked()
            {
                response = PropertiesResponse::Apply;
            }
            if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                response = PropertiesResponse::Cancel;
            }
        });
    });
    if modal.should_close() {
        response = PropertiesResponse::Cancel;
    }
    response
}
