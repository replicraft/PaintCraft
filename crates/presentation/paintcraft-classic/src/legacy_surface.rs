//! Windows 7 ribbon surfaces have a four-stop gloss and an inset highlight.
use super::ClassicVariant;
use egui::{Color32, Rect, Response, Stroke, StrokeKind, Ui, Vec2};
use paintcraft_ui::i18n::tr;
use paintcraft_ui::icons::{self, Icon};

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
    if selected
        || response.hovered()
        || response.has_focus()
        || response.is_pointer_button_down_on()
    {
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

/// Historical popup frames and items retain square corners after egui menu styling.
pub(super) fn menu_config() -> egui::containers::menu::MenuConfig {
    egui::containers::menu::MenuConfig::new().style(|style: &mut egui::Style| {
        egui::containers::menu::menu_style(style);
        style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::ZERO;
        }
    })
}

pub(super) fn open_state(ui: &Ui, response: &Response, label: &str) {
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::menu(response).get_id());
    if open && ui.is_enabled() {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            0,
            ui.visuals().selection.stroke,
            StrokeKind::Inside,
        );
    }
    let label = tr(ui.ctx(), label);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), open, &label)
    });
}

/// Menu navigation is linear by default. A gallery follows its visible rows.
pub(super) fn grid_navigation(ui: &Ui, entries: &[egui::Id], columns: usize) {
    if ui.ctx().current_pass_index() != 0 {
        return;
    }
    let focused = ui.memory(|memory| memory.focused());
    let Some(index) = entries.iter().position(|id| Some(*id) == focused) else {
        return;
    };
    let key = ui.input_mut(|input| {
        [
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
        ]
        .into_iter()
        .find(|key| input.consume_key(egui::Modifiers::NONE, *key))
    });
    let Some(key) = key else {
        return;
    };
    let next = match key {
        egui::Key::ArrowLeft => {
            if index % columns > 0 {
                index - 1
            } else {
                index
            }
        }
        egui::Key::ArrowRight => {
            if index % columns + 1 < columns && index + 1 < entries.len() {
                index + 1
            } else {
                index
            }
        }
        egui::Key::ArrowUp => index.saturating_sub(columns),
        egui::Key::ArrowDown => (index + columns).min(entries.len() - 1),
        _ => return,
    };
    // The enclosing MenuState sees None and does not replace this grid move
    // with its own one-dimensional Up/Down/Left behavior.
    ui.memory_mut(|memory| {
        memory.move_focus(egui::FocusDirection::None);
        memory.request_focus(entries[next]);
    });
    ui.ctx().request_repaint();
}

/// Compact historical command row shared by ribbon popups.
pub(super) fn menu_row(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    label: &str,
    accessible: &str,
    icon: Option<Icon>,
    selected: Option<bool>,
    shortcut: Option<&str>,
) -> egui::Response {
    let response = button(
        ui,
        variant,
        egui::Button::new(" ").min_size(Vec2::new(ui.available_width().max(220.0), 24.0)),
        false,
    );
    let accessible = tr(ui.ctx(), accessible);
    response.widget_info(|| match selected {
        Some(selected) => {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &accessible)
        }
        None => egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &accessible),
    });
    let icon_rect = Rect::from_center_size(
        response.rect.left_center() + Vec2::new(14.0, 0.0),
        Vec2::splat(16.0),
    );
    if selected == Some(true) {
        ui.painter()
            .rect_filled(icon_rect.expand(2.0), 1, ui.visuals().selection.bg_fill);
        ui.painter().rect_stroke(
            icon_rect.expand(2.0),
            1,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    if let Some(icon) = icon {
        if matches!(icon, Icon::RectangleSelect | Icon::DeleteLayer) {
            icons::legacy_image(ui.ctx(), icon, 16.0).paint_at(ui, icon_rect);
        } else {
            let ink = if ui.visuals().dark_mode {
                Color32::from_rgb(150, 205, 240)
            } else {
                Color32::from_rgb(46, 91, 130)
            };
            icons::classic_menu_image(ui.ctx(), icon, 16.0, ink).paint_at(ui, icon_rect);
        }
    }
    ui.painter().text(
        response.rect.left_center() + Vec2::new(32.0, 0.0),
        egui::Align2::LEFT_CENTER,
        tr(ui.ctx(), label),
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            response.rect.right_center() - Vec2::new(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            shortcut,
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
    response
}
