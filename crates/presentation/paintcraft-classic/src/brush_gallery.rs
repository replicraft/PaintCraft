//! Brush gallery bound to the shared deterministic brush media.
use super::{Action, Controls};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::tr;

use paintcraft_core::types::brush::BrushMedia;

pub(super) fn gallery(
    ui: &mut egui::Ui,
    state: &mut Controls,
    actions: &mut Vec<Action>,
    opening: bool,
) {
    let height = super::popup_layout::height(ui, 56.0, 520.0);
    ui.visuals_mut().override_text_color = Some(Color32::from_gray(if ui.visuals().dark_mode {
        245
    } else {
        24
    }));
    ui.set_width(320.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical()
        .id_salt("classic-brush-media-gallery")
        .max_height(height)
        .show(ui, |ui| {
            for (index, media) in BrushMedia::ALL.into_iter().enumerate() {
                let label = media.name();
                let enabled = true;
                let selected = state.tool == Tool::Brush
                    && state.options.round
                    && state.options.media == media;
                let response = row(
                    ui,
                    label,
                    if index == 0 { "Round brush" } else { label },
                    selected,
                    enabled,
                    index,
                );
                if opening && selected {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                if response.clicked() {
                    state.options.media = media;
                    choose(state, actions, true);
                    ui.close();
                }
            }
            ui.separator();
            let selected = state.tool == Tool::Brush && !state.options.round;
            let square = row(ui, "Square brush", "Square brush", selected, true, 9);
            if opening && selected {
                square.scroll_to_me(Some(egui::Align::Center));
            }
            if square.clicked() {
                choose(state, actions, false);
                ui.close();
            }
        });
}

fn choose(state: &mut Controls, actions: &mut Vec<Action>, round: bool) {
    if !round {
        state.options.media = BrushMedia::Plain;
    }
    state.tool = Tool::Brush;
    state.options.round = round;
    actions.push(Action::SelectTool(Tool::Brush));
}

fn row(
    ui: &mut egui::Ui,
    label: &str,
    accessible: &str,
    selected: bool,
    enabled: bool,
    index: usize,
) -> egui::Response {
    let localized_label = tr(ui.ctx(), label);
    let label = localized_label.as_str();
    let localized_accessible = tr(ui.ctx(), accessible);
    let accessible = localized_accessible.as_str();
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(" ")
            .selected(selected)
            .frame_when_inactive(selected)
            .frame(true)
            .min_size(Vec2::new(320.0, 56.0)),
    );
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            4,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled() && enabled,
            selected,
            accessible,
        )
    });
    let ink = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    ui.painter().text(
        response.rect.left_center() + Vec2::new(14.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(16.0),
        ink,
    );
    let sample = Rect::from_center_size(
        response.rect.right_center() - Vec2::new(90.0, 0.0),
        Vec2::new(84.0, 24.0),
    );
    draw_sample(ui.painter(), sample, index, ink);
    if enabled {
        response
    } else {
        response.on_disabled_hover_text(tr(ui.ctx(), "This brush is not available yet."))
    }
}

/// Deterministic, original UI illustrations. These are not document stroke previews.
fn draw_sample(painter: &egui::Painter, rect: Rect, media: usize, ink: Color32) {
    let point = |t: f32| {
        Pos2::new(
            rect.left() + t * rect.width(),
            rect.center().y - (t * std::f32::consts::TAU).sin() * 5.0,
        )
    };
    if matches!(media, 3 | 5 | 7) {
        let mut seed = 0x9142_371bu32;
        for _ in 0..400 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let t = (seed >> 8) as f32 / 16777216.0;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let jitter = (seed >> 8) as f32 / 16777216.0 - 0.5;
            let spread = if media == 3 {
                9.0
            } else if media == 5 {
                6.0
            } else {
                3.0
            };
            painter.circle_filled(
                point(t) + Vec2::new(0.0, jitter * spread),
                0.4,
                ink.gamma_multiply(0.6),
            );
        }
        return;
    }
    if media == 9 {
        painter.rect_filled(
            Rect::from_center_size(rect.center(), Vec2::new(80.0, 7.0)),
            0.0,
            ink,
        );
        return;
    }
    if media == 4 {
        let band = (0..=64).map(|i| point(i as f32 / 64.0)).collect();
        painter.add(egui::epaint::PathShape::line(
            band,
            Stroke::new(8.0, ink.gamma_multiply(0.3)),
        ));
        for offset in [-3.0_f32, -1.5, 0.0, 1.5, 3.0] {
            let strand = |t: f32| {
                point(t)
                    + Vec2::new(
                        0.0,
                        offset + 0.4 * (t * std::f32::consts::TAU * 3.0 + offset).sin(),
                    )
            };
            for i in 0..64 {
                let t = i as f32 / 64.0;
                painter.line_segment(
                    [strand(t), strand((i + 1) as f32 / 64.0)],
                    Stroke::new(
                        0.65,
                        ink.gamma_multiply(0.75 + 0.2 * (t * 8.0 + offset).sin()),
                    ),
                );
            }
        }
        return;
    }
    if media == 0 {
        for end in [0.0, 1.0] {
            painter.circle_filled(point(end), 3.0, ink);
        }
    }
    let width = |t: f32| match media {
        1 => 1.0 + 4.0 * (t * std::f32::consts::TAU).sin().abs(),
        2 => 2.0 + 5.0 * (t * std::f32::consts::TAU).cos().abs(),
        6 => 10.0,
        8 => 8.0,
        _ => 6.0,
    };
    let color = if media == 8 {
        ink.gamma_multiply(0.45)
    } else {
        ink
    };
    draw_band(painter, point, width, color);
}

/// Shared vertices avoid antialiasing seams where adjacent sample segments meet.
fn draw_band(
    painter: &egui::Painter,
    point: impl Fn(f32) -> Pos2,
    width: impl Fn(f32) -> f32,
    ink: Color32,
) {
    let feather = painter.ctx().pixels_per_point().recip();
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..=64 {
        let t = i as f32 / 64.0;
        let direction = point(t + 0.001) - point(t - 0.001);
        let normal = Vec2::new(-direction.y, direction.x).normalized();
        let inner = (width(t) * 0.5 - feather * 0.5).max(0.0);
        let outer = width(t) * 0.5 + feather * 0.5;
        for (offset, color) in [
            (outer, Color32::TRANSPARENT),
            (inner, ink),
            (-inner, ink),
            (-outer, Color32::TRANSPARENT),
        ] {
            mesh.colored_vertex(point(t) + normal * offset, color);
        }
    }
    for i in 0..64 {
        for band in 0..3 {
            let a = i * 4 + band;
            mesh.add_triangle(a, a + 1, a + 4);
            mesh.add_triangle(a + 1, a + 5, a + 4);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
