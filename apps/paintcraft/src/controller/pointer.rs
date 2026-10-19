//! Pointer gestures retain button and color ownership until release or cancellation.
use super::*;
use paintcraft_input::PointerButton;

impl Editor {
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
        if self.tools.gesture_in_progress() || !self.tool.supports_button(button) {
            return;
        }
        self.commit_text();
        let layer = match (
            self.tool,
            self.target
                .filter(|id| self.document().layers.contains_key(id)),
        ) {
            (Tool::Select, t) => t.unwrap_or_else(Uuid::nil),
            (_, Some(layer)) => layer,
            (_, None) => {
                self.status = "No editing target: add or choose a layer".into();
                return;
            }
        };
        if self.tool != Tool::Select && !self.document().layers[&layer].visible {
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
    }
    pub fn pointer_move(&mut self, point: (i64, i64)) {
        self.tools.pointer_move(point);
    }
    pub fn pointer_move_subpixel(&mut self, point: (i64, i64)) {
        self.tools.pointer_move_subpixel(point);
    }
    pub fn pointer_up(&mut self) {
        self.pointer_up_button(PointerButton::Primary);
    }
    pub fn pointer_up_button(&mut self, button: PointerButton) {
        if let Some((token, action)) = self.tools.pointer_up_button(button) {
            self.outbox.push(token, action);
        }
    }
    pub fn cancel_gesture(&mut self) {
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
