//! Desktop canvas integration.

use super::*;

impl App {
    pub(super) fn tool_controls_visible(&self) -> bool {
        self.experience() == Experience::Classic
            && self.variant() == ClassicVariant::V11
            && self.settings.classic_tool_controls
    }
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui, first_pass: bool) {
        let mut viewport = ui.available_rect_before_wrap();
        if self.experience() == Experience::Classic && self.settings.classic_rulers {
            viewport.min =
                (viewport.min + Vec2::splat(paintcraft_classic::RULER_SIZE)).min(viewport.max);
        }
        let mut options = self.editor.options;
        let controls = if self.tool_controls_visible() {
            paintcraft_classic::tool_controls::show(
                ui,
                egui::Id::unique(self.active_tab_id()),
                viewport,
                self.editor.tool,
                &mut options,
                !self.action_context().blocked,
            )
        } else {
            None
        };
        self.editor.options = options;
        let left_clearance = if self.tool_controls_visible() {
            paintcraft_classic::tool_controls::CANVAS_CLEARANCE
        } else {
            0.0
        };
        self.canvas_viewport = Some(viewport);
        if first_pass && self.initial_fit {
            self.initial_fit = false;
            if self.experience() == Experience::Classic {
                self.dispatch_view_action(ui.ctx(), paintcraft_ui::actions::ViewAction::FitWindow);
            }
        }
        let ppp = ui.ctx().pixels_per_point();
        let (w, h) = self.editor.document().canvas;
        let size = self.canvas_view((0.0, 0.0), ppp).canvas_size(w, h);
        let id = egui::Id::unique(self.active_tab_id());
        let canvas_size = Vec2::new(size.0, size.1);
        match self.experience() {
            Experience::Classic if self.settings.classic_rulers => {
                paintcraft_classic::canvas::ruled_canvas(
                    ui,
                    id,
                    canvas_size,
                    (w, h),
                    left_clearance,
                    |ui, response, painter| {
                        self.paint_canvas(ui, response, painter, first_pass, controls)
                    },
                )
            }
            Experience::Classic => paintcraft_classic::canvas::canvas(
                ui,
                id,
                canvas_size,
                left_clearance,
                |ui, response, painter| {
                    self.paint_canvas(ui, response, painter, first_pass, controls)
                },
            ),
            Experience::Studio => paintcraft_studio::canvas::canvas_for_profile(
                ui,
                id,
                self.studio_pan_scope(ui.ctx()),
                canvas_size,
                self.settings.studio_overscroll,
                |ui, response, painter| {
                    self.paint_canvas(ui, response, painter, first_pass, controls)
                },
            ),
        }
    }

    fn paint_canvas(
        &mut self,
        ui: &mut egui::Ui,
        response: egui::Response,
        painter: egui::Painter,
        first_pass: bool,
        controls: Option<egui::Rect>,
    ) {
        // Disabled chrome must not tint texture vertices: the modal owns the scrim.
        let painter = egui::Painter::new(ui.ctx().clone(), painter.layer_id(), painter.clip_rect());
        let ppp = ui.ctx().pixels_per_point();
        let rect = response.rect;
        let pending_center = self.pending_center.take();
        self.canvas_rect = Some(rect);
        // The scroll area reserves edge chrome outside its clipped content. Keep
        // viewport metrics and zoom centers tied to that actual drawable region.
        let viewport = ui
            .clip_rect()
            .intersect(self.canvas_viewport.unwrap_or(ui.clip_rect()));
        self.canvas_viewport = Some(viewport);
        let visible_canvas = rect.intersect(viewport);
        let view = self.canvas_view((rect.min.x, rect.min.y), ppp);
        self.hover_doc = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| visible_canvas.contains(*p) && controls.is_none_or(|r| !r.contains(*p)))
            .and_then(|p| view.to_doc((p.x, p.y)));

        self.validate_studio_pan(ui, viewport);
        if first_pass {
            #[cfg(target_os = "windows")]
            self.framework_force_validate(ui, view);
            let events = ui.input(|i| i.events.clone());
            #[cfg(target_os = "macos")]
            let events = {
                let mut events = events;
                self.tablet_canvas_input(&mut events, first_pass, painter.layer_id(), controls);
                events
            };
            for event in events {
                #[cfg(target_os = "windows")]
                if self.framework_force_event(
                    ui,
                    painter.layer_id(),
                    visible_canvas,
                    controls,
                    view,
                    &event,
                ) {
                    continue;
                }

                if self.canvas_text_event(ui.ctx(), &event) {
                    continue;
                }
                if self.studio_pan_event(ui, &painter, viewport, &event)
                    || self.studio_zoom_event(ui, &painter, rect, viewport, &event)
                {
                    continue;
                }
                match event {
                    egui::Event::WindowFocused(false) if self.editor.is_constructing_shape() => {
                        self.editor.cancel_gesture();
                        break;
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        ..
                    } if self.editor.is_constructing_shape()
                        && !ui.ctx().egui_wants_keyboard_input()
                        && !egui::Popup::is_any_open(ui.ctx()) =>
                    {
                        match key {
                            egui::Key::Enter => self.editor.finish_shape(),
                            egui::Key::Backspace => self.editor.remove_shape_vertex(),
                            egui::Key::Escape => self.editor.cancel_gesture(),
                            _ => {}
                        }
                    }
                    egui::Event::PointerGone if self.editor.is_constructing_shape() => {
                        self.editor.cancel_gesture()
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: true,
                        modifiers,
                    } => {
                        let button = match button {
                            egui::PointerButton::Primary => {
                                paintcraft_input::PointerButton::Primary
                            }
                            egui::PointerButton::Secondary
                                if self.experience() == Experience::Studio
                                    || matches!(
                                        self.editor.tool,
                                        Tool::ColorPicker | Tool::Magnifier
                                    ) =>
                            {
                                paintcraft_input::PointerButton::Secondary
                            }
                            _ => continue,
                        };
                        if (visible_canvas.contains(pos)
                            && controls.is_none_or(|r| !r.contains(pos))
                            && ui.ctx().layer_id_at(pos) == Some(painter.layer_id()))
                            && let Some(d) = view.to_doc_subpixel((pos.x, pos.y))
                        {
                            if self.editor.tool == Tool::Magnifier {
                                let zoom = paintcraft_ui::actions::stepped_zoom(
                                    self.zoom,
                                    button == paintcraft_input::PointerButton::Primary,
                                );
                                self.set_zoom(
                                    ui.ctx(),
                                    zoom,
                                    (d.0 as f32 / 256.0, d.1 as f32 / 256.0),
                                );
                                continue;
                            }
                            // Classic restores the solid canvas ground; Studio removes
                            // pixels. Capture this policy with the rest of the gesture.
                            self.editor.options.eraser_color = match self.experience() {
                                Experience::Classic => self.editor.canvas_color(),
                                Experience::Studio => Rgba::TRANSPARENT,
                            };
                            let source = self.editor.options.picker_source;
                            if self.editor.tool == Tool::ColorPicker {
                                use paintcraft_core::types::picker::{
                                    PickerAfter, SampleSize, SampleSource,
                                };
                                if self.experience() == Experience::Classic {
                                    self.editor.options.picker_source = SampleSource::Image;
                                    self.editor.options.picker_size = SampleSize::One;
                                    self.editor.options.picker_after = PickerAfter::Previous;
                                } else if modifiers.ctrl {
                                    self.editor.options.picker_source = SampleSource::Image;
                                }
                            }
                            let was_down = self.editor.pointer_is_down();
                            self.editor.pointer_down_subpixel(d, button);
                            self.editor.options.picker_source = source;
                            self.editor.constrain_shape(modifiers.shift);
                            if !was_down && self.editor.pointer_is_down() {
                                self.begin_studio_auto_scroll(ui, rect, viewport);
                            }
                        }
                    }
                    egui::Event::ModifiersChanged(modifiers) => {
                        self.editor.constrain_shape(modifiers.shift);
                    }
                    egui::Event::PointerMoved(pos) => {
                        if self.editor.is_constructing_shape()
                            && !self.editor.pointer_is_down()
                            && (!visible_canvas.contains(pos)
                                || controls.is_some_and(|r| r.contains(pos)))
                        {
                            continue;
                        }
                        if let Some(d) = view.to_doc_subpixel((pos.x, pos.y)) {
                            self.editor.pointer_move_subpixel(d);
                        }
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: false,
                        modifiers,
                    } => {
                        let button = match button {
                            egui::PointerButton::Primary => {
                                paintcraft_input::PointerButton::Primary
                            }
                            egui::PointerButton::Secondary
                                if self.experience() == Experience::Studio
                                    || matches!(
                                        self.editor.tool,
                                        Tool::ColorPicker | Tool::Magnifier
                                    ) =>
                            {
                                paintcraft_input::PointerButton::Secondary
                            }
                            _ => continue,
                        };
                        if self.editor.gesture_button() != Some(button) {
                            continue;
                        }
                        if let Some(d) = view.to_doc_subpixel((pos.x, pos.y)) {
                            self.editor.pointer_move_subpixel(d);
                        }
                        self.editor.constrain_shape(modifiers.shift);
                        self.editor.pointer_up_button(button);
                    }
                    _ => {}
                }
            }
            self.advance_studio_auto_scroll(ui, &painter, rect, viewport);
        }

        // Shift may change without a pointer event. Apply it before preview or release.
        self.editor.constrain_shape(ui.input(|i| i.modifiers.shift));
        if let Some(region) = view.visible_region(
            (visible_canvas.min.x, visible_canvas.min.y),
            (visible_canvas.max.x, visible_canvas.max.y),
            self.editor.document().canvas,
        ) {
            // A small image is cheaper to keep as one reusable texture than to
            // continually composite differently clipped/resampled views on zoom.
            let small = self.experience() == Experience::Classic
                && u64::from(self.editor.document().canvas.0)
                    * u64::from(self.editor.document().canvas.1)
                    <= 1_048_576
                && self.editor.document().canvas.0 <= 2048
                && self.editor.document().canvas.1 <= 2048
                // Magnified live painting must keep the smaller visible region.
                && !(view.zoom > 1.0
                    && (self.editor.gesture_in_progress()
                        || self.editor.is_editing_text()
                        || self.editor.has_pending_edits()));
            let region = if small {
                self.editor.document().canvas_rect()
            } else {
                region
            };
            let level = if small { 0 } else { self.canvas_mip_level(ppp) };
            match paintcraft_render::display::DisplayRequest::new(
                self.editor.document(),
                region,
                level,
            ) {
                Ok(request) => {
                    self.render_region = Some(request.region());
                    self.render_level = request.level();
                    self.sync_texture(ui.ctx());
                }
                Err(error) => self
                    .editor
                    .set_status(format!("Cannot render viewport: {error}")),
            }
        }

        // Transparency checkerboard, then the composite. Display only: the document is untouched.
        painter.rect_filled(
            rect,
            0.0,
            if ui.visuals().dark_mode {
                Color32::from_gray(85)
            } else {
                Color32::from_gray(200)
            },
        );
        let cell = 8.0 / ppp;
        let visible = painter.clip_rect().intersect(rect);
        let (mut y, mut row) = (
            rect.min.y + ((visible.min.y - rect.min.y) / cell).floor() * cell,
            ((visible.min.y - rect.min.y) / cell).floor() as i64,
        );
        let mut checker = egui::Mesh::default();
        while y < visible.max.y {
            let (mut x, mut col) = (
                rect.min.x + ((visible.min.x - rect.min.x) / cell).floor() * cell,
                ((visible.min.x - rect.min.x) / cell).floor() as i64,
            );
            while x < visible.max.x {
                if (row + col) % 2 == 0 {
                    checker.add_colored_rect(
                        Rect::from_min_size(Pos2::new(x, y), Vec2::splat(cell)).intersect(rect),
                        if ui.visuals().dark_mode {
                            Color32::from_gray(105)
                        } else {
                            Color32::from_gray(235)
                        },
                    );
                }
                x += cell;
                col += 1;
            }
            y += cell;
            row += 1;
        }
        painter.add(checker);
        if let Some(id) = self
            .gpu_texture
            .as_ref()
            .map(|texture| texture.id)
            .or_else(|| self.texture.as_ref().map(egui::TextureHandle::id))
        {
            painter
                .with_clip_rect(painter.clip_rect().intersect(rect))
                .image(
                    id,
                    self.texture_key.map_or(rect, |key| {
                        let region = key.3;
                        let min = view.to_screen((region.x0, region.y0));
                        let [w, h] = Self::display_size(key);
                        let step = 1i64 << key.4;
                        let max = view.to_screen((
                            region.x0 + i64::from(w) * step,
                            region.y0 + i64::from(h) * step,
                        ));
                        Rect::from_min_max(Pos2::new(min.0, min.1), Pos2::new(max.0, max.1))
                    }),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
        }

        if self.pixel_grid {
            self.draw_pixel_grid(&painter, rect, &view);
        }
        self.draw_overlays(&painter, &view, ppp);
        self.draw_tool_cursor(ui, &painter, visible_canvas, &view, ppp);
        self.draw_studio_zoom(ui, &painter, rect, viewport);
        if let Some((x, y)) = pending_center {
            let center =
                rect.min + Vec2::new(x * view.points_per_pixel(), y * view.points_per_pixel());
            ui.scroll_to_rect(
                Rect::from_center_size(center, Vec2::ZERO),
                Some(egui::Align::Center),
            );
        }
    }

    /// Display-only feedback: the committed selection and the unpublished gesture.
    pub(super) fn draw_overlays(&mut self, painter: &egui::Painter, view: &View, ppp: f32) {
        let screen = |p: (i64, i64)| {
            let s = view.to_screen(p);
            Pos2::new(s.0, s.1)
        };
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
        self.draw_mask_overlay(painter, view, ppp);
        if let Selection::Rect(r) = self.editor.display_selection() {
            ants(box_of((r.x0, r.y0), (r.x1 - 1, r.y1 - 1)));
        }
        let snapshot = self.canvas_text_preview();
        let overlay = snapshot.as_deref().filter(|preview| {
            !matches!(
                preview,
                Preview::ResourceShape(_)
                    | Preview::SampledStroke(_)
                    | Preview::PenStroke(_)
                    | Preview::Path(_)
                    | Preview::Paths(_)
                    | Preview::Shape { .. }
                    | Preview::Contour(_)
                    | Preview::MoveSelection { .. }
            )
        });
        match overlay.cloned() {
            Some(
                Preview::ResourceShape(_)
                | Preview::SampledStroke(_)
                | Preview::PenStroke(_)
                | Preview::Path(_)
                | Preview::Paths(_)
                | Preview::Shape { .. }
                | Preview::Contour(_)
                | Preview::MoveSelection { .. },
            ) => {} // Staged raster pixels, not vector approximations.
            Some(Preview::SelectionPath { points }) => {
                let path: Vec<_> = points.into_iter().map(screen).collect();
                painter.add(egui::Shape::closed_line(
                    path.clone(),
                    egui::Stroke::new(3.0 / ppp, Color32::WHITE),
                ));
                painter.add(egui::Shape::closed_line(
                    path,
                    egui::Stroke::new(1.0 / ppp, Color32::BLACK),
                ));
            }
            Some(Preview::Marquee { from, to }) => ants(box_of(from, to)),
            Some(Preview::Text {
                style: Some(style),
                at,
                ref text,
                ..
            }) => {
                let mask = if self.renderer.is_some() {
                    self.editor.cached_font_text_mask(text, &style, at)
                } else {
                    self.editor.font_text_mask(text, &style, at).ok()
                };
                if let Some(mask) = mask {
                    let (x, y, height) = mask.caret;
                    if self.canvas_ime.selected_all
                        && let Some(bounds) = mask.bounds
                    {
                        painter.rect_filled(
                            Rect::from_min_max(
                                screen((bounds.x0, bounds.y0)),
                                screen((bounds.x1, bounds.y1)),
                            ),
                            0.0,
                            painter
                                .ctx()
                                .style_of(painter.ctx().theme())
                                .visuals
                                .selection
                                .bg_fill
                                .gamma_multiply(0.25),
                        );
                    }
                    if !painter.ctx().egui_wants_keyboard_input()
                        && !egui::Popup::is_any_open(painter.ctx())
                        && !self.action_context().blocked
                    {
                        let cursor_rect =
                            Rect::from_min_max(screen((x, y)), screen((x + 1, y + height)));
                        if painter.clip_rect().intersects(cursor_rect) {
                            let interrupt = std::mem::take(&mut self.canvas_ime.interrupt);
                            painter.ctx().output_mut(|output| {
                                output.ime = Some(egui::output::IMEOutput {
                                    purpose: egui::IMEPurpose::Normal,
                                    rect: self
                                        .canvas_rect
                                        .unwrap_or(cursor_rect)
                                        .intersect(painter.clip_rect()),
                                    cursor_rect,
                                    should_interrupt_composition: interrupt,
                                })
                            });
                        }
                    }
                    painter.line_segment(
                        [screen((x, y)), screen((x, y + height))],
                        egui::Stroke::new(
                            1.0 / ppp,
                            painter
                                .ctx()
                                .style_of(painter.ctx().theme())
                                .visuals
                                .selection
                                .stroke
                                .color,
                        ),
                    );
                }
            }
            Some(Preview::Text {
                layer: _,
                style: None,
                background: _,
                at,
                text,
                scale,
                color: _,
            }) => {
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
