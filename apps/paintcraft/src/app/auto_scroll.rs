//! Studio edge scrolling is armed by an accepted tool press, never by hovering.
use super::*;

impl App {
    fn studio_auto_scroll_scope(&self, ctx: &egui::Context, viewport: Rect) -> egui::Id {
        egui::Id::unique((
            self.studio_pan_scope(ctx),
            self.editor.gesture_token(),
            viewport.width().to_bits(),
            viewport.height().to_bits(),
            self.settings.studio_auto_scroll,
        ))
    }

    pub(super) fn begin_studio_auto_scroll(&mut self, ui: &egui::Ui, rect: Rect, viewport: Rect) {
        if self.experience() == Experience::Studio
            && self.settings.studio_auto_scroll
            && self.editor.pointer_is_down()
            && !matches!(
                self.editor.tool,
                Tool::Pan | Tool::Magnifier | Tool::Text | Tool::Fill | Tool::ColorPicker
            )
        {
            self.studio.auto_scroll.begin(
                self.studio_auto_scroll_scope(ui.ctx(), viewport),
                (rect.min.x, rect.min.y),
                ui.input(|input| input.time),
            );
        }
    }

    pub(super) fn advance_studio_auto_scroll(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        viewport: Rect,
    ) {
        let pointer = ui.input(|input| {
            let down = match self.editor.gesture_button() {
                Some(paintcraft_input::PointerButton::Primary) => input.pointer.primary_down(),
                Some(paintcraft_input::PointerButton::Secondary) => input.pointer.secondary_down(),
                None => false,
            };
            (input.focused && down)
                .then(|| input.pointer.hover_pos())
                .flatten()
        });
        let enabled = self.experience() == Experience::Studio
            && self.settings.studio_auto_scroll
            && ui.is_enabled()
            && !self.action_context().blocked
            && !egui::Popup::is_any_open(ui.ctx())
            && !self.ime_composing
            && !self.studio.pan.is_active()
            && !self.studio.zoom.is_active();
        let Some(pointer) = pointer
            .filter(|point| enabled && ui.ctx().layer_id_at(*point) == Some(painter.layer_id()))
        else {
            self.studio.auto_scroll.cancel();
            return;
        };
        let scope = self.studio_auto_scroll_scope(ui.ctx(), viewport);
        let Some(step) = self.studio.auto_scroll.advance(
            &scope,
            (rect.min.x, rect.min.y),
            (pointer.x, pointer.y),
            (
                (viewport.min.x, viewport.min.y),
                (viewport.max.x, viewport.max.y),
            ),
            ui.input(|input| input.time),
        ) else {
            return;
        };
        // Scroll moves the document under the pointer even without a new OS move
        // event. Use the actual new transform, preserving the original gesture.
        let moved = ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerMoved(_)))
        });
        if step.rebase && !moved {
            let view = self.canvas_view((rect.min.x, rect.min.y), ui.ctx().pixels_per_point());
            if let Some(at) = view.to_doc_subpixel((pointer.x, pointer.y)) {
                self.editor.pointer_move_subpixel(at);
            }
        }
        if step.at_edge {
            if step.delta == (0.0, 0.0) {
                // The press frame has zero elapsed time. Start the animation once.
                ui.ctx().request_repaint();
            } else {
                paintcraft_studio::canvas::pan_by_clamped(
                    ui.ctx(),
                    egui::Id::unique(self.active_tab_id()),
                    self.studio_pan_scope(ui.ctx()),
                    Vec2::new(step.delta.0, step.delta.1),
                );
            }
        }
    }
}
