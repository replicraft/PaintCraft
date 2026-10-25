//! Scoped form surfaces; egui retains control input, values and accessibility.
use egui::{Color32, Stroke, Ui};

/// Apply inside a form's scoped UI so its outlines do not restyle application
/// chrome or modal actions. Both the control fill and its surrounding card stay
/// distinguishable from the boundary in light and dark themes.
pub fn style(ui: &mut Ui) {
    let dark = ui.visuals().dark_mode;
    let accent = ui.visuals().selection.stroke.color;
    let idle = crate::control_surface::fill(ui, false, false);
    let hovered = crate::control_surface::fill(ui, true, false);
    let pressed = crate::control_surface::fill(ui, false, true);
    let hover_border = Color32::from_gray(if dark { 160 } else { 96 });
    let widgets = &mut ui.visuals_mut().widgets;
    for (state, fill, border) in [
        (&mut widgets.inactive, idle, Color32::from_gray(128)),
        (&mut widgets.hovered, hovered, hover_border),
        (&mut widgets.active, pressed, accent),
        (&mut widgets.open, hovered, hover_border),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, border);
    }
}
