//! Zoom preset list follows the current value on opening, then scrolls freely.
use super::*;
use paintcraft_ui::i18n::tr;

const PRESETS: [u32; 14] = [
    100000, 90000, 80000, 70000, 60000, 50000, 40000, 30000, 20000, 10000, 7500, 5000, 2500, 1250,
];

pub(super) fn show(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>) {
    let state_id = ui.make_persistent_id("zoom-presets-was-open");
    let was_open = ui.data(|data| data.get_temp::<bool>(state_id).unwrap_or(false));
    let current = (context.zoom * 10000.0).round() as u32;
    let nearest = PRESETS
        .into_iter()
        .min_by_key(|value| value.abs_diff(current));
    ui.spacing_mut().button_padding = egui::vec2(6.0, 0.0);
    ui.spacing_mut().icon_width = 12.0;
    let menu = egui::ComboBox::from_id_salt("zoom-presets")
        .popup_style(egui::style::StyleModifier::new(|style| {
            // Popup areas use context styles, so set the cue on the popup itself.
            style.spacing.scroll.floating_allocated_width = 4.0;
            style.spacing.scroll.dormant_handle_opacity = 0.6;
        }))
        .selected_text("")
        .width(24.0)
        .icon(|ui, rect, visuals, _| {
            let center = rect.center();
            ui.painter().add(egui::Shape::line(
                vec![
                    center + egui::vec2(-3.0, -1.5),
                    center + egui::vec2(0.0, 1.5),
                    center + egui::vec2(3.0, -1.5),
                ],
                egui::Stroke::new(1.0, visuals.fg_stroke.color),
            ));
        })
        .height((ui.ctx().content_rect().height() - 96.0).clamp(80.0, 420.0))
        .show_ui(ui, |ui| {
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
            ] {
                state.corner_radius = egui::CornerRadius::same(5);
            }
            ui.set_min_width(100.0);
            ui.spacing_mut().button_padding.x = 12.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, egui::FontId::proportional(15.0));
            }
            for value in PRESETS {
                let selected = (context.zoom * 10000.0 - value as f32).abs() < 0.5;
                let response = ui.add(
                    egui::Button::new(())
                        .left_text(percentage_text(value as f32 / 10000.0))
                        .selected(selected)
                        .frame_when_inactive(selected)
                        .min_size(egui::vec2(100.0, 40.0)),
                );
                if !was_open && Some(value) == nearest {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if selected {
                    ui.painter().rect_stroke(
                        response.rect.shrink(0.5),
                        4,
                        ui.visuals().selection.stroke,
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    actions.push(Action::View(ViewAction::SetZoom(value)));
                    ui.close();
                }
            }
        });
    let accessible_label = tr(ui.ctx(), "Zoom presets");
    menu.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &accessible_label)
    });
    let open = egui::ComboBox::is_open(ui.ctx(), menu.response.id);
    ui.data_mut(|data| data.insert_temp(state_id, open));
}
