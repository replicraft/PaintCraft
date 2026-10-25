//! Browser admission and feedback for Classic's commit-on-release selected pixels.
use crate::app::WebApp;
use paintcraft_tools::{FloatingSelection, PointerButton, Tool};

pub(crate) const FINISH_MOVE: &str =
    "Finish moving the selection before opening, saving or exporting a file";
impl WebApp {
    /// True consumes the press, including a refused press over an existing field.
    pub(crate) fn begin_selection_float(
        &mut self,
        point: (i64, i64),
        button: PointerButton,
    ) -> bool {
        if self.controls.tool != Tool::Select || button != PointerButton::Primary {
            return false;
        }
        let at = (point.0.div_euclid(256), point.1.div_euclid(256));
        if !FloatingSelection::hit_test(&self.editor.session.selection(), at) {
            return false;
        }
        if self.blocked()
            || self.editor.tools.gesture_in_progress()
            || self.editor.tools.is_editing_text()
            || !self.text.preedit.is_empty()
        {
            return true;
        }
        let bounds = match self.selected_bounds() {
            Ok(Some(bounds)) => bounds,
            Ok(None) => return false,
            Err(error) => {
                self.message = error.message;
                return true;
            }
        };
        match self.editor.begin_floating(bounds, at, button) {
            Ok(_) => true,
            Err(error) => {
                self.message = error.message;
                true
            }
        }
    }
    pub(crate) fn selection_move_cursor(&self, at: (i64, i64)) -> bool {
        self.controls.tool == Tool::Select
            && self
                .editor
                .display_selection()
                .ok()
                .is_some_and(|selection| FloatingSelection::hit_test(&selection, at))
    }
    pub(crate) fn floating_file_ready(&mut self) -> bool {
        if self.paste.is_placing() {
            self.message = "Finish placing the pasted image before editing or saving.".into();
            return false;
        }
        self.editor.validate_floating();
        if self.editor.tools.floating_selection().is_some() {
            self.message = FINISH_MOVE.into();
            false
        } else {
            true
        }
    }
}
