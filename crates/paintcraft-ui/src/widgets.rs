//! Shared vector widgets; no operating-system or editor-session side effects.
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Select,
    Pencil,
    Brush,
    Eraser,
    Fill,
    Line,
    Rectangle,
    Ellipse,
    Text,
    New,
    Open,
    Save,
    Export,
    Undo,
    Redo,
}

pub fn icon_button(ui: &mut Ui, label: &str, icon: Icon, selected: bool, large: bool) -> Response {
    let size = if large {
        Vec2::new(64.0, 54.0)
    } else {
        Vec2::splat(30.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    let visuals = ui.style().interact_selectable(&response, false);
    let ink = if selected {
        Color32::from_rgb(0, 95, 184)
    } else {
        visuals.fg_stroke.color
    };
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect(
            rect.shrink(1.0),
            4.0,
            if selected {
                Color32::from_rgb(224, 236, 249)
            } else {
                visuals.bg_fill
            },
            if selected {
                Stroke::new(1.0, ink)
            } else {
                visuals.bg_stroke
            },
            StrokeKind::Inside,
        );
    }
    let center = rect.center()
        - if large {
            Vec2::new(0.0, 7.0)
        } else {
            Vec2::ZERO
        };
    draw_icon(
        ui.painter(),
        Rect::from_center_size(center, Vec2::splat(22.0)),
        icon,
        ink,
    );
    if large {
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            ink,
        );
    }
    response.on_hover_text(label)
}
fn draw_icon(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let point = |x: f32, y: f32| {
        Pos2::new(
            rect.min.x + x * rect.width(),
            rect.min.y + y * rect.height(),
        )
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], Stroke::new(1.5, color));
    };
    let box_at =
        |a: (f32, f32), b: (f32, f32)| Rect::from_min_max(point(a.0, a.1), point(b.0, b.1));
    match icon {
        Icon::Select => {
            p.rect_stroke(
                rect.shrink(3.0),
                0.0,
                Stroke::new(1.0, color),
                StrokeKind::Inside,
            );
            for i in [0.2, 0.8] {
                p.rect_filled(
                    Rect::from_center_size(point(i, i), Vec2::splat(3.0)),
                    0.0,
                    color,
                );
            }
        }
        Icon::Pencil => {
            line((0.2, 0.8), (0.75, 0.15));
            line((0.2, 0.8), (0.4, 0.74));
            line((0.4, 0.74), (0.87, 0.27));
            line((0.75, 0.15), (0.87, 0.27));
        }
        Icon::Brush => {
            line((0.5, 0.52), (0.86, 0.12));
            line((0.62, 0.62), (0.94, 0.2));
            p.circle_filled(point(0.4, 0.7), 4.0, Color32::from_rgb(0, 95, 184));
        }
        Icon::Eraser => {
            p.rect_filled(
                box_at((0.15, 0.3), (0.8, 0.7)),
                2.0,
                Color32::from_rgb(231, 147, 168),
            );
            line((0.12, 0.75), (0.9, 0.75));
            line((0.55, 0.3), (0.55, 0.7));
        }
        Icon::Fill => {
            line((0.2, 0.45), (0.5, 0.15));
            line((0.5, 0.15), (0.8, 0.45));
            line((0.2, 0.45), (0.5, 0.75));
            line((0.5, 0.75), (0.8, 0.45));
            p.circle_filled(point(0.83, 0.77), 2.5, Color32::from_rgb(0, 95, 184));
        }
        Icon::Line => line((0.15, 0.8), (0.85, 0.2)),
        Icon::Rectangle => {
            p.rect_stroke(
                rect.shrink(3.0),
                1.0,
                Stroke::new(1.5, color),
                StrokeKind::Inside,
            );
        }
        Icon::Ellipse => {
            p.circle_stroke(rect.center(), 8.0, Stroke::new(1.5, color));
        }
        Icon::Text => {
            line((0.2, 0.85), (0.5, 0.15));
            line((0.5, 0.15), (0.8, 0.85));
            line((0.33, 0.58), (0.67, 0.58));
        }
        Icon::New => {
            p.rect_stroke(
                box_at((0.2, 0.1), (0.8, 0.9)),
                1.0,
                Stroke::new(1.3, color),
                StrokeKind::Inside,
            );
            line((0.35, 0.4), (0.65, 0.4));
            line((0.5, 0.25), (0.5, 0.55));
        }
        Icon::Open => {
            p.rect_stroke(
                box_at((0.12, 0.3), (0.9, 0.82)),
                1.0,
                Stroke::new(1.3, color),
                StrokeKind::Inside,
            );
            line((0.12, 0.3), (0.12, 0.18));
            line((0.12, 0.18), (0.43, 0.18));
            line((0.43, 0.18), (0.57, 0.3));
        }
        Icon::Save => {
            p.rect_stroke(
                rect.shrink(3.0),
                1.0,
                Stroke::new(1.3, color),
                StrokeKind::Inside,
            );
            p.rect_stroke(
                box_at((0.3, 0.15), (0.65, 0.4)),
                0.0,
                Stroke::new(1.0, color),
                StrokeKind::Inside,
            );
            p.rect_stroke(
                box_at((0.3, 0.55), (0.7, 0.85)),
                0.0,
                Stroke::new(1.0, color),
                StrokeKind::Inside,
            );
        }
        Icon::Export => {
            line((0.5, 0.1), (0.5, 0.7));
            line((0.3, 0.5), (0.5, 0.7));
            line((0.5, 0.7), (0.7, 0.5));
            line((0.15, 0.65), (0.15, 0.9));
            line((0.15, 0.9), (0.85, 0.9));
            line((0.85, 0.9), (0.85, 0.65));
        }
        Icon::Undo | Icon::Redo => {
            let flip = matches!(icon, Icon::Redo);
            let q = |x: f32| if flip { 1.0 - x } else { x };
            line((q(0.25), 0.35), (q(0.75), 0.35));
            line((q(0.75), 0.35), (q(0.8), 0.75));
            line((q(0.25), 0.35), (q(0.45), 0.15));
            line((q(0.25), 0.35), (q(0.45), 0.55));
        }
    }
}
