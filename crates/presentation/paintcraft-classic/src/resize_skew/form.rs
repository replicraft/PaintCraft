//! Shared form contents for host-owned, genuinely scoped Retro modals.
use super::*;
/// Draw the staged controls in the caller's current style; no global theme change.
pub fn contents(ui: &mut Ui, draft: &mut Draft, maximum: u32) -> Response {
    let mut response = Response::Pending;
    ui.set_min_width(270.0);
    ui.strong(t!(ui.ctx(), "Resize and skew"));
    egui::ScrollArea::vertical()
        .max_height((ui.ctx().content_rect().height() - 160.0).max(64.0))
        .show(ui, |ui| {
            legacy::group(ui, "Resize", |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut draft.pixels, false, t!(ui.ctx(), "Percentage"));
                    ui.radio_value(&mut draft.pixels, true, t!(ui.ctx(), "Pixels"));
                });
                for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                    legacy::row(ui, label, |ui, width| {
                        ui.push_id(("retro-resize", axis, draft.pixels), |ui| {
                            size_field(ui, draft, axis, maximum, width, 24.0, label)
                        });
                    });
                }
                ui.checkbox(&mut draft.linked, t!(ui.ctx(), "Maintain aspect ratio"));
            });
            legacy::group(ui, "Skew", |ui| {
                for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                    legacy::row(ui, label, |ui, width| {
                        let r = ui.add(
                            egui::DragValue::new(&mut draft.skew[axis])
                                .range(-89..=89)
                                .suffix("°")
                                .min_size(Vec2::new(width, 24.0)),
                        );
                        numeric_info(
                            ui,
                            &r,
                            &format!("Skew {}", label.to_lowercase()),
                            f64::from(draft.skew[axis]),
                        );
                    });
                }
            });
        });
    ui.separator();
    ui.horizontal(|ui| {
        let p = draft.parameters(maximum);
        if ui
            .add_enabled(
                p.is_some(),
                egui::Button::new(t!(ui.ctx(), "OK")).min_size(Vec2::new(80.0, 24.0)),
            )
            .clicked()
        {
            response = Response::Apply(p.expect("enabled validated parameters"));
        }
        if ui
            .add_sized([80.0, 24.0], egui::Button::new(t!(ui.ctx(), "Cancel")))
            .clicked()
        {
            response = Response::Cancel;
        }
    });
    response
}
