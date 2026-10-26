//! Pointer gestures retain button and color ownership until release or cancellation.
use super::*;
use paintcraft_input::PointerButton;

impl Editor {
    pub(crate) fn shape_file_ready(&mut self) -> bool {
        if self.has_image_paste() {
            self.status = "Finish placing the pasted image before editing or saving.".into();
            return false;
        }
        self.finish_selection_move();
        if self.is_constructing_shape() {
            self.set_status(paintcraft_tools::FINISH_SHAPE_BEFORE_FILE);
            false
        } else {
            true
        }
    }
    pub fn pointer_down(&mut self, point: (i64, i64)) {
        self.pointer_down_button(point, PointerButton::Primary);
    }
    pub fn pointer_down_button(&mut self, point: (i64, i64), button: PointerButton) {
        self.begin_pointer(point, button, None);
    }
    pub fn pointer_down_subpixel(&mut self, at: (i64, i64), button: PointerButton) {
        self.begin_pointer(
            (at.0.div_euclid(256), at.1.div_euclid(256)),
            button,
            Some(at),
        );
    }
    fn begin_pointer(
        &mut self,
        point: (i64, i64),
        button: PointerButton,
        precise: Option<(i64, i64)>,
    ) {
        if self.image_paste_press(point, button) {
            return;
        }
        if self.painting_blocked()
            || self.tools.pointer_is_down()
            || !self.tool.supports_button(button)
        {
            return;
        }
        if self.tool != Tool::ColorPicker {
            self.commit_text();
        }
        if self.begin_floating(point, button) {
            return;
        }
        let layer = match (
            self.tool,
            self.target
                .filter(|id| self.document().layers.contains_key(id)),
        ) {
            (Tool::Select | Tool::ColorPicker, t) => t.unwrap_or_else(Uuid::nil),
            (_, Some(layer)) => layer,
            (Tool::Brush, None)
                if self.tools.sampled_tip.is_some() && self.document().layers.is_empty() =>
            {
                Uuid::nil()
            }
            (_, None) => {
                self.status = "No editing target: add or choose a layer".into();
                return;
            }
        };
        if !matches!(self.tool, Tool::Select | Tool::ColorPicker)
            && self
                .document()
                .layers
                .get(&layer)
                .is_some_and(|layer| !layer.visible)
        {
            self.status = "The target layer is hidden; show it or choose another layer".into();
            return;
        }
        if self.tool == Tool::Text {
            self.tools.begin_text(layer, point);
            self.status =
                "Type your text; click elsewhere or press Ctrl+Enter to place it, Escape to cancel"
                    .into();
        } else if let Some(at) = precise {
            self.tools.pointer_down_subpixel(layer, at, button);
        } else {
            self.tools.pointer_down_button(layer, point, button);
        }
        self.refresh_shape_hint();
    }
    pub fn pointer_move(&mut self, point: (i64, i64)) {
        if self.image_paste_move(point) {
            return;
        }
        self.guard_floating_source();
        self.move_paint_pointer(point, false);
    }
    pub fn pointer_move_subpixel(&mut self, point: (i64, i64)) {
        if self.image_paste_move((point.0.div_euclid(256), point.1.div_euclid(256))) {
            return;
        }
        self.guard_floating_source();
        self.move_paint_pointer(point, true);
    }
    pub fn pointer_up(&mut self) {
        self.pointer_up_button(PointerButton::Primary);
    }
    pub fn pointer_up_button(&mut self, button: PointerButton) {
        if self.image_paste_release(button) {
            return;
        }
        self.guard_floating_source();
        if let Some((token, action)) = self.tools.pointer_up_button(button) {
            self.request(token, action);
        }
        self.refresh_shape_hint();
    }
    pub fn is_constructing_shape(&self) -> bool {
        self.tools.is_constructing_shape()
    }
    pub fn gesture_button(&self) -> Option<PointerButton> {
        if self.image_paste_dragging() {
            Some(PointerButton::Primary)
        } else {
            self.tools.gesture_button()
        }
    }
    pub fn pointer_is_down(&self) -> bool {
        self.image_paste_dragging() || self.tools.pointer_is_down()
    }
    pub fn finish_shape(&mut self) {
        if let Some((token, action)) = self.tools.finish_shape() {
            self.request(token, action);
        }
    }
    pub fn remove_shape_vertex(&mut self) {
        self.tools.remove_shape_vertex();
        self.refresh_shape_hint();
    }
    fn refresh_shape_hint(&mut self) {
        if let Some(hint) = self.tools.shape_hint() {
            self.status = hint.into();
        }
    }
    pub fn cancel_gesture(&mut self) {
        self.interrupt_image_paste();
        self.floating_source = None;
        self.tools.cancel_gesture();
    }
    pub fn pointer_event(&mut self, event: paintcraft_input::PointerEvent) {
        use paintcraft_input::PointerEvent;
        match event {
            PointerEvent::Down { at } => self.pointer_down(at),
            PointerEvent::ButtonDown { at, button } => self.pointer_down_button(at, button),
            PointerEvent::ButtonUp { button } => self.pointer_up_button(button),
            PointerEvent::Move { at } => self.pointer_move(at),
            PointerEvent::Up => self.pointer_up(),
            PointerEvent::Cancel => self.cancel_gesture(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_input::PointerEvent;

    #[test]
    fn normalized_secondary_events_publish_only_on_the_matching_release() {
        let mut editor = Editor::new_document(4, 4).unwrap();
        editor.color = Rgba([255, 0, 0, 255]);
        editor.secondary = Rgba([0, 0, 255, 255]);
        let target = editor.target().unwrap();
        editor.pointer_event(PointerEvent::ButtonDown {
            at: (1, 1),
            button: PointerButton::Secondary,
        });
        editor.pointer_event(PointerEvent::Move { at: (2, 1) });
        editor.pointer_event(PointerEvent::Up);
        assert_eq!(editor.run_pending(), 0);
        assert_eq!(editor.gesture_button(), Some(PointerButton::Secondary));
        editor.pointer_event(PointerEvent::ButtonUp {
            button: PointerButton::Secondary,
        });
        assert_eq!(editor.run_pending(), 1);
        assert_eq!(
            editor.document().layers[&target].raster.get(1, 1),
            Rgba([0, 0, 255, 255])
        );
        assert_eq!(editor.color, Rgba([255, 0, 0, 255]));
        editor.pointer_event(PointerEvent::Down { at: (3, 3) });
        editor.pointer_event(PointerEvent::Cancel);
        editor.pointer_event(PointerEvent::Up);
        assert_eq!(editor.run_pending(), 0);
        assert_eq!(editor.gesture_button(), None);
    }
}
