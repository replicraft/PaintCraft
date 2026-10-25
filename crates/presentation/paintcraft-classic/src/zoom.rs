//! Classic status-bar zoom: percentage entry, presets, fit and slider.
use paintcraft_ui::actions::{Action, Context, ViewAction};
use paintcraft_ui::t;
use paintcraft_ui::widgets::Icon;

mod percentage;
mod presets;

pub(super) fn width(ui: &egui::Ui) -> f32 {
    if ui.ctx().content_rect().width() >= 1200.0 {
        400.0
    } else {
        340.0
    }
}

#[derive(Clone, Debug)]
struct Draft {
    source: f32,
    dirty: bool,
    text: String,
}

pub(super) fn percent(text: &str) -> Option<u32> {
    let value: f32 = text.trim().trim_end_matches('%').trim().parse().ok()?;
    (value.is_finite() && (12.5..=1000.0).contains(&value)).then(|| (value * 100.0).round() as u32)
}

pub(super) fn percentage_text(zoom: f32) -> String {
    let text = format!("{:.2}", zoom * 100.0);
    format!("{}%", text.trim_end_matches('0').trim_end_matches('.'))
}

pub fn zoom_controls(ui: &mut egui::Ui, context: Context, state_id: egui::Id) -> Vec<Action> {
    let mut actions = Vec::new();
    let width = width(ui);
    ui.push_id(("classic-zoom", state_id), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width, ui.spacing().interact_size.y.max(30.0)),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().slider_width = width - 240.0;
                if paintcraft_ui::actions::classic_status_button(
                    ui,
                    Action::View(ViewAction::FitWindow),
                    Icon::FitWindow,
                    context,
                ) {
                    actions.push(Action::View(ViewAction::FitWindow));
                }
                percentage::show(ui, context, &mut actions);
                if paintcraft_ui::actions::classic_status_button(
                    ui,
                    Action::View(ViewAction::ZoomOut),
                    Icon::ZoomOut,
                    context,
                ) {
                    actions.push(Action::View(ViewAction::ZoomOut));
                }
                let mut zoom = context.zoom * 100.0;
                let response = ui
                    .add_enabled_ui(!context.blocked, |ui| {
                        crate::slider_thumb::style(ui);
                        ui.spacing_mut().interact_size.y = 30.0;
                        let response = ui.add(
                            egui::Slider::new(&mut zoom, 12.5..=1000.0)
                                .trailing_fill(true)
                                .logarithmic(true)
                                .show_value(false)
                                .clamping(egui::SliderClamping::Edits)
                                .handle_shape(egui::style::HandleShape::Circle),
                        );
                        let (mut response, tap) = crate::slider_input::finish(ui, response, false);
                        if let Some(fraction) = tap {
                            let next = (12.5 * (1000.0_f32 / 12.5).powf(fraction) * 100.0).round();
                            if (zoom * 100.0).round() != next {
                                zoom = next / 100.0;
                                crate::slider_input::changed_tap(ui, &mut response);
                            }
                        }
                        let fraction =
                            (zoom.clamp(12.5, 1000.0) / 12.5).ln() / (1000.0_f32 / 12.5).ln();
                        crate::slider_thumb::paint(ui, &response, fraction, false);
                        response
                    })
                    .inner;
                let accessible_label = t!(ui.ctx(), "Canvas zoom");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Slider,
                        ui.is_enabled(),
                        &accessible_label,
                    )
                });
                if response.changed() {
                    actions.push(Action::View(ViewAction::SetZoom(
                        (zoom * 100.0).round() as u32
                    )));
                }
                if paintcraft_ui::actions::classic_status_button(
                    ui,
                    Action::View(ViewAction::ZoomIn),
                    Icon::ZoomIn,
                    context,
                ) {
                    actions.push(Action::View(ViewAction::ZoomIn));
                }
            },
        );
    });
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percent_entry_rejects_partial_invalid_and_out_of_range_values() {
        assert_eq!(percentage_text(1.0479999), "104.8%");
        assert_eq!(percentage_text(1.0), "100%");
        assert_eq!(percent("73%"), Some(7300));
        assert_eq!(percent("12.5"), Some(1250));
        assert_eq!(percent(" 1000 % "), Some(100000));
        for text in ["", "-1", "NaN", "inf", "73oops", "1001", "0"] {
            assert_eq!(percent(text), None);
        }
    }
}
