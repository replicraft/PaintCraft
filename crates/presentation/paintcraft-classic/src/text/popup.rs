//! Shared choice rows for Classic text dropdowns.
use super::*;

pub(super) fn style(ui: &mut Ui, metrics: Metrics) {
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::proportional(metrics.font_points));
    }
    ui.spacing_mut().button_padding.x = 12.0;
    ui.spacing_mut().scroll.floating_allocated_width = 4.0;
    ui.spacing_mut().scroll.dormant_handle_opacity = 0.6;
}

pub(super) fn row(ui: &mut Ui, label: &str, selected: bool, metrics: Metrics) -> Response {
    let response = ui.add_sized(
        [ui.available_width(), metrics.height],
        Button::new(())
            .left_text(label)
            .selected(selected)
            .frame_when_inactive(selected)
            .truncate(),
    );
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            metrics.radius.min(4),
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    response
}
