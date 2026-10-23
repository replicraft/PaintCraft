//! Joined percentage field/preset presentation; draft acceptance remains staged.
use super::*;

pub(super) fn show(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>) {
    ui.allocate_ui_with_layout(
        egui::vec2(114.0, 30.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().interact_size.y = 30.0;
            ui.visuals_mut().extreme_bg_color = ui.visuals().widgets.inactive.weak_bg_fill;
            corners(ui, true);
            let id = ui.scope_id().with("percentage-draft");
            let mut draft = ui
                .ctx()
                .data_mut(|d| d.get_temp::<Draft>(id))
                .unwrap_or_else(|| Draft {
                    source: context.zoom,
                    dirty: false,
                    text: percentage_text(context.zoom),
                });
            if draft.source != context.zoom {
                draft = Draft {
                    source: context.zoom,
                    dirty: false,
                    text: percentage_text(context.zoom),
                };
            }
            let field_id = ui.scope_id().with("percentage-entry");
            let focused = ui.memory(|memory| memory.has_focus(field_id));
            let cancel = focused
                && ui.ctx().current_pass_index() == 0
                && ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if cancel {
                draft.dirty = false;
                draft.text = percentage_text(context.zoom);
            }
            let response = ui.add_enabled(
                !context.blocked,
                egui::TextEdit::singleline(&mut draft.text)
                    .id(field_id)
                    .event_filter(egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    })
                    .font(egui::FontId::proportional(15.0))
                    .margin(egui::vec2(8.0, 4.0))
                    .desired_width(90.0),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), "Zoom percentage")
            });
            draft.dirty |= response.changed();
            let enter = response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if cancel {
                response.surrender_focus();
            } else if (response.lost_focus() || enter) && ui.ctx().current_pass_index() == 0 {
                if draft.dirty
                    && let Some(value) = percent(&draft.text)
                {
                    actions.push(Action::View(ViewAction::SetZoom(value)));
                    response.surrender_focus();
                }
                draft.dirty = false;
                draft.text = percentage_text(context.zoom);
            }
            ui.ctx().data_mut(|d| d.insert_temp(id, draft));
            corners(ui, false);
            ui.spacing_mut().button_padding = egui::vec2(6.0, 0.0);
            ui.spacing_mut().icon_width = 12.0;
            let menu = egui::ComboBox::from_id_salt("zoom-presets")
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
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                        ui.style_mut()
                            .text_styles
                            .insert(style, egui::FontId::proportional(15.0));
                    }
                    for value in [
                        100000, 90000, 80000, 70000, 60000, 50000, 40000, 30000, 20000, 10000,
                        7500, 5000, 2500, 1250,
                    ] {
                        let selected = (context.zoom * 10000.0 - value as f32).abs() < 0.5;
                        let response = ui.add(
                            egui::Button::new(percentage_text(value as f32 / 10000.0))
                                .selected(selected)
                                .frame_when_inactive(selected)
                                .min_size(egui::vec2(100.0, 40.0)),
                        );
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
            menu.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Zoom presets")
            });
        },
    );
}

fn corners(ui: &mut egui::Ui, left: bool) {
    let radius = if left {
        egui::CornerRadius {
            nw: 5,
            sw: 5,
            ne: 0,
            se: 0,
        }
    } else {
        egui::CornerRadius {
            nw: 0,
            sw: 0,
            ne: 5,
            se: 5,
        }
    };
    let widgets = &mut ui.visuals_mut().widgets;
    for state in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = radius;
    }
}
