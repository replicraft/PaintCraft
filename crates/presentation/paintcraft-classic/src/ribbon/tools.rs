//! V11 tool controls; other skins retain their shared colorful toolbar.
use super::*;
use paintcraft_ui::i18n::tr;

pub(super) fn tool_button(ui: &mut egui::Ui, state: &mut Controls, tool: Tool) -> bool {
    let icon = match tool {
        Tool::Pencil => Icon::Pencil,
        Tool::Fill => Icon::Bucket,
        Tool::Text => Icon::Text,
        Tool::Eraser => Icon::Eraser,
        Tool::ColorPicker => Icon::Eyedropper,
        Tool::Magnifier => Icon::ZoomIn,
        _ => unreachable!("only the v11 Tools choices use this control"),
    };
    if button(ui, tool.name(), icon, state.tool == tool).clicked() {
        state.tool = tool;
        true
    } else {
        false
    }
}

fn button(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    let localized_label = tr(ui.ctx(), label);
    let label = localized_label.as_str();
    let response = crate::control_surface::button(
        ui,
        egui::Button::new("")
            .min_size(Vec2::splat(40.0))
            .frame_when_inactive(selected)
            .frame(true),
        selected,
    );
    paintcraft_ui::icons::classic_tool_image(ui.ctx(), icon, 24.0, ui.visuals().dark_mode)
        .paint_at(
            ui,
            egui::Rect::from_center_size(response.rect.center(), Vec2::splat(24.0)),
        );
    let accessible_label = tr(ui.ctx(), label);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled(),
            selected,
            &accessible_label,
        )
    });
    response.on_hover_text(tr(ui.ctx(), label))
}
