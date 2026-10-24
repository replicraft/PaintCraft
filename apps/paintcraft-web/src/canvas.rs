//! Classic browser input and texture presentation; no native viewport services.
use crate::{app::WebApp, editor::MAX_POINTS};
use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use paintcraft_editor::session::Ticket;
use paintcraft_input::PointerButton;
use paintcraft_tools::{Preview, PreviewRevision, Tool};

#[derive(Default)]
pub(crate) struct Canvas {
    texture: Option<TextureHandle>,
    key: Option<(Ticket, PreviewRevision)>,
    pub rect: Option<Rect>,
    pub viewport: Vec2,
    pub pointer: Option<(i64, i64)>,
    pub controls: Option<Rect>,
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl WebApp {
    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui) {
        self.canvas.viewport = ui.available_size();
        let modern = self.variant == paintcraft_classic::ClassicVariant::V11;
        let enabled = !self.blocked();
        self.canvas.controls = if modern {
            paintcraft_classic::tool_controls::show(
                ui,
                egui::Id::unique("web-tool-options"),
                ui.available_rect_before_wrap(),
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
                self.zoom =
                    (self.zoom * 1.1_f32.powf(scroll.clamp(-32.0, 32.0))).clamp(0.125, 10.0);
            }
        }
        let extent = self.editor.session.document().canvas;
        paintcraft_classic::canvas::canvas(
            ui,
            egui::Id::unique("web-editor-canvas"),
            Vec2::new(extent.0 as f32, extent.1 as f32) * self.zoom,
            clearance,
            |ui, response, painter| {
                self.canvas.rect = Some(response.rect);
                response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, "Canvas"));
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
                self.text_overlay(ui, &painter, response.rect);
                let selection = match self.editor.tools.preview() {
                    Some(Preview::Marquee { from, to }) => Some((from, (to.0 + 1, to.1 + 1))),
                    _ => match self.editor.session.selection() {
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
                    && ui.ctx().pointer_hover_pos().is_some_and(|pos| {
                        self.canvas.controls.is_none_or(|rect| !rect.contains(pos))
                    })
                {
                    ui.ctx()
                        .set_cursor_icon(if self.controls.tool == Tool::Text {
                            egui::CursorIcon::Text
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
            },
        );
    }

    fn refresh_texture(&mut self, ctx: &egui::Context) {
        let key = (
            self.editor.session.ticket(),
            self.editor.tools.preview_revision(),
        );
        if self.canvas.key == Some(key) {
            return;
        }
        let result = self.editor.display_document().and_then(|preview| {
            paintcraft_render::render_flat(
                preview.as_ref().unwrap_or(self.editor.session.document()),
            )
        });
        let pixels = match result {
            Ok(pixels) => pixels,
            Err(error) => {
                // Never leave a failed speculative image presented as committed.
                self.editor.tools.cancel_gesture();
                self.message = format!("Preview unavailable: {error}");
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
        self.canvas.key = Some((
            self.editor.session.ticket(),
            self.editor.tools.preview_revision(),
        ));
    }

    fn canvas_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        if self.blocked() {
            self.editor.tools.cancel_gesture();
            return;
        }
        self.editor.tools.activate_tool(self.controls.tool);
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
            paintcraft_tools::commands::canvas_color(self.editor.session.document());
        let rect = response.rect;
        let zoom = self.zoom;
        let point = |pos: Pos2| {
            let p = (pos - rect.min) / zoom;
            ((p.x * 256.0).floor() as i64, (p.y * 256.0).floor() as i64)
        };
        let events = ui.input(|i| i.events.clone());
        if !ui.input(|i| i.focused) {
            self.editor.tools.cancel_gesture();
            return;
        }
        self.editor
            .tools
            .constrain_shape(ui.input(|i| i.modifiers.shift));
        for event in events {
            if self.keyboard_event(ui.ctx(), &event) {
                continue;
            }
            match event {
                egui::Event::WindowFocused(false) => {
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
                        egui::Key::Escape => self.editor.tools.cancel_gesture(),
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
                        && response.contains_pointer()
                    {
                        let layer = self.editor.target.unwrap_or(uuid::Uuid::nil());
                        if self.controls.tool == Tool::ColorPicker
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
                            } else {
                                self.editor
                                    .tools
                                    .pointer_down_subpixel(layer, point(pos), button);
                            }
                            self.editor.tools.constrain_shape(modifiers.shift);
                        }
                    } else if !pressed && self.editor.tools.gesture_button() == Some(button) {
                        self.editor.tools.pointer_move_subpixel(point(pos));
                        self.editor.tools.constrain_shape(modifiers.shift);
                        if let Some((_, action)) = self.editor.tools.pointer_up_button(button) {
                            self.edit(action);
                        }
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
                    if self.editor.tools.gesture_points().len() >= MAX_POINTS {
                        self.editor.tools.cancel_gesture();
                        self.message =
                            "Stroke cancelled: browser point limit reached; use shorter strokes"
                                .into();
                    } else {
                        self.editor.tools.pointer_move_subpixel(p);
                    }
                }
                egui::Event::PointerGone => {
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
