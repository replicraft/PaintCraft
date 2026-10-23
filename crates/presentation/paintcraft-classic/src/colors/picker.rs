use super::*;

fn color(h: f32, s: f32, v: f32) -> Color32 {
    let rgba = Hsva { h, s, v, a: 1.0 }.to_srgba_unmultiplied();
    Color32::from_rgb(rgba[0], rgba[1], rgba[2])
}

pub(super) fn square(ui: &mut egui::Ui, editor: &mut ColorEditor, side: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), "Hue and saturation")
    });
    let mut mesh = egui::epaint::Mesh::default();
    const COLUMNS: u32 = 36;
    const ROWS: u32 = 16;
    for y in 0..=ROWS {
        for x in 0..=COLUMNS {
            let (h, white) = (x as f32 / COLUMNS as f32, y as f32 / ROWS as f32);
            mesh.colored_vertex(
                rect.min + rect.size() * egui::vec2(h, white),
                color(h, 1.0 - white, 1.0),
            );
            if x > 0 && y > 0 {
                let i = y * (COLUMNS + 1) + x;
                mesh.add_triangle(i - COLUMNS - 2, i - 1, i);
                mesh.add_triangle(i - COLUMNS - 2, i, i - COLUMNS - 1);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
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
    ui.painter()
        .circle_stroke(marker, 4.0, egui::Stroke::new(2.0, Color32::BLACK));
    ui.painter()
        .circle_stroke(marker, 3.0, egui::Stroke::new(1.0, Color32::WHITE));
}

pub(super) fn brightness(ui: &mut egui::Ui, editor: &mut ColorEditor, height: f32) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(32.0, height), egui::Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), editor.hsv.v.into(), "Brightness")
    });
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=32 {
        let t = i as f32 / 32.0;
        let y = rect.top() + t * rect.height();
        let color = color(editor.hsv.h, editor.hsv.s, 1.0 - t);
        mesh.colored_vertex(egui::pos2(rect.left() + 8.0, y), color);
        mesh.colored_vertex(egui::pos2(rect.right() - 8.0, y), color);
        if i > 0 {
            let i = i * 2;
            mesh.add_triangle(i - 2, i - 1, i + 1);
            mesh.add_triangle(i - 2, i + 1, i);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        editor.hsv.v = 1.0 - ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        editor.set_hsv();
        response.request_focus();
    }
    if response.has_focus() && ui.ctx().current_pass_index() == 0 {
        let delta = ui.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as i32
                - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as i32
        });
        if delta != 0 {
            editor.hsv.v = (editor.hsv.v + delta as f32 / 100.0).clamp(0.0, 1.0);
            editor.set_hsv();
        }
    }
    let marker = egui::pos2(
        rect.center().x,
        rect.top() + (1.0 - editor.hsv.v) * rect.height(),
    );
    ui.painter().circle_filled(marker, 5.0, Color32::WHITE);
    ui.painter()
        .circle_stroke(marker, 5.0, egui::Stroke::new(1.0, Color32::GRAY));
}
