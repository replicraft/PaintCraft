//! Shared choice rows for Classic text dropdowns.
use super::*;

pub(super) fn style(ui: &mut Ui) {
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::proportional(16.0));
    }
    ui.spacing_mut().button_padding.x = 12.0;
    ui.spacing_mut().scroll.floating_allocated_width = 4.0;
    ui.spacing_mut().scroll.dormant_handle_opacity = 0.6;
}

pub(super) fn row(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let response = ui.add_sized(
        [ui.available_width(), CONTROL_HEIGHT],
        Button::new(())
            .left_text(label)
            .selected(selected)
            .frame_when_inactive(selected)
            .truncate(),
    );
    selected_outline(ui, &response, selected);
    response
}
