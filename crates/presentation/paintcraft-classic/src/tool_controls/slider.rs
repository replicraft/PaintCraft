//! Native-style thumb decoration over egui's existing input/accessibility control.
use egui::{Response, Ui, Vec2, emath::Numeric};
use std::ops::RangeInclusive;

pub(super) fn show<Num: Numeric>(
    ui: &mut Ui,
    value: &mut Num,
    range: RangeInclusive<Num>,
    height: f32,
    clamping: egui::SliderClamping,
) -> Response {
    ui.scope(|ui| {
        let accent = egui::Color32::from_rgb(24, 205, 219);
        ui.visuals_mut().selection.bg_fill = accent;
        let widgets = &mut ui.visuals_mut().widgets;
        for state in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            state.bg_fill = egui::Color32::from_gray(135);
            state.fg_stroke = egui::Stroke::new(3.0, accent);
        }
        ui.spacing_mut().slider_rail_height = 3.0;
        ui.spacing_mut().interact_size.y = 16.0;
        ui.spacing_mut().slider_width = height;
        let min = range.start().to_f64();
        let max = range.end().to_f64();
        let response = ui.add_sized(
            Vec2::new(16.0, height),
            egui::Slider::new(value, range)
                .vertical()
                .show_value(false)
                .clamping(clamping)
                .handle_shape(egui::style::HandleShape::Circle)
                .trailing_fill(true),
        );
        let (mut response, tap) = crate::slider_input::finish(ui, response, true);
        if let Some(fraction) = tap {
            let mapped = min + (max - min) * f64::from(fraction);
            let next = Num::from_f64(if Num::INTEGRAL {
                mapped.round()
            } else {
                mapped
            });
            if *value != next {
                *value = next;
                crate::slider_input::changed_tap(ui, &mut response);
            }
        }
        response
    })
    .inner
}
