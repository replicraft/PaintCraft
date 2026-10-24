//! HSL spectrum and luminosity use the common-dialog scale, not HSV brightness.
use super::*;
use egui::{Pos2, Rect, Stroke};
use paintcraft_ui::t;

fn rgb(h: f32, s: f32, l: f32) -> Color32 {
    let [r, g, b] = super::super::fields::from_hsl([h, s, l]);
    Color32::from_rgb(r, g, b)
}
fn spectrum_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::unique("historical-hsl-spectrum");
    if let Some(texture) = ctx.data_mut(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let pixels = (0..128)
        .flat_map(|y| {
            (0..128).map(move |x| {
                rgb(
                    x as f32 * 358.5 / 127.0,
                    100.0 - y as f32 * 100.0 / 127.0,
                    50.0,
                )
            })
        })
        .collect();
    let texture = ctx.load_texture(
        "historical-hsl-spectrum",
        egui::ColorImage::new([128, 128], pixels),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

pub(super) fn spectrum(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(200.0, 160.0), egui::Sense::click_and_drag());
    let label = t!(ui.ctx(), "Hue and saturation");
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), &label));
    let texture = spectrum_texture(ui.ctx());
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let h = (((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 239.0).round() as u16;
        let s =
            ((1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0) * 240.0).round() as u16;
        let mut value = super::fields::native_hsl(editor);
        value[0] = h;
        value[1] = s;
        super::fields::set_native(editor, value);
        response.request_focus();
    }
    if response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        if ui.ctx().current_pass_index() == 0 {
            let (h, s) = ui.input_mut(|input| {
                (
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) as i32
                        - input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) as i32,
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as i32
                        - input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as i32,
                )
            });
            if h != 0 || s != 0 {
                let mut value = super::fields::native_hsl(editor);
                value[0] = (i32::from(value[0]) + h).rem_euclid(240) as u16;
                value[1] = (i32::from(value[1]) + s).clamp(0, 240) as u16;
                super::fields::set_native(editor, value);
            }
        }
    }
    let marker = rect.min
        + Vec2::new(
            editor.hsl[0] / 358.5 * rect.width(),
            (1.0 - editor.hsl[1] / 100.0) * rect.height(),
        );
    let painter = ui.painter().with_clip_rect(rect);
    for direction in [Vec2::X, Vec2::Y] {
        painter.line_segment(
            [marker - direction * 6.0, marker + direction * 6.0],
            Stroke::new(3.0, Color32::WHITE),
        );
        painter.line_segment(
            [marker - direction * 6.0, marker + direction * 6.0],
            Stroke::new(1.0, Color32::BLACK),
        );
    }
}

pub(super) fn luminosity(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    let mut value = f32::from(super::fields::native_hsl(editor)[2]);
    let previous = value;
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = 160.0;
            ui.spacing_mut().interact_size.y = 24.0;
            let widgets = &mut ui.visuals_mut().widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                widget.bg_fill = Color32::TRANSPARENT;
                widget.fg_stroke = Stroke::NONE;
            }
            ui.add(
                egui::Slider::new(&mut value, 0.0..=240.0)
                    .integer()
                    .vertical()
                    .show_value(false)
                    .trailing_fill(false),
            )
        })
        .inner;
    let (response, tap) = crate::slider_input::finish(ui, response, true);
    if let Some(fraction) = tap {
        value = (fraction * 240.0).round();
    }
    if value != previous {
        let mut hsl = super::fields::native_hsl(editor);
        hsl[2] = value as u16;
        super::fields::set_native(editor, hsl);
    }
    let label = t!(ui.ctx(), "Luminosity");
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), value.into(), &label));
    let rect = response.rect;
    let travel = rect.y_range().shrink(rect.width() / 2.5);
    let rail = Rect::from_min_max(
        egui::pos2(rect.left() + 5.0, travel.min),
        egui::pos2(rect.right() - 5.0, travel.max),
    );
    let mut mesh = egui::epaint::Mesh::default();
    for y in 0..=32 {
        let t = y as f32 / 32.0;
        let color = rgb(editor.hsl[0], editor.hsl[1], 100.0 * (1.0 - t));
        mesh.colored_vertex(
            egui::pos2(rail.left(), egui::lerp(rail.y_range(), t)),
            color,
        );
        mesh.colored_vertex(
            egui::pos2(rail.right(), egui::lerp(rail.y_range(), t)),
            color,
        );
        if y > 0 {
            let a = (y - 1) * 2;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let y = egui::lerp(travel.min..=travel.max, 1.0 - value / 240.0);
    ui.painter().line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        Stroke::new(2.0, ui.visuals().text_color()),
    );
}
