//! Explicit framework-force gesture. No native pen sample or tool is invented.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    policy::{MaskIntent, MaskMode},
    resources::SharedBudget,
};
use pen_stroke::{FrameworkPoint, PenStroke, Style};

impl ToolState {
    pub fn framework_force_eligible(&self) -> bool {
        self.tool == Tool::Brush
            && self.options.round
            && self.options.media == paintcraft_core::types::brush::BrushMedia::Plain
            && self.sampled_tip.is_none()
    }
    pub fn framework_force_in_progress(&self) -> bool {
        self.pen
            .as_ref()
            .is_some_and(|(_, stroke)| stroke.is_framework())
    }
    pub fn cancel_framework_force(&mut self) {
        if self.framework_force_in_progress() {
            self.pen = None;
            self.preview_revision.advance();
        }
    }
    pub fn begin_framework_force(
        &mut self,
        layer: Uuid,
        point: FrameworkPoint,
        budget: &SharedBudget,
    ) -> Result<bool> {
        if !self.framework_force_eligible() {
            return Ok(false);
        }
        if self.gesture_in_progress() || self.is_editing_text() {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Finish the current draft before force painting",
            ));
        }
        let mut color = self.color;
        color.0[3] = ((u16::from(color.a()) * u16::from(self.options.brush_opacity.min(100)) + 50)
            / 100) as u8;
        let stroke = PenStroke::begin_framework(
            Style {
                layer,
                diameter: self.options.size,
                color,
                mode: MaskMode::SourceOver,
                intent: MaskIntent::Brush,
                settings: self.options.pen,
                media: self.options.media,
            },
            point,
            budget,
        )?;
        let token = self
            .next_gesture
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Force gesture token exhausted"))?;
        self.next_gesture = token;
        self.pen = Some((token, stroke));
        self.preview_revision.advance();
        Ok(true)
    }
    pub fn finish_framework_force(&mut self) -> Option<(u64, Action)> {
        if !self.framework_force_in_progress() {
            return None;
        }
        let (token, stroke) = self.pen.take().expect("framework capture");
        self.preview_revision.advance();
        Some((token, Action::PenStroke(stroke)))
    }
    pub fn append_framework_force(
        &mut self,
        point: FrameworkPoint,
        finish: bool,
    ) -> Result<Option<(u64, Action)>> {
        if !self.framework_force_in_progress() {
            return Ok(None);
        }
        let (_, stroke) = self.pen.as_mut().expect("framework capture");
        if let Err(error) = stroke.append_framework(point) {
            self.cancel_framework_force();
            return Err(error);
        }
        self.preview_revision.advance();
        if finish {
            let (token, stroke) = self.pen.take().expect("framework capture");
            return Ok(Some((token, Action::PenStroke(stroke))));
        }
        Ok(None)
    }
}
