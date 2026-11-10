//! Status facts come from the real document and current publication owner.
use crate::{Intent, surface::with_style};
use paintcraft_ui::{
    actions::{Action, ViewAction},
    t,
};
#[derive(Clone, Copy, Debug)]
pub struct Status<'a> {
    pub message: &'a str,
    pub recovery: Option<&'a str>,
    pub canvas: Option<(u32, u32)>,
    pub pointer: Option<(i64, i64)>,
    pub selection: Option<(i64, i64)>,
    pub zoom: f32,
    pub blocked: bool,
}
pub fn status(ui: &mut egui::Ui, status: Status<'_>) -> Vec<Intent> {
    let mut intents = Vec::new();
    with_style(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(status.message);
            if let Some(recovery) = status.recovery {
                ui.separator();
                ui.weak(recovery);
            }
            if let Some((x, y)) = status.pointer {
                ui.separator();
                ui.label(format!("{x}, {y} px"));
            }
            if let Some((w, h)) = status.selection {
                ui.separator();
                ui.label(format!("{w} × {h} px"));
            }
            if let Some((w, h)) = status.canvas {
                ui.separator();
                ui.label(format!("{w} × {h} px"));
            }
            if status.zoom.is_finite() && status.zoom > 0.0 {
                let response = ui.add_enabled(
                    status.canvas.is_some() && !status.blocked,
                    egui::Button::new(format!("{:.0}%", status.zoom * 100.0)),
                );
                if response
                    .on_hover_text(t!(ui.ctx(), "Actual size"))
                    .clicked()
                {
                    intents.push(Intent::Action(Action::View(ViewAction::ActualSize)));
                }
            }
        });
    });
    intents
}
