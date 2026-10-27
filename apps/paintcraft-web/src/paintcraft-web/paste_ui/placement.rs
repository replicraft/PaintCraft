//! Interactive ordinary Paste. Source/target stay frozen until explicit placement.
use super::*;
use paintcraft_core::types::{Rect, policy::BlendMode};
use paintcraft_tools::{ImagePlacement, PointerButton, PreviewRevision, Tool};

#[derive(Debug)]
pub(super) struct Placing {
    position: ImagePlacement,
    draft: PasteDraft,
    target: uuid::Uuid,
    key: Option<paintcraft_core::types::Rgba>,
}
impl WebApp {
    pub(super) fn begin_paste_placement(&mut self, ctx: &egui::Context, ready: Ready) {
        let Some(target) = self.paste.owner.as_ref().and_then(|owner| owner.target) else {
            return;
        };
        match ImagePlacement::new(ready.draft.image().clone(), ready.origin) {
            Ok(position) => {
                self.paste.placing = Some(Placing {
                    position,
                    draft: ready.draft,
                    target,
                    key: self
                        .controls
                        .options
                        .transparent_selection
                        .then_some(self.controls.secondary),
                });
                self.controls.tool = Tool::Select;
                self.editor.tools.activate_tool(Tool::Select);
                self.canvas.invalidate();
                self.message = t!(ctx, "Drag to position; Enter places, Escape cancels.");
            }
            Err(error) => self.message = format!("{}: {error}", t!(ctx, "Clipboard paste failed")),
        }
    }
    fn placement_current(&self) -> bool {
        self.paste
            .owner
            .as_ref()
            .is_some_and(|owner| owner.current(self))
            && self.controls.tool == Tool::Select
            && self.editor.tools.tool == Tool::Select
    }
    pub(crate) fn validate_paste_placement(&mut self, ctx: &egui::Context) {
        if self.paste.is_placing() && !self.placement_current() {
            self.cancel_paste(ctx);
            self.message = t!(ctx, "The document changed; paste again.");
        }
    }
    pub(crate) fn paste_revision(&self) -> Option<PreviewRevision> {
        self.paste.placing.as_ref().map(|p| p.position.revision())
    }
    pub(crate) fn paste_bounds(&self) -> Option<Rect> {
        self.paste
            .placing
            .as_ref()
            .filter(|_| self.placement_current())
            .and_then(|p| {
                p.position
                    .bounds()
                    .intersect(&self.editor.session.document().canvas_rect())
            })
    }
    /// Real target compositing, clipped to the current canvas until placement.
    pub(crate) fn paste_preview_document(
        &self,
    ) -> paintcraft_core::types::Result<Option<Document>> {
        let Some(p) = &self.paste.placing else {
            return Ok(None);
        };
        if !self.placement_current() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "Paste source changed",
            ));
        }
        let mut doc = self.editor.session.preview_paste_image_onto(
            &p.draft,
            p.target,
            BlendMode::SourceOver,
            p.key,
            paintcraft_raster::Admission::new(crate::editor::MAX_PIXELS * 4),
            &|| Ok(()),
        )?;
        doc.canvas = self.editor.session.document().canvas;
        Ok(Some(doc))
    }
    fn change_paste_position(
        &mut self,
        ctx: &egui::Context,
        change: impl FnOnce(&mut ImagePlacement) -> paintcraft_core::types::Result<()>,
    ) {
        let Some(p) = &mut self.paste.placing else {
            return;
        };
        let previous = p.position.clone();
        let draft = p.draft.clone();
        let result = change(&mut p.position)
            .and_then(|()| p.draft.place(p.position.origin(), p.draft.mode()))
            .and_then(|()| {
                let (w, h) = p.draft.canvas_size();
                if u64::from(w) * u64::from(h) > crate::editor::MAX_PIXELS {
                    Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::ResourceLimit,
                        "Browser canvas limit is 4 megapixels",
                    ))
                } else {
                    Ok(())
                }
            });
        if result.is_err() {
            p.position = previous;
            p.draft = draft;
            self.message = t!(ctx, "Choose a valid image position and canvas size.");
        }
    }
    pub(crate) fn place_paste(&mut self, ctx: &egui::Context) {
        if !self.placement_current() {
            self.cancel_paste(ctx);
            self.message = t!(ctx, "The document changed; paste again.");
            return;
        }
        let Some(p) = self.paste.placing.take() else {
            return;
        };
        let before = self.editor.session.document().canvas;
        match self
            .editor
            .session
            .paste_image_onto(&p.draft, p.target, BlendMode::SourceOver, p.key)
        {
            Ok(receipt) => {
                self.editor.target = receipt
                    .aliases
                    .get("pasted")
                    .copied()
                    .or(self.editor.target);
                self.paste.owner = None;
                self.canvas.invalidate();
                let size = self.editor.session.document().canvas;
                if size != before {
                    self.remember_canvas_size(size);
                }
                self.message = t!(ctx, "Paste");
            }
            Err(error) => {
                self.paste.placing = Some(p);
                self.message = format!("{}: {error}", t!(ctx, "Clipboard paste failed"));
            }
        }
    }
    pub(crate) fn paste_keyboard(&mut self, ctx: &egui::Context, event: &egui::Event) -> bool {
        if !self.paste.is_placing() {
            return false;
        }
        let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            return false;
        };
        if self.blocked() || ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx) {
            return false;
        }
        match key {
            egui::Key::Escape => self.cancel_paste(ctx),
            egui::Key::Enter => self.place_paste(ctx),
            egui::Key::Z if modifiers.command => self.cancel_paste(ctx),
            egui::Key::ArrowLeft
            | egui::Key::ArrowRight
            | egui::Key::ArrowUp
            | egui::Key::ArrowDown
                if !modifiers.command && !modifiers.alt =>
            {
                let step = if modifiers.shift { 10 } else { 1 };
                let (dx, dy) = match key {
                    egui::Key::ArrowLeft => (-step, 0),
                    egui::Key::ArrowRight => (step, 0),
                    egui::Key::ArrowUp => (0, -step),
                    _ => (0, step),
                };
                self.change_paste_position(ctx, |p| {
                    let (x, y) = p.origin();
                    p.place((x + dx, y + dy))
                });
            }
            _ => {
                self.message = t!(
                    ctx,
                    "Finish placing the pasted image before editing or saving."
                )
            }
        }
        true
    }
    /// Consumes canvas events while floating; pointer release retains the draft.
    pub(crate) fn paste_canvas_input(&mut self, ui: &egui::Ui, response: &egui::Response) {
        self.validate_paste_placement(ui.ctx());
        if !self.paste.is_placing() {
            return;
        }
        let rect = response.rect;
        let zoom = self.zoom;
        let point = |pos: egui::Pos2| {
            (
                ((pos.x - rect.min.x) / zoom).floor() as i64,
                ((pos.y - rect.min.y) / zoom).floor() as i64,
            )
        };
        for event in self.canvas_events(ui.ctx()) {
            if self.keyboard_event(ui.ctx(), &event) {
                continue;
            }
            if !self.paste.is_placing() {
                break;
            }
            match event {
                egui::Event::PointerMoved(pos) => {
                    let at = point(pos);
                    self.canvas.pointer = Some(at);
                    self.change_paste_position(ui.ctx(), |p| p.move_to(at));
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    let at = point(pos);
                    if pressed {
                        let over = rect.contains(pos)
                            && ui.clip_rect().contains(pos)
                            && ui.ctx().layer_id_at(pos) == Some(ui.layer_id());
                        if over
                            && !self
                                .paste
                                .placing
                                .as_mut()
                                .unwrap()
                                .position
                                .press(at, PointerButton::Primary)
                        {
                            self.place_paste(ui.ctx());
                        }
                    } else {
                        self.change_paste_position(ui.ctx(), |p| {
                            p.release(at, PointerButton::Primary).map(|_| ())
                        });
                        if let Some(p) = &mut self.paste.placing {
                            p.position.interrupt_drag();
                        }
                    }
                }
                egui::Event::PointerGone | egui::Event::WindowFocused(false) => {
                    if let Some(p) = &mut self.paste.placing {
                        p.position.interrupt_drag();
                    }
                }
                _ => {}
            }
        }
        if let Some(p) = &self.paste.placing
            && ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|pos| p.position.bounds().contains(point(pos).0, point(pos).1))
        {
            ui.ctx().set_cursor_icon(if p.position.is_dragging() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
    }
    pub(crate) fn paste_placement_bar(&mut self, ui: &mut egui::Ui) {
        self.validate_paste_placement(ui.ctx());
        if !self.paste.is_placing() {
            return;
        }
        let ctx = ui.ctx().clone();
        let mut apply = false;
        let mut cancel = false;
        egui::Panel::top("web-paste-placement").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(t!(&ctx, "Drag to position; Enter places, Escape cancels."));
                if let Some(p) = &self.paste.placing {
                    let (x, y) = p.position.origin();
                    ui.label(format!("X: {x}  Y: {y}"));
                }
                apply = ui.button(t!(&ctx, "Apply")).clicked();
                cancel = ui.button(t!(&ctx, "Cancel")).clicked();
            });
        });
        if ctx.current_pass_index() == 0 {
            if cancel {
                self.cancel_paste(&ctx);
            } else if apply {
                self.place_paste(&ctx);
            }
        }
    }
}
#[cfg(test)]
mod tests;

impl crate::WebApp {
    pub(crate) fn interrupt_paste_touch_drag(&mut self) {
        if let Some(p) = &mut self.paste.placing {
            p.position.interrupt_drag();
        }
    }
    pub(crate) fn paste_touch_press_is_one_shot(&self, pos: Option<egui::Pos2>) -> bool {
        self.paste.placing.as_ref().is_some_and(|p| {
            pos.zip(self.canvas.rect).is_some_and(|(pos, rect)| {
                let at = (pos - rect.min) / self.zoom;
                p.position.is_dragging()
                    || !p
                        .position
                        .bounds()
                        .contains(at.x.floor() as i64, at.y.floor() as i64)
            })
        })
    }
}
