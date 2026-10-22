//! Classic status-bar zoom: percentage entry, presets, fit and slider.
use paintcraft_ui::actions::{Action, Context, ViewAction};
use paintcraft_ui::widgets::Icon;

#[derive(Clone, Debug)]
struct Draft {
    source: f32,
    dirty: bool,
    text: String,
}

fn percent(text: &str) -> Option<u32> {
    let value: f32 = text.trim().trim_end_matches('%').trim().parse().ok()?;
    (value.is_finite() && (12.5..=1000.0).contains(&value)).then(|| (value * 100.0).round() as u32)
}

pub fn zoom_controls(ui: &mut egui::Ui, context: Context, state_id: egui::Id) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.push_id(("classic-zoom", state_id), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(340.0, ui.spacing().interact_size.y.max(30.0)),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().slider_width = 100.0;
                if paintcraft_ui::actions::toolbar_button(
                    ui,
                    Action::View(ViewAction::FitWindow),
                    Icon::FitWindow,
                    context,
                ) {
                    actions.push(Action::View(ViewAction::FitWindow));
                }
                let id = ui.scope_id().with("percentage-draft");
                let mut draft = ui
                    .ctx()
                    .data_mut(|d| d.get_temp::<Draft>(id))
                    .unwrap_or_else(|| Draft {
                        source: context.zoom,
                        dirty: false,
                        text: format!("{}%", context.zoom * 100.0),
                    });
                if draft.source != context.zoom {
                    draft = Draft {
                        source: context.zoom,
                        dirty: false,
                        text: format!("{:.2}%", context.zoom * 100.0),
                    };
                }
                let response = ui.add_enabled(
                    !context.blocked,
                    egui::TextEdit::singleline(&mut draft.text).desired_width(60.0),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::TextInput,
                        ui.is_enabled(),
                        "Zoom percentage",
                    )
                });
                draft.dirty |= response.changed();
                let enter = response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (response.lost_focus() || enter) && ui.ctx().current_pass_index() == 0 {
                    if draft.dirty
                        && let Some(value) = percent(&draft.text)
                    {
                        actions.push(Action::View(ViewAction::SetZoom(value)));
                        response.surrender_focus();
                    }
                    draft.dirty = false;
                    draft.text = format!("{:.2}%", context.zoom * 100.0);
                }
                ui.ctx().data_mut(|d| d.insert_temp(id, draft));
                let menu = egui::ComboBox::from_id_salt("zoom-presets")
                    .selected_text("")
                    .width(12.0)
                    .height(460.0)
                    .show_ui(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.spacing_mut().button_padding.y = 2.0;
                        for value in [
                            100000, 90000, 80000, 70000, 60000, 50000, 40000, 30000, 20000, 10000,
                            7500, 5000, 2500, 1250,
                        ] {
                            if ui
                                .selectable_label(
                                    (context.zoom * 10000.0 - value as f32).abs() < 0.5,
                                    format!("{}%", value as f32 / 100.0),
                                )
                                .clicked()
                            {
                                actions.push(Action::View(ViewAction::SetZoom(value)));
                                ui.close();
                            }
                        }
                    });
                menu.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Zoom presets")
                });
                if paintcraft_ui::actions::toolbar_button(
                    ui,
                    Action::View(ViewAction::ZoomOut),
                    Icon::ZoomOut,
                    context,
                ) {
                    actions.push(Action::View(ViewAction::ZoomOut));
                }
                let mut zoom = context.zoom * 100.0;
                let response = ui.add_enabled(
                    !context.blocked,
                    egui::Slider::new(&mut zoom, 12.5..=1000.0)
                        .logarithmic(true)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Edits),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), "Canvas zoom")
                });
                if response.changed() {
                    actions.push(Action::View(ViewAction::SetZoom(
                        (zoom * 100.0).round() as u32
                    )));
                }
                if paintcraft_ui::actions::toolbar_button(
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
        assert_eq!(percent("73%"), Some(7300));
        assert_eq!(percent("12.5"), Some(1250));
        assert_eq!(percent(" 1000 % "), Some(100000));
        for text in ["", "-1", "NaN", "inf", "73oops", "1001", "0"] {
            assert_eq!(percent(text), None);
        }
    }
}
