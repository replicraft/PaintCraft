use super::*;
use paintcraft_ui::t;
mod gradients;

fn color(h: f32, s: f32, v: f32) -> Color32 {
    let rgba = Hsva { h, s, v, a: 1.0 }.to_srgba_unmultiplied();
    Color32::from_rgb(rgba[0], rgba[1], rgba[2])
}

pub(super) fn square(ui: &mut egui::Ui, editor: &mut ColorEditor, side: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click_and_drag());
    let accessible_label = t!(ui.ctx(), "Hue and saturation");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), &accessible_label)
    });
    gradients::square(ui, rect);
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        editor.hsv.h = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        editor.hsv.s = 1.0 - ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        editor.set_hsv();
        response.request_focus();
    }
    if response.has_focus() && ui.ctx().current_pass_index() == 0 {
        let (h, s) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) as i32
                    - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) as i32,
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as i32
                    - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as i32,
            )
        });
        if h != 0 || s != 0 {
            editor.hsv.h = (editor.hsv.h + h as f32 / 360.0).rem_euclid(1.0);
            editor.hsv.s = (editor.hsv.s + s as f32 / 100.0).clamp(0.0, 1.0);
            editor.set_hsv();
        }
    }
    let marker = rect.min + rect.size() * egui::vec2(editor.hsv.h, 1.0 - editor.hsv.s);
    let painter = ui.painter().with_clip_rect(rect);
    painter.circle_stroke(marker, 7.0, egui::Stroke::new(2.0, Color32::BLACK));
    painter.circle_stroke(marker, 6.0, egui::Stroke::new(2.0, Color32::WHITE));
}

pub(super) fn brightness(ui: &mut egui::Ui, editor: &mut ColorEditor, height: f32) {
    let previous = editor.hsv.v;
    // Keep the gradient/marker presentation, but let egui own the slider's
    // pointer, keyboard and accessibility actions rather than merely its role.
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = height;
            ui.spacing_mut().interact_size.y = 32.0;
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                state.bg_fill = Color32::TRANSPARENT;
                state.fg_stroke = egui::Stroke::NONE;
            }
            ui.add_sized(
                Vec2::new(32.0, height),
                egui::Slider::new(&mut editor.hsv.v, 0.0..=1.0)
                    .vertical()
                    .show_value(false)
                    .trailing_fill(false)
                    .handle_shape(egui::style::HandleShape::Circle),
            )
        })
        .inner;
    let (response, tap) = crate::slider_input::finish(ui, response, true);
    let rect = response.rect;
    let travel = rect.y_range().shrink(rect.width() / 2.5);
    if let Some(fraction) = tap {
        editor.hsv.v = fraction;
    }
    if editor.hsv.v != previous {
        editor.set_hsv();
    }
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), editor.hsv.v.into(), "Brightness")
    });
    let rail = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top()),
        egui::pos2(rect.right() - 8.0, rect.bottom()),
    );
    gradients::brightness(ui, rail, editor.hsv.h, editor.hsv.s);
    // Follow egui's circle-handle center inset, independently checked against
    // its actual painted shell in the GUI tests.
    let marker = egui::pos2(
        rect.center().x,
        egui::lerp(travel.min..=travel.max, 1.0 - editor.hsv.v),
    );
    ui.painter()
        .circle_filled(marker, 10.0, Color32::from_gray(76));
    ui.painter().circle_filled(marker, 6.0, Color32::WHITE);
    if response.has_focus() {
        ui.painter()
            .circle_stroke(marker, 9.0, ui.visuals().selection.stroke);
    }
}
