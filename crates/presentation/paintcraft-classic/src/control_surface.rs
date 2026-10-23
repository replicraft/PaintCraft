//! Neutral toolbar selections retain the shared accent for outlines and text editing.
use egui::{Button, Color32, Response, Shape, Stroke, StrokeKind, Ui};

pub(super) fn button(ui: &mut Ui, button: Button<'_>, selected: bool) -> Response {
    if !selected {
        return ui.add(button.selected(false));
    }
    // Reserve a background before egui paints the button's text and images.
    // Its interaction/accessibility still belongs to the native Button.
    let background = ui.painter().add(Shape::Noop);
    let response = ui.add(
        button
            .selected(true)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    );
    let dark = ui.visuals().dark_mode;
    let gray = if response.is_pointer_button_down_on() {
        if dark { 43 } else { 219 }
    } else if response.hovered() || response.has_focus() {
        if dark { 61 } else { 229 }
    } else if dark {
        51
    } else {
        237
    };
    let stroke = if ui.is_enabled() {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    // One rounded shape supplies both the neutral surface and accent boundary.
    ui.painter().set(
        background,
        egui::epaint::RectShape::new(
            response.rect.shrink(0.5),
            5,
            Color32::from_gray(gray),
            stroke,
            StrokeKind::Inside,
        ),
    );
    response
}
