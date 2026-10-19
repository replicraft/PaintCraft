//! Desktop canvas integration.

use super::*;

impl App {
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui, first_pass: bool) {
        self.canvas_viewport = Some(ui.available_rect_before_wrap());
        let ppp = ui.ctx().pixels_per_point();
        let (w, h) = self.editor.document().canvas;
        let size = View::with_scale((0.0, 0.0), self.zoom, ppp).canvas_size(w, h);
        egui::ScrollArea::both()
            .id_salt(("document-canvas", self.active_tab_id()))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let canvas_size = Vec2::new(size.0, size.1);
                let (response, painter) = if self.experience() == Experience::Studio {
                    let inset = (ui.available_size() - canvas_size) * 0.5;
                    ui.add_space(inset.y.max(0.0));
                    ui.horizontal(|ui| {
                        ui.add_space(inset.x.max(0.0));
                        ui.allocate_painter(canvas_size, Sense::click_and_drag())
                    })
                    .inner
                } else {
                    ui.allocate_painter(canvas_size, Sense::click_and_drag())
                };
                let rect = response.rect;
                self.canvas_rect = Some(rect);
                let visible_canvas = rect.intersect(painter.clip_rect());
                let view = View::with_scale((rect.min.x, rect.min.y), self.zoom, ppp);
                self.hover_doc = ui
                    .input(|i| i.pointer.hover_pos())
                    .filter(|p| visible_canvas.contains(*p))
                    .and_then(|p| view.to_doc((p.x, p.y)));

                if first_pass {
                    let events = ui.input(|i| i.events.clone());
                    for event in events {
                        match event {
                            egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: true,
                                ..
                            } => {
                                let button = match button {
                                    egui::PointerButton::Primary => {
                                        paintcraft_input::PointerButton::Primary
                                    }
                                    egui::PointerButton::Secondary
                                        if self.experience() == Experience::Studio =>
                                    {
                                        paintcraft_input::PointerButton::Secondary
                                    }
                                    _ => continue,
                                };
                                if (visible_canvas.contains(pos)
                                    && ui.ctx().layer_id_at(pos) == Some(painter.layer_id()))
                                    && let Some(d) = view.to_doc((pos.x, pos.y))
                                {
                                    self.editor.pointer_down_button(d, button);
                                }
                            }
                            egui::Event::PointerMoved(pos) => {
                                if let Some(d) = view.to_doc((pos.x, pos.y)) {
                                    self.editor.pointer_move(d);
                                }
                            }
                            egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: false,
                                ..
                            } => {
                                let button = match button {
                                    egui::PointerButton::Primary => {
                                        paintcraft_input::PointerButton::Primary
                                    }
                                    egui::PointerButton::Secondary
                                        if self.experience() == Experience::Studio =>
                                    {
                                        paintcraft_input::PointerButton::Secondary
                                    }
                                    _ => continue,
                                };
                                if self.editor.gesture_button() != Some(button) {
                                    continue;
                                }
                                if let Some(d) = view.to_doc((pos.x, pos.y)) {
                                    self.editor.pointer_move(d);
                                }
                                self.editor.pointer_up_button(button);
                            }
                            _ => {}
                        }
                    }
                }

                self.sync_texture(ui.ctx());

                // Transparency checkerboard, then the composite. Display only: the document is untouched.
                painter.rect_filled(rect, 0.0, Color32::from_gray(200));
                let cell = 8.0 / ppp;
                let visible = painter.clip_rect().intersect(rect);
                let (mut y, mut row) = (
                    rect.min.y + ((visible.min.y - rect.min.y) / cell).floor() * cell,
                    ((visible.min.y - rect.min.y) / cell).floor() as i64,
                );
                while y < visible.max.y {
                    let (mut x, mut col) = (
                        rect.min.x + ((visible.min.x - rect.min.x) / cell).floor() * cell,
                        ((visible.min.x - rect.min.x) / cell).floor() as i64,
                    );
                    while x < visible.max.x {
                        if (row + col) % 2 == 0 {
                            painter.rect_filled(
                                Rect::from_min_size(Pos2::new(x, y), Vec2::splat(cell))
                                    .intersect(rect),
                                0.0,
                                Color32::from_gray(235),
                            );
                        }
                        x += cell;
                        col += 1;
                    }
                    y += cell;
                    row += 1;
                }
                if let Some(t) = &self.texture {
                    painter.image(
                        t.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                if self.pixel_grid {
                    self.draw_pixel_grid(&painter, rect, &view);
                }
                self.draw_overlays(&painter, &view, ppp);
                if let Some((x, y)) = self.pending_center.take() {
                    let center = rect.min
                        + Vec2::new(x * view.points_per_pixel(), y * view.points_per_pixel());
                    ui.scroll_to_rect(
                        Rect::from_center_size(center, Vec2::ZERO),
                        Some(egui::Align::Center),
                    );
                }
            });
    }

    /// Display-only feedback: the committed selection and the unpublished gesture.
    pub(super) fn draw_overlays(&self, painter: &egui::Painter, view: &View, ppp: f32) {
        let screen = |p: (i64, i64)| {
            let s = view.to_screen(p);
            Pos2::new(s.0, s.1)
        };
        let px = view.points_per_pixel();
        let box_of = |a: (i64, i64), b: (i64, i64)| {
            Rect::from_min_max(
                screen((a.0.min(b.0), a.1.min(b.1))),
                screen((a.0.max(b.0) + 1, a.1.max(b.1) + 1)),
            )
        };
        let ants = |r: Rect| {
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0 / ppp, Color32::BLACK),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(
                r.expand(1.0 / ppp),
                0.0,
                egui::Stroke::new(1.0 / ppp, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        };
        if let Selection::Rect(r) = self.editor.selection() {
            ants(box_of((r.x0, r.y0), (r.x1 - 1, r.y1 - 1)));
        }
        let color = |c: Rgba| Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3]);
        match self.editor.preview() {
            Some(Preview::Path { .. }) => {} // Rendered through the same staged pixels as publication.
            Some(Preview::Shape {
                tool,
                from,
                to,
                stroke,
                fill,
            }) => {
                let r = box_of(from, to);
                let line = stroke.map(|(c, size)| {
                    egui::Stroke::new((size as f32 * px).max(1.0 / ppp), color(c))
                });
                match tool {
                    Tool::Line => {
                        if let Some(l) = line {
                            let half = Vec2::splat(px / 2.0);
                            painter.line_segment([screen(from) + half, screen(to) + half], l);
                        }
                    }
                    Tool::Rect => {
                        if let Some(f) = fill {
                            painter.rect_filled(r, 0.0, color(f));
                        }
                        if let Some(l) = line {
                            painter.rect_stroke(r, 0.0, l, egui::StrokeKind::Inside);
                        }
                    }
                    _ => {
                        if let Some(f) = fill {
                            painter.add(egui::epaint::EllipseShape::filled(
                                r.center(),
                                r.size() / 2.0,
                                color(f),
                            ));
                        }
                        if let Some(l) = line {
                            painter.add(egui::epaint::EllipseShape::stroke(
                                r.center(),
                                r.size() / 2.0,
                                l,
                            ));
                        }
                    }
                }
            }
            Some(Preview::Marquee { from, to }) => ants(box_of(from, to)),
            Some(Preview::Text {
                at,
                text,
                scale,
                color: c,
            }) => {
                let canvas = paintcraft_core::types::Rect::new(
                    i64::MIN / 4,
                    i64::MIN / 4,
                    i64::MAX / 2,
                    i64::MAX / 2,
                )
                .expect("a huge clip");
                if let Ok(spans) =
                    paintcraft_raster::font::layout(&text, at, i64::from(scale), &canvas)
                {
                    for s in spans {
                        painter.rect_filled(
                            Rect::from_min_max(screen((s.x0, s.y)), screen((s.x1 + 1, s.y + 1))),
                            0.0,
                            color(c),
                        );
                    }
                }
                // A caret after the last typed character, and a frame around the box.
                let lines: Vec<&str> = text.split('\n').collect();
                let cols = lines.last().map_or(0, |l| l.chars().count()) as i64;
                let (cx, cy) = (
                    at.0 + cols * 8 * i64::from(scale),
                    at.1 + (lines.len() as i64 - 1) * 8 * i64::from(scale),
                );
                painter.rect_filled(
                    Rect::from_min_max(
                        screen((cx, cy)),
                        screen((cx + 1, cy + 8 * i64::from(scale))),
                    ),
                    0.0,
                    Color32::from_rgb(0, 95, 184),
                );
            }
            None => {}
        }
    }
}
