//! Point-size entry and presets for the text toolbar.
use super::*;

const FONT_SIZES: [u16; 16] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];

pub(super) fn font_size(ui: &mut Ui, points: &mut u16) {
    ui.allocate_ui_with_layout(
        Vec2::new(90.0, CONTROL_HEIGHT),
        egui::Layout::left_to_right(Align::Center).with_main_align(Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.visuals_mut().extreme_bg_color = ui.visuals().widgets.inactive.weak_bg_fill;
            corners(ui, true);
            let padding = ui.spacing().button_padding;
            ui.spacing_mut().button_padding.x = 14.0;
            ui.style_mut().drag_value_text_style = egui::TextStyle::Body;
            let size = size_entry(ui, points);
            size.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), "Font size")
            });
            size.on_hover_text(paintcraft_ui::i18n::tr(
                ui.ctx(),
                "Font size in points · type a value or choose a preset",
            ));
            ui.spacing_mut().button_padding = padding;
            corners(ui, false);
            let state_id = ui.make_persistent_id("font-size-was-open");
            let was_open = ui.data(|data| data.get_temp::<bool>(state_id).unwrap_or(false));
            let nearest = FONT_SIZES
                .into_iter()
                .min_by_key(|size| size.abs_diff(*points));
            let (response, _) = egui::containers::menu::MenuButton::from_button(
                Button::new(" ").min_size(Vec2::new(24.0, CONTROL_HEIGHT)),
            )
            .ui(ui, |ui| {
                popup::style(ui);
                ui.set_min_width(90.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical()
                    .id_salt("classic-font-size-presets")
                    .max_height((ui.ctx().content_rect().height() * 0.5 - 24.0).clamp(72.0, 280.0))
                    .show(ui, |ui| {
                        for size in FONT_SIZES {
                            let selected = *points == size;
                            let choice = popup::row(ui, &size.to_string(), selected);
                            if !was_open && Some(size) == nearest {
                                choice.scroll_to_me(Some(Align::Center));
                            }
                            choice.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    ui.is_enabled(),
                                    *points == size,
                                    format!("{size} pt"),
                                )
                            });
                            if choice.clicked() {
                                *points = size;
                                ui.close();
                            }
                        }
                    });
            });
            let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::menu(&response).get_id());
            ui.data_mut(|data| data.insert_temp(state_id, open));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Font size presets")
            });
            let center = response.rect.center();
            ui.painter().add(egui::Shape::line(
                vec![
                    center + Vec2::new(-3.0, -1.5),
                    center + Vec2::new(0.0, 1.5),
                    center + Vec2::new(3.0, -1.5),
                ],
                egui::Stroke::new(1.0, ui.visuals().text_color()),
            ));
            response.on_hover_text(paintcraft_ui::i18n::tr(ui.ctx(), "Font size presets"));
        },
    );
}

fn corners(ui: &mut Ui, left: bool) {
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

/// Keep typed sizes local until accepted. A discarded private DragValue buffer
/// must not be parsed back into the preference on a repeated layout pass.
fn size_entry(ui: &mut Ui, points: &mut u16) -> Response {
    let id = ui.next_auto_id();
    let cancel_id = id.with("font-size-cancel-frame");
    let frame = ui.ctx().cumulative_frame_nr();
    let focused = ui.memory(|memory| memory.has_focus(id));
    if focused && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        ui.data_mut(|data| data.insert_temp(cancel_id, (frame, *points)));
    }
    let cancelled = ui.data(|data| {
        data.get_temp::<(u64, u16)>(cancel_id)
            .filter(|(cancel_frame, _)| *cancel_frame == frame)
            .map(|(_, original)| original)
    });
    if let Some(original) = cancelled {
        *points = original;
        ui.memory_mut(|memory| memory.surrender_focus(id));
    }
    // add_sized centers its child layout; min_size retains leading text alignment
    // in both the resting button and the focused text editor.
    let response = ui.add(
        egui::DragValue::new(points)
            .min_size(Vec2::new(66.0, CONTROL_HEIGHT))
            .range(1..=512)
            .update_while_editing(false),
    );
    if let Some(original) = cancelled {
        *points = original;
        response.surrender_focus();
    }
    if response.has_focus() {
        // Preserve focus until this widget handles Escape, before the host can
        // treat that key as cancellation of the unpublished canvas text.
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                },
            )
        });
    }
    response
}
