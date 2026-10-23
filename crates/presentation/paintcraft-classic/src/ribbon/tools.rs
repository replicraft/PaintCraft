//! V11 tool controls; other skins retain their shared colorful toolbar.
use super::*;

pub(super) fn tool_button(ui: &mut egui::Ui, state: &mut Controls, tool: Tool) -> bool {
    let icon = match tool {
        Tool::Pencil => Icon::Pencil,
        Tool::Fill => Icon::Bucket,
        Tool::Text => Icon::Text,
        Tool::Eraser => Icon::Eraser,
        _ => unreachable!("only the four v11 Tools choices use this control"),
    };
    if button(ui, tool.name(), icon, state.tool == tool).clicked() {
        state.tool = tool;
        true
    } else {
        false
    }
}

pub(super) fn prototype(ui: &mut egui::Ui, label: &str, icon: Icon, help: &str) {
    ui.add_enabled_ui(false, |ui| button(ui, label, icon, false))
        .inner
        .on_disabled_hover_text(help);
}

fn button(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    let response = crate::control_surface::button(
        ui,
        egui::Button::new("")
            .min_size(Vec2::splat(40.0))
            .frame_when_inactive(selected)
            .frame(true),
        selected,
    );
    paintcraft_ui::icons::classic_tool_image(
        ui.ctx(),
        icon,
        24.0,
        ui.visuals().strong_text_color(),
    )
    .paint_at(
        ui,
        egui::Rect::from_center_size(response.rect.center(), Vec2::splat(24.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}
