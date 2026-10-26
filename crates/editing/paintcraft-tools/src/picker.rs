//! Picker ownership and session-only post-click behavior.
use super::*;
use paintcraft_core::types::picker::PickerAfter;
impl ToolState {
    /// Explicit activation also invalidates late picker results on re-selection.
    pub fn choose_tool(&mut self, tool: Tool) {
        self.resource_shape = None;
        if self.floating.is_some() {
            self.cancel_gesture();
        }
        if self.tool == tool {
            self.preview_revision.advance();
        } else {
            self.activate_tool(tool);
        }
    }
    /// Synchronize tool state without advancing its revision every layout frame.
    pub fn activate_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.resource_shape = None;
            if self.floating.is_some() {
                self.cancel_gesture();
            }
            if self.tool != Tool::ColorPicker {
                self.previous_tool = self.tool;
            }
            self.tool = tool;
            self.preview_revision.advance();
        }
    }
    pub(super) fn color_pick(&self, g: &Gesture, at: (i64, i64)) -> ColorPick {
        ColorPick {
            original: g.color,
            revision: self.preview_revision,
            layer: g.layer,
            at,
            button: g.button,
            source: g.options.picker_source,
            size: g.options.picker_size,
            return_to: match g.options.picker_after {
                PickerAfter::Stay => None,
                PickerAfter::Previous => Some(g.return_tool),
                PickerAfter::Pencil => Some(Tool::Pencil),
            },
        }
    }
    pub fn can_apply_color_pick(&self, pick: &ColorPick) -> bool {
        let current = if pick.button == PointerButton::Primary {
            self.color
        } else {
            self.secondary
        };
        self.tool == Tool::ColorPicker
            && self.preview_revision == pick.revision
            && current == pick.original
    }
    pub fn apply_picked_color(&mut self, pick: &ColorPick, color: Rgba) -> bool {
        if !self.can_apply_color_pick(pick) {
            return false;
        }
        if pick.button == PointerButton::Primary {
            self.color = color;
        } else {
            self.secondary = color;
        }
        if self.tool == Tool::ColorPicker
            && let Some(tool) = pick.return_to
        {
            self.activate_tool(tool);
        }
        true
    }
}
