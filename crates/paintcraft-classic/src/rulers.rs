//! Fixed ruler strips around the scrolling Classic canvas; all units are pixels.
use super::ruler_axis::ticks;
use egui::{Align2, FontId, Pos2, Rect, Stroke, Vec2};

pub const RULER_SIZE: f32 = 22.0;

pub(super) fn paint(ui: &egui::Ui, viewport: Rect, canvas: Rect, extent: (u32, u32)) {
    let ppp = ui.ctx().pixels_per_point();
    let horizontal = Rect::from_min_max(
        viewport.min + Vec2::new(RULER_SIZE, 0.0),
        Pos2::new(viewport.right(), viewport.top() + RULER_SIZE),
    );
    let vertical = Rect::from_min_max(
        viewport.min + Vec2::new(0.0, RULER_SIZE),
        Pos2::new(viewport.left() + RULER_SIZE, viewport.bottom()),
    );
    let painter = ui.painter();
    let ink = ui.visuals().text_color();
    let stroke = Stroke::new(
        1.0 / ppp,
        ui.visuals().widgets.noninteractive.fg_stroke.color,
    );
    painter.rect_filled(
        Rect::from_min_size(viewport.min, Vec2::splat(RULER_SIZE)),
        0.0,
        ui.visuals().panel_fill,
    );
    painter.text(
        viewport.min + Vec2::splat(RULER_SIZE / 2.0),
        Align2::CENTER_CENTER,
        "px",
        FontId::proportional(9.0),
        ink,
    );
    for (rect, axis, label) in [
        (horizontal, 0, "Horizontal ruler (pixels)"),
        (vertical, 1, "Vertical ruler (pixels)"),
    ] {
        ui.interact(rect, ui.scope_id().with(label), egui::Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), label));
        let painter = painter.with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, 0.0, ui.visuals().panel_fill);
        let (origin, scale, end, range) = if axis == 0 {
            (
                canvas.left(),
                canvas.width() / extent.0.max(1) as f32,
                extent.0,
                (rect.left(), rect.right()),
            )
        } else {
            (
                canvas.top(),
                canvas.height() / extent.1.max(1) as f32,
                extent.1,
                (rect.top(), rect.bottom()),
            )
        };
        for tick in ticks(origin, scale, end, range) {
            let length = if tick.major { 9.0 } else { 4.0 };
            let (a, b) = if axis == 0 {
                (
                    Pos2::new(tick.position, rect.bottom() - length),
                    Pos2::new(tick.position, rect.bottom()),
                )
            } else {
                (
                    Pos2::new(rect.right() - length, tick.position),
                    Pos2::new(rect.right(), tick.position),
                )
            };
            painter.line_segment([a, b], stroke);
            if tick.major {
                if axis == 0 {
                    painter.text(
                        Pos2::new(tick.position + 3.0, rect.top() + 1.0),
                        Align2::LEFT_TOP,
                        tick.value.to_string(),
                        FontId::proportional(10.0),
                        ink,
                    );
                } else {
                    let galley = painter.layout_no_wrap(
                        tick.value.to_string(),
                        FontId::proportional(10.0),
                        ink,
                    );
                    painter.add(
                        egui::epaint::TextShape::new(
                            Pos2::new(rect.left() + 12.0, tick.position + 3.0),
                            galley,
                            ink,
                        )
                        .with_angle(std::f32::consts::FRAC_PI_2),
                    );
                }
            }
        }
        let (a, b) = if axis == 0 {
            (rect.left_bottom(), rect.right_bottom())
        } else {
            (rect.right_top(), rect.right_bottom())
        };
        painter.line_segment([a, b], stroke);
        if let Some(pos) = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|pos| canvas.contains(*pos) && viewport.contains(*pos))
        {
            let cursor = Stroke::new(1.0 / ppp, ui.visuals().selection.stroke.color);
            let (a, b) = if axis == 0 {
                (
                    Pos2::new(pos.x, rect.top()),
                    Pos2::new(pos.x, rect.bottom()),
                )
            } else {
                (
                    Pos2::new(rect.left(), pos.y),
                    Pos2::new(rect.right(), pos.y),
                )
            };
            painter.line_segment([a, b], cursor);
        }
    }
}
