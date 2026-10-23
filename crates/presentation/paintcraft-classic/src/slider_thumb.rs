//! Shared Classic thumb surfaces; egui retains input and accessibility behavior.
use egui::{Color32, Response, Ui};

pub(super) fn style(ui: &mut Ui) {
    let accent = ui.visuals().selection.stroke.color;
    let shell = Color32::from_gray(if ui.visuals().dark_mode { 76 } else { 224 });
    ui.visuals_mut().selection.bg_fill = accent;
    let widgets = &mut ui.visuals_mut().widgets;
    for state in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        state.bg_fill = shell;
        state.fg_stroke = egui::Stroke::NONE;
    }
    ui.spacing_mut().slider_rail_height = 4.0;
}

pub(super) fn paint(ui: &Ui, response: &Response, fraction: f32, vertical: bool) {
    // egui 0.37's circle centers travel within a cross-axis / 2.5 inset.
    // Tests compare the decoration against egui's actual painted shell.
    let rect = response.rect;
    let center = if vertical {
        let inset = rect.width() / 2.5;
        egui::pos2(
            rect.center().x,
            egui::lerp(rect.bottom() - inset..=rect.top() + inset, fraction),
        )
    } else {
        let inset = rect.height() / 2.5;
        egui::pos2(
            egui::lerp(rect.left() + inset..=rect.right() - inset, fraction),
            rect.center().y,
        )
    };
    let radius = if response.is_pointer_button_down_on() {
        5.0
    } else if response.hovered() || response.has_focus() {
        7.0
    } else {
        6.0
    };
    ui.painter()
        .circle_filled(center, radius, ui.visuals().selection.stroke.color);
}
