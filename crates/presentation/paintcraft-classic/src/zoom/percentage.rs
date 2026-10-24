//! Joined percentage field/preset presentation; draft acceptance remains staged.
use super::*;
use paintcraft_ui::t;
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
            let accessible_label = t!(ui.ctx(), "Zoom percentage");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), &accessible_label)
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
            super::presets::show(ui, context, actions);
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
