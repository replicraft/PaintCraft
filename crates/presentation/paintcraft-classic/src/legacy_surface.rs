//! Windows 7 ribbon surfaces have a four-stop gloss and an inset highlight.
use super::ClassicVariant;
use egui::{Color32, Rect, Response, Stroke, StrokeKind, Ui};

pub(super) fn gradient(ui: &Ui, rect: Rect, top: Color32, bottom: Color32) {
    ui.painter().add(gradient_shape(rect, top, bottom));
}

fn gradient_shape(rect: Rect, top: Color32, bottom: Color32) -> egui::Shape {
    let mut mesh = egui::epaint::Mesh::default();
    for (point, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.colored_vertex(point, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    egui::Shape::mesh(mesh)
}

/// Reserve the background before the widget so its artwork remains on top.
pub(super) fn button(
    ui: &mut Ui,
    variant: ClassicVariant,
    button: egui::Button<'_>,
    selected: bool,
) -> Response {
    if variant != ClassicVariant::V7 || ui.visuals().dark_mode {
        return ui.add(button.selected(selected).frame_when_inactive(selected));
    }
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(button.frame(false));
    if selected || response.hovered() || response.is_pointer_button_down_on() {
        let rect = response.rect.shrink(0.5);
        let middle = rect.top() + rect.height() * 0.45;
        let (top, upper, lower, bottom) = if selected {
            (
                [255, 242, 200],
                [255, 228, 149],
                [255, 213, 102],
                [255, 236, 170],
            )
        } else {
            (
                [255, 252, 230],
                [255, 244, 199],
                [255, 232, 151],
                [255, 246, 213],
            )
        };
        let rgb = |v: [u8; 3]| Color32::from_rgb(v[0], v[1], v[2]);
        ui.painter().set(
            background,
            egui::Shape::Vec(vec![
                gradient_shape(
                    Rect::from_min_max(rect.min, egui::pos2(rect.right(), middle)),
                    rgb(top),
                    rgb(upper),
                ),
                gradient_shape(
                    Rect::from_min_max(egui::pos2(rect.left(), middle), rect.max),
                    rgb(lower),
                    rgb(bottom),
                ),
                egui::Shape::rect_stroke(
                    rect,
                    2,
                    Stroke::new(1.0, Color32::from_rgb(194, 155, 70)),
                    StrokeKind::Inside,
                ),
                egui::Shape::rect_stroke(
                    rect.shrink(1.0),
                    1,
                    Stroke::new(1.0, Color32::from_white_alpha(165)),
                    StrokeKind::Inside,
                ),
            ]),
        );
    }
    response
}
