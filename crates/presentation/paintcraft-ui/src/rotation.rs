//! Staged scalar angle form shared by real captured host workflows.
//! No raster preview, document mutation or execution/budget authority.
use crate::t;
use paintcraft_core::types::{
    Rect, Result,
    rotation::{Rotation, coefficients_from_degrees},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    source: Rect,
    degrees: f64,
    compiled: Option<((i32, i32), (u32, u32))>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Cancel,
    Apply((i32, i32)),
}
impl Draft {
    /// # Errors
    /// Refuses an invalid or out-of-policy captured source extent.
    pub fn new(source: Rect) -> Result<Self> {
        let g = Rotation::new(source, (1 << 30, 0))?;
        let bounds = g.bounds(source)?;
        Ok(Self {
            source,
            degrees: 0.0,
            compiled: Some((
                g.coefficients(),
                (bounds.width() as u32, bounds.height() as u32),
            )),
        })
    }
    pub fn degrees(&self) -> f64 {
        self.degrees
    }
    /// Compile once per actual scalar edit; Apply reuses the cached pair.
    /// Invalid input disables Apply rather than retaining a previous valid pair.
    pub fn set_degrees(&mut self, value: f64) {
        if value.to_bits() == self.degrees.to_bits() {
            return;
        }
        self.degrees = value;
        self.compiled = coefficients_from_degrees(value)
            .and_then(|pair| {
                let bounds = Rotation::new(self.source, pair)?.bounds(self.source)?;
                Ok((pair, (bounds.width() as u32, bounds.height() as u32)))
            })
            .ok();
    }
    pub fn coefficients(&self) -> Option<(i32, i32)> {
        self.compiled.map(|p| p.0)
    }
    pub fn output_size(&self) -> Option<(u32, u32)> {
        self.compiled.map(|p| p.1)
    }
}
/// Draw scalar staging controls; only the caller can act on Apply.
pub fn contents(ui: &mut egui::Ui, draft: &mut Draft) -> Response {
    ui.set_min_width(280.0);
    ui.heading(t!(ui.ctx(), "Rotate custom"));
    ui.label(t!(ui.ctx(), "Positive angles rotate clockwise."));
    let mut degrees = draft.degrees();
    let label = t!(ui.ctx(), "Angle");
    ui.horizontal(|ui| {
        ui.label(&label);
        let r = ui.add(
            egui::DragValue::new(&mut degrees)
                .range(-360.0..=360.0)
                .speed(0.1)
                .max_decimals(3)
                .suffix("°"),
        );
        r.widget_info(|| {
            let mut info =
                egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &label);
            info.value = Some(degrees);
            info
        });
        if r.changed() {
            draft.set_degrees(degrees);
        }
    });
    if let Some((w, h)) = draft.output_size() {
        ui.label(t!(
            ui.ctx(),
            "Output: { $width } × { $height } pixels",
            width = w,
            height = h
        ));
    } else {
        ui.label(t!(
            ui.ctx(),
            "Choose an angle whose output fits the canvas limits."
        ));
    }
    ui.label(t!(
        ui.ctx(),
        "Pixels change only when you apply this rotation."
    ));
    ui.separator();
    let mut response = Response::Pending;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                draft.coefficients().is_some(),
                egui::Button::new(t!(ui.ctx(), "Apply")),
            )
            .clicked()
            && let Some(pair) = draft.coefficients()
        {
            response = Response::Apply(pair);
        }
        if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
            response = Response::Cancel;
        }
    });
    response
}
/// A real modal; Escape/outside dismissal is Cancel and never Apply.
pub fn dialog(ctx: &egui::Context, draft: &mut Draft) -> Response {
    let mut response = Response::Pending;
    let modal = egui::Modal::new(egui::Id::unique("custom-rotation"))
        .show(ctx, |ui| response = contents(ui, draft));
    if modal.should_close() && response == Response::Pending {
        Response::Cancel
    } else {
        response
    }
}
#[cfg(test)]
mod tests;
