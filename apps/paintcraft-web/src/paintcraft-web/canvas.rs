//! Classic browser input and texture presentation; no native viewport services.
use crate::{app::WebApp, editor::MAX_POINTS};
use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use paintcraft_editor::session::Ticket;
use paintcraft_input::PointerButton;
use paintcraft_tools::{Preview, PreviewRevision, Tool};

#[derive(Default)]
pub(crate) struct Canvas {
    pub(crate) touch: crate::touch_navigation::TouchNavigation,
    pub(crate) texture: Option<TextureHandle>,
    pub(crate) pixel_grid: bool,
    pub(crate) outline: paintcraft_raster::selection::OutlineCache,
    pub(crate) bounds: std::cell::RefCell<paintcraft_raster::selection::BoundsCache>,
    fitted: bool,
    pub(crate) pending_center: Option<(f32, f32)>,
    key: Option<(Ticket, Option<PreviewRevision>)>,
    paste_revision: Option<PreviewRevision>,
    pub rect: Option<Rect>,
    pub viewport: Vec2,
    pub pointer: Option<(i64, i64)>,
    pub controls: Option<Rect>,
}

impl Canvas {
    pub(crate) fn invalidate(&mut self) {
        self.key = None;
        self.paste_revision = None;
    }
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl WebApp {
    pub(crate) fn fit_selection(&mut self) {
        self.editor.cancel_floating();
        let Ok(Some(bounds)) = self.selected_bounds() else {
            return;
        };
        self.editor.tools.cancel_gesture();
        let clearance = if self.tool_controls_visible() {
            2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
        } else {
            0.0
        };
        let size = (self.canvas.viewport - Vec2::new(clearance + 24.0, 24.0)).max(Vec2::splat(1.0));
        self.zoom = (size.x / bounds.width() as f32)
            .min(size.y / bounds.height() as f32)
            .clamp(0.125, 10.0);
        self.canvas.pending_center = Some((
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ));
    }
    pub(crate) fn fit_canvas(&mut self) {
        let (w, h) = self.editor.session.document().canvas;
        let clearance = if self.tool_controls_visible() {
            2.0 * paintcraft_classic::tool_controls::CANVAS_CLEARANCE
        } else {
            0.0
        };
        let size = (self.canvas.viewport - Vec2::new(clearance + 24.0, 24.0)).max(Vec2::splat(1.0));
        self.zoom = (size.x / w as f32)
            .min(size.y / h as f32)
            .clamp(0.125, 10.0);
    }

    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui) {
        let mut viewport = ui.available_rect_before_wrap();
        let rulers =
            self.frontend == paintcraft_ui::actions::Frontend::Classic && self.rulers_visible();
        if rulers {
            viewport.min =
                (viewport.min + Vec2::splat(paintcraft_classic::RULER_SIZE)).min(viewport.max);
        }
        self.canvas.viewport = viewport.size();
        let modern = self.tool_controls_visible();
        let enabled = !self.blocked();
        self.canvas.controls = if modern {
            paintcraft_classic::tool_controls::show(
                ui,
                egui::Id::unique("web-tool-options"),
                viewport,
                self.controls.tool,
                &mut self.controls.options,
                enabled,
            )
        } else {
            None
        };
        let clearance = if modern {
            paintcraft_classic::tool_controls::CANVAS_CLEARANCE
        } else {
            0.0
        };
        if !self.canvas.fitted && ui.ctx().current_pass_index() == 0 {
            self.canvas.fitted = true;
            self.fit_canvas();
        }
        let pointer_on_workspace = ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| {
            ui.clip_rect().contains(pos)
                && self.canvas.controls.is_none_or(|rect| !rect.contains(pos))
                && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
        });
        if ui.ctx().current_pass_index() == 0 && !self.blocked() {
            let scroll = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|event| {
                        if let egui::Event::MouseWheel {
                            unit,
                            delta,
                            modifiers,
                            ..
                        } = event
                            && (modifiers.ctrl || modifiers.command || modifiers.mac_cmd)
                            && !modifiers.alt
                            && pointer_on_workspace
                        {
                            Some(match unit {
                                egui::MouseWheelUnit::Point => delta.y / 40.0,
                                egui::MouseWheelUnit::Line => delta.y,
                                egui::MouseWheelUnit::Page => delta.y * 3.0,
                            })
                        } else {
                            None
                        }
                    })
                    .filter(|delta| delta.is_finite())
                    .sum::<f32>()
            });
            if scroll != 0.0 {
                self.editor.cancel_floating();
                self.zoom =
                    (self.zoom * 1.1_f32.powf(scroll.clamp(-32.0, 32.0))).clamp(0.125, 10.0);
            }
        }
        let extent = self.editor.session.document().canvas;
        let scope = self.studio_canvas_scope();
        let studio = self.frontend == paintcraft_ui::actions::Frontend::Studio;
        let size = Vec2::new(extent.0 as f32, extent.1 as f32) * self.zoom;
        let paint = |ui: &mut egui::Ui, response: egui::Response, painter: egui::Painter| {
            self.canvas.rect = Some(response.rect);
            self.pen.region = Some((
                ui.clip_rect().intersect(viewport).intersect(response.rect),
                ui.layer_id(),
            ));
            self.canvas
                .touch
                .viewport(ui.clip_rect().intersect(viewport), ui.layer_id());
            if let Some((x, y)) = self.canvas.pending_center.take() {
                let center = response.rect.min + Vec2::new(x, y) * self.zoom;
                ui.scroll_to_rect(
                    Rect::from_center_size(center, Vec2::ZERO),
                    Some(egui::Align::Center),
                );
            }
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "Canvas"));
            if ui.ctx().current_pass_index() == 0 {
                self.canvas_input(ui, &response);
            }
            self.refresh_texture(ui.ctx());
            painter.rect_filled(response.rect, 0, Color32::WHITE);
            if let Some(texture) = &self.canvas.texture {
                painter.image(
                    texture.id(),
                    response.rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            if self.canvas.pixel_grid {
                paintcraft_ui::view::pixel_grid(
                    &painter,
                    response.rect,
                    self.editor.session.document().canvas,
                    self.zoom,
                    ui.ctx().pixels_per_point(),
                );
            }
            self.text_overlay(ui, &painter, response.rect);
            self.draw_mask_overlay(&painter, response.rect);
            if let Some(Preview::SelectionPath { points }) = self.editor.tools.preview() {
                let path: Vec<_> = points
                    .into_iter()
                    .map(|p| response.rect.min + Vec2::new(p.0 as f32, p.1 as f32) * self.zoom)
                    .collect();
                let ppp = ui.ctx().pixels_per_point();
                painter.add(egui::Shape::closed_line(
                    path.clone(),
                    egui::Stroke::new(3.0 / ppp, Color32::WHITE),
                ));
                painter.add(egui::Shape::closed_line(
                    path,
                    egui::Stroke::new(1.0 / ppp, Color32::BLACK),
                ));
            }
            let selection = match self.editor.tools.preview() {
                Some(Preview::Marquee { from, to }) => Some((from, (to.0 + 1, to.1 + 1))),
                _ => match self
                    .visible_selection()
                    .unwrap_or(paintcraft_raster::Selection::Empty)
                {
                    paintcraft_editor::execute::Selection::Rect(r) => {
                        Some(((r.x0, r.y0), (r.x1, r.y1)))
                    }
                    _ => None,
                },
            };
            if let Some((a, b)) = selection {
                let rect = Rect::from_two_pos(
                    response.rect.min + Vec2::new(a.0 as f32, a.1 as f32) * self.zoom,
                    response.rect.min + Vec2::new(b.0 as f32, b.1 as f32) * self.zoom,
                );
                painter.rect_stroke(
                    rect,
                    0,
                    egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215)),
                    egui::StrokeKind::Inside,
                );
            }
            if response.hovered()
                && !self.blocked()
                && ui
                    .ctx()
                    .pointer_hover_pos()
                    .is_some_and(|pos| self.canvas.controls.is_none_or(|rect| !rect.contains(pos)))
            {
                ui.ctx()
                    .set_cursor_icon(if self.controls.tool == Tool::Text {
                        egui::CursorIcon::Text
                    } else if self.controls.tool == Tool::Magnifier {
                        egui::CursorIcon::ZoomIn
                    } else if self
                        .canvas
                        .pointer
                        .is_some_and(|point| self.selection_move_cursor(point))
                    {
                        egui::CursorIcon::Move
                    } else {
                        egui::CursorIcon::Crosshair
                    });
                if self.controls.tool == Tool::Eraser
                    && let Some(pos) = ui.ctx().pointer_hover_pos()
                {
                    let rect = Rect::from_center_size(
                        pos,
                        Vec2::splat(self.controls.options.size as f32 * self.zoom),
                    );
                    painter.rect_stroke(
                        rect,
                        0,
                        egui::Stroke::new(2.0, Color32::WHITE),
                        egui::StrokeKind::Middle,
                    );
                    painter.rect_stroke(
                        rect,
                        0,
                        egui::Stroke::new(1.0, Color32::BLACK),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        };
        if studio {
            paintcraft_studio::canvas::canvas_for_scope(
                ui,
                egui::Id::unique("web-studio-canvas"),
                scope,
                size,
                paint,
            );
        } else if rulers {
            paintcraft_classic::canvas::ruled_canvas_for_scope(
                ui,
                egui::Id::unique("web-editor-canvas"),
                scope,
                size,
                extent,
                clearance,
                paint,
            );
        } else {
            paintcraft_classic::canvas::canvas_for_scope(
                ui,
                egui::Id::unique("web-editor-canvas"),
                scope,
                size,
                clearance,
                paint,
            );
        }
    }

    fn refresh_texture(&mut self, ctx: &egui::Context) {
        self.editor.validate_floating();
        self.validate_paste_placement(ctx);
        let paste_revision = self.paste_revision();
        let key = (
            self.editor.session.ticket(),
            (!self.editor.tools.gesture_is_selection())
                .then(|| self.editor.tools.preview_revision()),
        );
        if self.canvas.key == Some(key) && self.canvas.paste_revision == paste_revision {
            return;
        }
        let result = (if self.paste.is_placing() {
            self.paste_preview_document()
        } else {
            self.editor.display_document()
        })
        .and_then(|preview| {
            paintcraft_render::render_flat(
                preview.as_ref().unwrap_or(self.editor.session.document()),
            )
        });
        let pixels = match result {
            Ok(pixels) => pixels,
            Err(error) => {
                // Never leave a failed speculative image presented as committed.
                self.editor.cancel_floating();
                self.editor.tools.cancel_gesture();
                self.message = if error.message.starts_with("Sampled brush stroke")
                    || error.message.starts_with("Custom shape size")
                    || error.message == "The selected custom shape is no longer available"
                {
                    paintcraft_ui::t!(ctx, &error.message)
                } else {
                    format!("Preview unavailable: {error}")
                };
                match paintcraft_render::render_flat(self.editor.session.document()) {
                    Ok(pixels) => pixels,
                    Err(error) => {
                        self.canvas.texture = None;
                        self.message = error.to_string();
                        return;
                    }
                }
            }
        };
        let (width, height) = self.editor.session.document().canvas;
        let image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);
        if let Some(texture) = &mut self.canvas.texture {
            texture.set(image, TextureOptions::NEAREST);
        } else {
            self.canvas.texture =
                Some(ctx.load_texture("web-document", image, TextureOptions::NEAREST));
        }
        self.canvas.paste_revision = paste_revision;
        self.canvas.key = Some((
            self.editor.session.ticket(),
            (!self.editor.tools.gesture_is_selection())
                .then(|| self.editor.tools.preview_revision()),
        ));
    }

    fn canvas_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if self.canvas.touch.suppress && !self.pen_canvas_owned() {
            return;
        }
        if self.paste.is_placing() {
            self.paste_canvas_input(ui, response);
            return;
        }
        if self.blocked() {
            self.editor.cancel_floating();
            self.editor.tools.cancel_gesture();
            return;
        }
        if self.navigate_canvas(ui, response) {
            return;
        }
        if self.frontend == paintcraft_ui::actions::Frontend::Studio
            && self.controls.tool == Tool::Pan
        {
            if response.dragged_by(egui::PointerButton::Primary) {
                let delta = ui.input(|input| input.pointer.delta());
                paintcraft_studio::canvas::pan_by(
                    ui.ctx(),
                    egui::Id::unique("web-studio-canvas"),
                    self.studio_canvas_scope(),
                    delta,
                );
                ui.ctx().request_repaint();
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            return;
        }
        self.editor.tools.activate_tool(self.controls.tool);
        self.editor.validate_floating();
        self.editor.tools.color = self.controls.primary;
        self.editor.tools.secondary = self.controls.secondary;
        self.editor.tools.options = self.controls.options;
        if self.controls.tool == Tool::ColorPicker {
            use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
            self.editor.tools.options.picker_source = SampleSource::Image;
            self.editor.tools.options.picker_size = SampleSize::One;
            self.editor.tools.options.picker_after = PickerAfter::Previous;
        }
        self.editor.tools.options.eraser_color =
            if self.frontend == paintcraft_ui::actions::Frontend::Studio {
                paintcraft_core::types::Rgba([0; 4])
            } else {
                paintcraft_tools::commands::canvas_color(self.editor.session.document())
            };
        let rect = response.rect;
        let zoom = self.zoom;
        let point = |pos: Pos2| {
            let p = (pos - rect.min) / zoom;
            ((p.x * 256.0).floor() as i64, (p.y * 256.0).floor() as i64)
        };
        let legacy = self.canvas_events(ui.ctx());
        let events = self.pen_canvas_events(legacy, ui.ctx());
        if !ui.input(|i| i.focused) {
            self.editor.cancel_floating();
            self.editor.tools.cancel_gesture();
            return;
        }
        self.editor
            .tools
            .constrain_shape(ui.input(|i| i.modifiers.shift));
        for event in events {
            let (event, mouse_sequence) = match event {
                crate::pen_input::CanvasEvent::Legacy(event) => (event, None),
                crate::pen_input::CanvasEvent::Mouse { sequence, event } => (event, Some(sequence)),
                crate::pen_input::CanvasEvent::Pen(index) => {
                    let Some(event) = self.pen_canvas_sample(index, ui.ctx()) else {
                        continue;
                    };
                    (event, self.pen_sample_sequence(index))
                }
            };
            if self.keyboard_event(ui.ctx(), &event) {
                continue;
            }
            match event {
                egui::Event::WindowFocused(false) => {
                    self.editor.cancel_floating();
                    self.editor.tools.cancel_gesture();
                    self.canvas.pointer = None;
                    break;
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } if self.editor.tools.is_constructing_shape()
                    && !self.text.widget_owned_input
                    && !ui.ctx().egui_wants_keyboard_input()
                    && !egui::Popup::is_any_open(ui.ctx()) =>
                {
                    match key {
                        egui::Key::Enter => {
                            if let Some((_, action)) = self.editor.tools.finish_shape() {
                                self.edit(action);
                            }
                        }
                        egui::Key::Backspace => self.editor.tools.remove_shape_vertex(),
                        egui::Key::Escape => {
                            self.cancel_clipboard_cut(ui.ctx());
                            self.cancel_paste(ui.ctx());
                            self.editor.tools.cancel_gesture();
                        }
                        _ => {}
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    let button = match button {
                        egui::PointerButton::Primary => PointerButton::Primary,
                        egui::PointerButton::Secondary => PointerButton::Secondary,
                        _ => continue,
                    };
                    if pressed
                        && rect.contains(pos)
                        && ui.clip_rect().contains(pos)
                        && self.canvas.controls.is_none_or(|rect| !rect.contains(pos))
                        && (response.contains_pointer()
                            || (self.canvas.touch.delivered_tap == Some(pos)
                                && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())))
                    {
                        if self.controls.tool == Tool::Magnifier {
                            let p = point(pos);
                            self.zoom = paintcraft_ui::actions::stepped_zoom(
                                self.zoom,
                                button == PointerButton::Primary,
                            )
                            .clamp(0.125, 10.0);
                            self.canvas.pending_center =
                                Some((p.0 as f32 / 256.0, p.1 as f32 / 256.0));
                            ui.ctx().request_repaint();
                            continue;
                        }
                        let layer = self.editor.target.unwrap_or(uuid::Uuid::nil());
                        if (self.controls.tool == Tool::Rect
                            && self.editor.tools.resource_shape.is_some()
                            && self.editor.session.document().layers.is_empty()
                            && self.editor.session.document().roots.is_empty())
                            || (self.controls.tool == Tool::Brush
                                && self.editor.tools.sampled_tip.is_some()
                                && self.editor.session.document().layers.is_empty())
                            || matches!(self.controls.tool, Tool::Select | Tool::ColorPicker)
                            || self
                                .editor
                                .session
                                .document()
                                .layers
                                .get(&layer)
                                .is_some_and(|layer| layer.visible)
                        {
                            if self.controls.tool == Tool::Text {
                                let p = point(pos);
                                let doc_point = (p.0.div_euclid(256), p.1.div_euclid(256));
                                let inside_draft = match self.editor.tools.preview() {
                                    Some(Preview::Text {
                                        style: Some(style),
                                        text,
                                        at,
                                        ..
                                    }) => self
                                        .editor
                                        .text_mask(&text, &style, at)
                                        .ok()
                                        .and_then(|mask| mask.bounds)
                                        .is_some_and(|bounds| {
                                            doc_point.0 >= bounds.x0
                                                && doc_point.0 < bounds.x1
                                                && doc_point.1 >= bounds.y0
                                                && doc_point.1 < bounds.y1
                                        }),
                                    _ => false,
                                };
                                if button == PointerButton::Primary
                                    && !inside_draft
                                    && self.finish_text()
                                {
                                    let p = point(pos);
                                    self.sync_text();
                                    self.text.selected_all = false;
                                    self.editor.tools.begin_text(
                                        layer,
                                        (p.0.div_euclid(256), p.1.div_euclid(256)),
                                    );
                                }
                            } else if !self.begin_selection_float(point(pos), button) {
                                self.editor
                                    .tools
                                    .pointer_down_subpixel(layer, point(pos), button);
                            }
                            if self.editor.tools.pointer_is_down() {
                                self.pen.legacy_canvas_owned = true;
                            }
                            self.editor.tools.constrain_shape(modifiers.shift);
                        }
                    } else if !pressed && self.editor.tools.gesture_button() == Some(button) {
                        self.move_selection_pointer(point(pos));
                        self.editor.tools.constrain_shape(modifiers.shift);
                        if let Some((_, action)) = self.editor.tools.pointer_up_button(button) {
                            let before = self.pen_predecessor_owner();
                            self.editor.take_publication_witness();
                            self.edit(action);
                            let receipt = self.editor.take_publication_witness();
                            if let Some(sequence) = mouse_sequence {
                                self.pen_contact_predecessor(before, sequence, receipt, ui.ctx());
                            }
                        }
                        self.pen.legacy_canvas_owned = false;
                        if let Some(hint) = self.editor.tools.shape_hint() {
                            self.message = hint.into();
                        }
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    let p = point(pos);
                    self.canvas.pointer = (rect.contains(pos)
                        && ui.clip_rect().contains(pos)
                        && self.canvas.controls.is_none_or(|rect| !rect.contains(pos)))
                    .then_some((p.0.div_euclid(256), p.1.div_euclid(256)));
                    if self.editor.tools.is_constructing_shape()
                        && !self.editor.tools.pointer_is_down()
                        && self.canvas.pointer.is_none()
                    {
                        continue;
                    }
                    if self.editor.tools.gesture_is_lasso() {
                        self.move_selection_pointer(p);
                    } else if self.editor.tools.gesture_points().len() >= MAX_POINTS {
                        self.editor.tools.cancel_gesture();
                        self.message =
                            "Stroke cancelled: browser point limit reached; use shorter strokes"
                                .into();
                    } else {
                        self.editor.tools.pointer_move_subpixel(p);
                    }
                }
                egui::Event::PointerGone => {
                    self.editor.cancel_floating();
                    self.editor.tools.cancel_gesture();
                    self.canvas.pointer = None;
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } if !self.text.widget_owned_input
                    && !ui.ctx().egui_wants_keyboard_input()
                    && !egui::Popup::is_any_open(ui.ctx()) =>
                {
                    self.editor.cancel();
                    self.text.preedit.clear();
                }
                event => self.text_event(ui.ctx(), event),
            }
        }
    }
}
