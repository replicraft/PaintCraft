//! Historical 22-pixel status strip and compact Windows slider presentation.
use super::*;
use paintcraft_ui::actions::ViewAction;
use paintcraft_ui::t;

#[derive(Clone)]
struct Draft {
    zoom: f32,
    text: String,
}

pub(super) fn contents(
    ui: &mut egui::Ui,
    variant: crate::ClassicVariant,
    status: Status<'_>,
    context: Context,
    id: egui::Id,
) -> Vec<Action> {
    ui.spacing_mut().interact_size.y = 20.0;
    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
    let mut actions = Vec::new();
    let width = ui.available_width();
    let narrow = width < 860.0;
    let metrics = if narrow { width * 0.65 } else { 470.0 };
    ui.allocate_ui_with_layout(
        Vec2::new(width, 22.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            if status.canvas.is_none() {
                super::message(ui, &status, width, 22.0);
                return;
            }
            super::metrics(ui, &status, metrics, 22.0, context.selection_active);
            super::message(
                ui,
                &status,
                (width - metrics - if narrow { 8.0 } else { 252.0 }).max(0.0),
                22.0,
            );
            if !narrow {
                zoom(ui, variant, context, id, &mut actions);
            }
        },
    );
    if narrow && status.canvas.is_some() {
        ui.allocate_ui_with_layout(
            Vec2::new(width, 22.0),
            Layout::right_to_left(Align::Center),
            |ui| zoom(ui, variant, context, id, &mut actions),
        );
    }
    actions
}

fn zoom(
    ui: &mut egui::Ui,
    variant: crate::ClassicVariant,
    context: Context,
    id: egui::Id,
    actions: &mut Vec<Action>,
) {
    ui.push_id(("legacy-zoom", id), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let draft_id = ui.make_persistent_id("percentage");
            let mut draft = ui
                .data(|data| data.get_temp::<Draft>(draft_id))
                .filter(|draft| draft.zoom == context.zoom)
                .unwrap_or_else(|| Draft {
                    zoom: context.zoom,
                    text: format!("{}%", context.zoom * 100.0),
                });
            let response = ui.add_enabled(
                !context.blocked,
                egui::TextEdit::singleline(&mut draft.text)
                    .desired_width(46.0)
                    .font(egui::FontId::proportional(12.0))
                    .margin(Vec2::ZERO)
                    .frame(egui::Frame::NONE),
            );
            let label = t!(ui.ctx(), "Zoom percentage");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), &label)
            });
            let cancel =
                response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape));
            if cancel
                || response.lost_focus()
                || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
            {
                if !cancel && let Some(value) = crate::zoom::percent(&draft.text) {
                    actions.push(Action::View(ViewAction::SetZoom(value)));
                }
                draft.text = format!("{}%", context.zoom * 100.0);
                response.surrender_focus();
            }
            ui.data_mut(|data| data.insert_temp(draft_id, draft));
            step(ui, context, ViewAction::ZoomOut, actions);
            let mut value = context.zoom * 100.0;
            let response = ui
                .add_enabled_ui(!context.blocked, |ui| {
                    ui.spacing_mut().slider_width = 120.0;
                    ui.spacing_mut().slider_rail_height = 2.0;
                    let accent = if ui.visuals().dark_mode {
                        egui::Color32::from_rgb(76, 194, 255)
                    } else {
                        egui::Color32::from_rgb(0, 120, 215)
                    };
                    ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                    ui.visuals_mut().widgets.inactive.fg_stroke = egui::Stroke::NONE;
                    ui.visuals_mut().widgets.hovered.fg_stroke = egui::Stroke::NONE;
                    ui.visuals_mut().widgets.active.fg_stroke = egui::Stroke::NONE;
                    ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
                    ui.visuals_mut().widgets.active.bg_fill = egui::Color32::TRANSPARENT;
                    let response = ui.add(
                        egui::Slider::new(&mut value, 12.5..=1000.0)
                            .logarithmic(true)
                            .trailing_fill(false)
                            .show_value(false),
                    );
                    let (mut response, tap) = crate::slider_input::finish(ui, response, false);
                    if let Some(fraction) = tap {
                        value = 12.5 * (1000.0_f32 / 12.5).powf(fraction);
                        crate::slider_input::changed_tap(ui, &mut response);
                    }
                    let rect = response.rect;
                    let inset = rect.height() / 2.5;
                    let fraction =
                        (value.clamp(12.5, 1000.0) / 12.5).ln() / (1000.0_f32 / 12.5).ln();
                    let x = egui::lerp(rect.left() + inset..=rect.right() - inset, fraction);
                    let y = rect.center().y;
                    ui.painter().line_segment(
                        [
                            egui::pos2(rect.left() + inset, y),
                            egui::pos2(rect.right() - inset, y),
                        ],
                        egui::Stroke::new(2.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    );
                    if variant == crate::ClassicVariant::V7 && !ui.visuals().dark_mode {
                        let thumb =
                            egui::Rect::from_center_size(egui::pos2(x, y), Vec2::new(10.0, 17.0));
                        crate::legacy_surface::gradient(
                            ui,
                            thumb,
                            egui::Color32::WHITE,
                            egui::Color32::from_rgb(205, 214, 224),
                        );
                        ui.painter().rect_stroke(
                            thumb,
                            2,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(125, 143, 160)),
                            egui::StrokeKind::Inside,
                        );
                        for dx in [-1.5, 1.5] {
                            ui.painter().line_segment(
                                [egui::pos2(x + dx, y - 3.0), egui::pos2(x + dx, y + 3.0)],
                                egui::Stroke::new(1.0, egui::Color32::from_rgb(155, 167, 179)),
                            );
                        }
                    } else {
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![
                                egui::pos2(x - 4.0, y - 7.0),
                                egui::pos2(x + 4.0, y - 7.0),
                                egui::pos2(x + 4.0, y + 4.0),
                                egui::pos2(x, y + 8.0),
                                egui::pos2(x - 4.0, y + 4.0),
                            ],
                            accent,
                            egui::Stroke::NONE,
                        ));
                    }
                    response
                })
                .inner;
            let label = t!(ui.ctx(), "Canvas zoom");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), &label)
            });
            if response.changed() {
                actions.push(Action::View(ViewAction::SetZoom(
                    (value * 100.0).round() as u32
                )));
            }
            step(ui, context, ViewAction::ZoomIn, actions);
        });
    });
}

fn step(ui: &mut egui::Ui, context: Context, view: ViewAction, actions: &mut Vec<Action>) {
    let action = Action::View(view);
    let response = ui.add_enabled(
        action.enabled(context),
        egui::Button::new("")
            .min_size(Vec2::splat(18.0))
            .frame(false),
    );
    let ink = if action.enabled(context) {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let center = response.rect.center();
    ui.painter()
        .circle_stroke(center, 6.0, egui::Stroke::new(0.8, ink));
    ui.painter().line_segment(
        [center - Vec2::new(3.0, 0.0), center + Vec2::new(3.0, 0.0)],
        egui::Stroke::new(0.8, ink),
    );
    if view == ViewAction::ZoomIn {
        ui.painter().line_segment(
            [center - Vec2::new(0.0, 3.0), center + Vec2::new(0.0, 3.0)],
            egui::Stroke::new(0.8, ink),
        );
    }
    let label = t!(ui.ctx(), action.descriptor().label);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, action.enabled(context), &label)
    });
    if response.on_hover_text(label).clicked() {
        actions.push(action);
    }
}
