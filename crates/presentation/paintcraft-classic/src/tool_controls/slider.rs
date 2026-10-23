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
        crate::slider_thumb::style(ui);
        ui.spacing_mut().interact_size.y = 32.0;
        ui.spacing_mut().slider_width = height;
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
        let fraction = ((value.to_f64() - min) / (max - min)).clamp(0.0, 1.0) as f32;
        crate::slider_thumb::paint(ui, &response, fraction, true);
        response
    })
    .inner
}
