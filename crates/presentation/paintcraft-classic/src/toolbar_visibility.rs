//! Presentation-only reveal state for Classic's automatically hidden toolbar.
use egui::{Rect, Ui};

#[derive(Debug, Default)]
pub struct ToolbarVisibility {
    collapsed: bool,
    bounds: Option<Rect>,
    frame: Option<u64>,
}

impl ToolbarVisibility {
    pub fn visible(&self) -> bool {
        !self.collapsed
    }

    /// The host includes any font/options rows in the same hover region.
    pub fn remember_bounds(&mut self, bounds: Rect) {
        self.bounds = Some(bounds);
    }

    pub(super) fn update(&mut self, ui: &Ui, menu: Rect, auto_hide: bool) -> bool {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.frame == Some(frame) {
            return self.visible();
        }
        self.frame = Some(frame);
        let previous = self.collapsed;
        if !auto_hide {
            self.collapsed = false;
        } else {
            let (pointer, hold) = ui.input(|input| {
                (
                    input.pointer.hover_pos(),
                    input.pointer.any_down() || input.pointer.any_released(),
                )
            });
            // Hold the previous layout through down/up frames, including a
            // coalesced canvas press/release. A gesture never sees two mappings.
            if !hold {
                if pointer.is_some_and(|point| {
                    menu.contains(point) || self.bounds.is_some_and(|bounds| bounds.contains(point))
                }) {
                    self.collapsed = false;
                } else if !egui::Popup::is_any_open(ui.ctx())
                    && !ui.ctx().egui_wants_keyboard_input()
                {
                    self.collapsed = true;
                }
            }
        }
        if self.collapsed != previous {
            ui.ctx().request_repaint();
        }
        self.visible()
    }
}
