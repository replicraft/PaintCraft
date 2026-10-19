//! Reference-shaped gallery; sample decorations do not implement brush media.
use super::{Action, Controls};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use paintcraft_tools::Tool;

const PRESETS: [&str; 9] = [
    "Brush",
    "Calligraphy brush",
    "Calligraphy pen",
    "Airbrush",
    "Oil brush",
    "Crayon",
    "Marker",
    "Natural pencil",
    "Watercolor brush",
];

pub(super) fn gallery(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    // A popup starts with a conservative area estimate. Give the gallery the
    // actual remaining screen height so lower rows are not clipped by that estimate.
    let height = (ui.ctx().content_rect().bottom() - ui.cursor().min.y - 12.0).clamp(44.0, 520.0);
    ui.set_height(height);
    egui::ScrollArea::vertical()
        .id_salt("classic-brush-media-gallery")
        .max_height(height)
        .show(ui, |ui| {
            for (index, label) in PRESETS.into_iter().enumerate() {
                let enabled = index == 0;
                let selected = enabled && state.tool == Tool::Brush && state.options.round;
                let response = row(
                    ui,
                    label,
                    if enabled { "Round brush" } else { label },
                    selected,
                    enabled,
                    index,
                );
                if response.clicked() {
                    choose(state, actions, true);
                    ui.close();
                }
            }
            ui.separator();
            let selected = state.tool == Tool::Brush && !state.options.round;
            if row(ui, "Square brush", "Square brush", selected, true, 9).clicked() {
                choose(state, actions, false);
                ui.close();
            }
        });
}

fn choose(state: &mut Controls, actions: &mut Vec<Action>, round: bool) {
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
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(" ")
            .selected(selected)
            .min_size(Vec2::new(280.0, 44.0)),
    );
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
        response.rect.left_center() + Vec2::new(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        ink,
    );
    let sample = Rect::from_center_size(
        response.rect.right_center() - Vec2::new(54.0, 0.0),
        Vec2::new(84.0, 24.0),
    );
    draw_sample(ui.painter(), sample, index, ink);
    if enabled {
        response
    } else {
        response.on_disabled_hover_text("This brush is not available yet.")
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
    for i in 0..64 {
        let t = i as f32 / 64.0;
        let width = match media {
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
        painter.line_segment(
            [point(t), point((i + 1) as f32 / 64.0)],
            Stroke::new(width, color),
        );
        if media == 4 {
            for offset in [-2.0, 0.0, 2.0] {
                painter.line_segment(
                    [
                        point(t) + Vec2::new(0.0, offset),
                        point((i + 1) as f32 / 64.0) + Vec2::new(0.0, offset),
                    ],
                    Stroke::new(0.7, ink.gamma_multiply(0.4)),
                );
            }
        }
    }
}
