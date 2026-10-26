//! Shape-resource browsing. Hosts retain exact source resources and own I/O.
use crate::t;

#[derive(Clone, Debug)]
pub struct Path {
    pub open: bool,
    /// Normalized x/y control, anchor and control coordinates for each knot.
    pub knots: Vec<[f64; 6]>,
}
#[derive(Clone, Debug)]
pub struct Shape {
    pub name: String,
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub paths: Vec<Path>,
}
#[derive(Clone, Debug)]
pub struct Resource {
    pub name: String,
    pub shapes: Vec<Shape>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Close,
    Import,
    Export,
    Select(usize),
}

pub fn dialog(
    ctx: &egui::Context,
    resource: Option<&Resource>,
    busy: bool,
    error: Option<&str>,
    selected: usize,
) -> Response {
    let mut response = Response::Pending;
    egui::Modal::new(egui::Id::unique("studio-shape-assets")).show(ctx, |ui| {
        ui.set_width(520.0_f32.min((ctx.viewport_rect().width() - 48.0).max(220.0)));
        ui.heading(t!(ctx, "Shape resources"));
        egui::ScrollArea::vertical()
            .id_salt("shape-resource-dialog")
            .max_height((ctx.viewport_rect().height() - 160.0).max(120.0))
            .show(ui, |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(t!(ctx, "Import shapes…")).clicked() {
                            response = Response::Import;
                        }
                        if ui.add_enabled(resource.is_some(), egui::Button::new(t!(ctx, "Export shapes…"))).clicked() {
                            response = Response::Export;
                        }
                    });
                });
                if busy { ui.spinner(); }
                if let Some(error) = error { ui.colored_label(ui.visuals().error_fg_color, error); }
                ui.label(t!(ctx, "Browse and export custom shape resources. Selecting a preview does not change your painting."));
                if let Some(resource) = resource {
                    ui.add(egui::Label::new(&resource.name).truncate());
                    egui::ScrollArea::vertical()
                        .id_salt("shape-resource-list")
                        .max_height(120.0)
                        .auto_shrink([false, false])
                        .show_rows(ui, 24.0, resource.shapes.len(), |ui, range| {
                            for index in range {
                                ui.push_id(index, |ui| {
                                    let name = if resource.shapes[index].name.is_empty() { t!(ctx, "Unnamed shape") } else { resource.shapes[index].name.clone() };
                                    let clicked = ui.add_enabled_ui(!busy, |ui| {
                                        ui.add_sized(
                                            egui::vec2(ui.available_width(), 24.0),
                                            egui::Button::selectable(index == selected, name).truncate(),
                                        ).clicked()
                                    }).inner;
                                    if clicked {
                                        response = Response::Select(index);
                                    }
                                });
                            }
                        });
                    if let Some(shape) = resource.shapes.get(selected) {
                        ui.add(egui::Label::new(&shape.id).truncate());
                        ui.label(t!(ctx, "Outline preview"));
                        ui.label(t!(ctx, "Fill and Boolean operations are preserved in the resource; this preview shows outlines only."));
                        preview(ui, shape);
                    }
                } else {
                    ui.label(t!(ctx, "Import an Adobe custom shapes file to browse its shape presets."));
                }
            });
        ui.separator();
        if ui.add_enabled(!busy, egui::Button::new(t!(ctx, "Close"))).clicked() { response = Response::Close; }
    });
    if !busy && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        response = Response::Close;
    }
    response
}

fn preview(ui: &mut egui::Ui, shape: &Shape) {
    let knots: usize = shape.paths.iter().map(|p| p.knots.len()).sum();
    if knots > 4096 {
        ui.label(t!(
            ui.ctx(),
            "This shape is too complex to preview. Its original paths are preserved."
        ));
        return;
    }
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 160.0));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let mut low = [0.0f64; 2];
    let mut high = [
        f64::from(shape.width.max(1)),
        f64::from(shape.height.max(1)),
    ];
    for knot in shape.paths.iter().flat_map(|p| &p.knots) {
        for pair in knot.as_chunks::<2>().0 {
            for axis in 0..2 {
                let value = pair[axis]
                    * if axis == 0 {
                        f64::from(shape.width)
                    } else {
                        f64::from(shape.height)
                    };
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
    }
    let scale = (f64::from(rect.width() - 16.0) / (high[0] - low[0]))
        .min(f64::from(rect.height() - 16.0) / (high[1] - low[1]));
    let point = |x: f64, y: f64| {
        rect.center()
            + egui::vec2(
                ((x * f64::from(shape.width) - (high[0] + low[0]) / 2.0) * scale) as f32,
                ((y * f64::from(shape.height) - (high[1] + low[1]) / 2.0) * scale) as f32,
            )
    };
    let stroke = egui::Stroke::new(1.5, ui.visuals().text_color());
    for path in &shape.paths {
        let draw = |a: &[f64; 6], b: &[f64; 6]| {
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [
                    point(a[2], a[3]),
                    point(a[4], a[5]),
                    point(b[0], b[1]),
                    point(b[2], b[3]),
                ],
                false,
                egui::Color32::TRANSPARENT,
                stroke,
            ));
        };
        for pair in path.knots.windows(2) {
            draw(&pair[0], &pair[1]);
        }
        if !path.open && path.knots.len() > 1 {
            draw(path.knots.last().unwrap(), &path.knots[0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_preview_uses_cubic_controls_and_closes_only_closed_paths() {
        for (open, expected) in [(true, 1), (false, 2)] {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    preview(
                        ui,
                        &Shape {
                            name: "Curve".into(),
                            id: "curve".into(),
                            width: 200,
                            height: 100,
                            paths: vec![Path {
                                open,
                                knots: vec![
                                    [0.0, 0.0, 0.25, 0.5, 0.5, 0.0],
                                    [0.5, 1.0, 0.75, 0.5, 1.0, 0.5],
                                ],
                            }],
                        },
                    );
                },
            );
            output.textures_delta.clear();
            let curves: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::CubicBezier(curve) = &s.shape {
                        Some(curve)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(curves.len(), expected);
            let points = curves[0].points;
            assert_eq!(points[0].y, points[3].y);
            assert_eq!(points[1].x, points[2].x);
            assert!(points[1].y < points[0].y);
            assert!(points[2].y > points[3].y);
            assert!((points[1].x - (points[0].x + points[3].x) / 2.0).abs() < 0.001);
        }
    }

    #[test]
    fn resource_preview_policy_is_translated_in_every_language() {
        for language in crate::i18n::Language::ALL
            .into_iter()
            .filter(|l| *l != crate::i18n::Language::English)
        {
            for source in [
                "Shape resources",
                "Shape resources…",
                "Import shapes…",
                "Export shapes…",
                "Outline preview",
                "Unnamed shape",
                "Browse and export custom shape resources. Selecting a preview does not change your painting.",
                "Fill and Boolean operations are preserved in the resource; this preview shows outlines only.",
                "This shape is too complex to preview. Its original paths are preserved.",
            ] {
                assert_ne!(
                    crate::i18n::translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
        }
    }

    #[test]
    fn shape_resource_actions_are_studio_only() {
        use crate::actions::{Action, Frontend};
        assert!(Action::ShapeResources.available(Frontend::Studio));
        assert!(!Action::ShapeResources.available(Frontend::Classic));
        assert_ne!(
            Action::ShapeResources.activation_code(),
            Action::Palettes.activation_code()
        );
    }
}
