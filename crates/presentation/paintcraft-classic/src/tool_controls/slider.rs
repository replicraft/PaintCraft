//! Native-style thumb decoration over egui's existing input/accessibility control.
use egui::{Color32, Response, Ui, Vec2, emath::Numeric};
use std::ops::RangeInclusive;

pub(super) fn show<Num: Numeric>(
    ui: &mut Ui,
    value: &mut Num,
    range: RangeInclusive<Num>,
    height: f32,
    clamping: egui::SliderClamping,
) -> Response {
    ui.scope(|ui| {
        let accent = ui.visuals().selection.stroke.color;
        let shell = Color32::from_gray(if ui.visuals().dark_mode { 76 } else { 224 });
        ui.visuals_mut().selection.bg_fill = accent;
        // egui uses inactive.bg_fill for the rail as well as the resting thumb.
        // Keep both neutral and add only the small accent center below.
        let widgets = &mut ui.visuals_mut().widgets;
        for state in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            state.bg_fill = shell;
            state.fg_stroke = egui::Stroke::NONE;
        }
        ui.spacing_mut().interact_size.y = 32.0;
        ui.spacing_mut().slider_width = height;
        ui.spacing_mut().slider_rail_height = 4.0;
        let min = range.start().to_f64();
        let max = range.end().to_f64();
        let response = ui.add_sized(
            Vec2::new(32.0, height),
            egui::Slider::new(value, range)
                .vertical()
                .show_value(false)
                .clamping(clamping)
                .handle_shape(egui::style::HandleShape::Circle)
                .trailing_fill(true),
        );
        // egui's circle handle radius is cross-axis extent / 2.5; the center
        // travels inside that inset. Read the updated value so pointer, keys and
        // accessibility requests all paint their new position in this frame.
        let inset = response.rect.width() / 2.5;
        let fraction = ((value.to_f64() - min) / (max - min)).clamp(0.0, 1.0) as f32;
        let y = egui::lerp(
            response.rect.bottom() - inset..=response.rect.top() + inset,
            fraction,
        );
        let radius = if response.is_pointer_button_down_on() {
            5.0
        } else if response.hovered() || response.has_focus() {
            7.0
        } else {
            6.0
        };
        ui.painter()
            .circle_filled(egui::pos2(response.rect.center().x, y), radius, accent);
        response
    })
    .inner
}
