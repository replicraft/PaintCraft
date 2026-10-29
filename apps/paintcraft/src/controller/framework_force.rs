//! Frame-based force contacts retain editor ownership without native pen claims.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, resources::SharedBudget};
use paintcraft_tools::pen_stroke::FrameworkPoint;

impl Editor {
    pub(crate) fn framework_force_context_valid(&self) -> bool {
        self.tools.framework_force_eligible()
            && !self.painting_blocked()
            && !self.has_pending_edits()
            && !self.is_editing_text()
            && !self.has_image_paste()
            && !self.has_floating_selection()
            && !self.is_constructing_shape()
    }
    pub(crate) fn framework_force_eligible(&self) -> bool {
        self.framework_force_context_valid()
            && !self.gesture_in_progress()
            && self
                .target
                .is_some_and(|id| self.document().layers.get(&id).is_some_and(|l| l.visible))
    }
    pub(crate) fn framework_mouse_ink_active(&self) -> bool {
        self.framework_force_context_valid()
            && self.tools.gesture_is_path()
            && self.tools.pointer_is_down()
    }
    pub(crate) fn cancel_framework_force(&mut self) {
        if self.tools.framework_force_in_progress() {
            self.tools.cancel_framework_force();
            self.pen_base = None;
        }
    }
    pub(crate) fn framework_force_active(&self) -> bool {
        self.tools.framework_force_in_progress()
    }
    pub(crate) fn begin_framework_force(
        &mut self,
        point: FrameworkPoint,
        budget: &SharedBudget,
    ) -> Result<bool> {
        if !self.framework_force_eligible() {
            return Ok(false);
        }
        let layer = self.target.expect("eligible visible target");
        if self.tools.begin_framework_force(layer, point, budget)? {
            self.pen_base = Some(self.session.ticket());
            return Ok(true);
        }
        Ok(false)
    }
    pub(crate) fn append_framework_force(
        &mut self,
        point: Option<FrameworkPoint>,
        finish: bool,
    ) -> Result<()> {
        if !self.tools.framework_force_in_progress() {
            return Ok(());
        }
        if !self.framework_force_context_valid() {
            self.cancel_framework_force();
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Framework force editor context changed",
            ));
        }
        if self.pen_base != Some(self.session.ticket()) {
            self.cancel_framework_force();
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Framework force document changed during capture",
            ));
        }
        let action = if let Some(point) = point {
            self.tools.append_framework_force(point, finish)?
        } else if finish {
            self.tools.finish_framework_force()
        } else {
            None
        };
        if let Some((token, action)) = action {
            self.pen_base = None;
            self.request(token, action);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "framework_force/tests.rs"]
mod tests;
