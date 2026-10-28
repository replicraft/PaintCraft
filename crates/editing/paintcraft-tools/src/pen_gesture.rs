//! Captured pressure route; textured media require explicit dynamics opt-in.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    policy::{MaskIntent, MaskMode},
    resources::SharedBudget,
};
use paintcraft_input::tablet::{Phase, Sample};
impl ToolState {
    pub fn pressure_eligible(&self, sample: Sample) -> bool {
        use paintcraft_input::tablet::DeviceKind;
        // PaintCraft's inverted-end policy is a pressure-scaled round eraser,
        // independent of the selected paint media. It never changes that media
        // or substitutes a round tip for an ordinary selected square eraser.
        sample.identity.kind == DeviceKind::Eraser
            || (sample.identity.kind == DeviceKind::Pen
                && (sample.pressure.is_some()
                    || (self.options.pen.media_dynamics
                        && paintcraft_raster::media::dynamics::supports_orientation(
                            self.options.media,
                        )
                        && match self.options.pen.orientation {
                            paintcraft_core::types::pen::Orientation::Historical => false,
                            paintcraft_core::types::pen::Orientation::Tilt => sample.tilt.is_some(),
                            paintcraft_core::types::pen::Orientation::Twist => {
                                sample.twist_degrees.is_some()
                            }
                        }))
                && self.tool == Tool::Brush
                && ((self.options.round
                    && self.options.media == paintcraft_core::types::brush::BrushMedia::Plain)
                    || (self.options.pen.media_dynamics
                        && self.options.media != paintcraft_core::types::brush::BrushMedia::Plain))
                && self.sampled_tip.is_none())
    }
    pub fn pen_in_progress(&self) -> bool {
        self.pen.is_some()
    }
    /// False selects the established pointer/tool path; it is not a request to
    /// replace a selected media or sampled tip with generic round pressure ink.
    pub fn begin_pen(
        &mut self,
        layer: Uuid,
        sample: Sample,
        budget: &SharedBudget,
    ) -> Result<bool> {
        if !self.pressure_eligible(sample) {
            return Ok(false);
        }
        if self.gesture_in_progress() || self.is_editing_text() {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Finish the current draft before starting pen ink",
            ));
        }
        let inverted = sample.identity.kind == paintcraft_input::tablet::DeviceKind::Eraser;
        let button = if inverted {
            PointerButton::Primary
        } else {
            sample.buttons.ink_button()
        };
        if !inverted && !self.tool.supports_button(button) {
            return Ok(false);
        }
        let mut color = if button == PointerButton::Secondary {
            self.secondary
        } else {
            self.color
        };
        let (mode, intent) = if inverted {
            color = self.options.eraser_color;
            match color.a() {
                0 => {
                    // Eraser strength is coverage, not transparent source alpha.
                    // DestinationOut preserves surviving straight and hidden RGB.
                    color = Rgba([0, 0, 0, 255]);
                    (MaskMode::DestinationOut, MaskIntent::Erase)
                }
                255 => (MaskMode::Replace, MaskIntent::Erase),
                _ => {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "Pen eraser requires transparent or opaque canvas policy",
                    ));
                }
            }
        } else {
            color.0[3] = ((u16::from(color.a()) * u16::from(self.options.brush_opacity.min(100))
                + 50)
                / 100) as u8;
            (MaskMode::SourceOver, MaskIntent::Brush)
        };
        let stroke = pen_stroke::PenStroke::begin(
            pen_stroke::Style {
                layer,
                diameter: self.options.size,
                color,
                mode,
                intent,
                settings: self.options.pen,
                media: if inverted {
                    Default::default()
                } else {
                    self.options.media
                },
            },
            sample,
            budget,
        )?;
        let token = self
            .next_gesture
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Pen gesture token exhausted"))?;
        self.next_gesture = token;
        self.pen = Some((token, stroke));
        self.preview_revision.advance();
        Ok(true)
    }
    /// Only the classified pen stream may finish this capture. A promoted mouse
    /// release stays on the legacy method and cannot publish the pen action.
    pub fn append_pen(&mut self, sample: Sample) -> Result<Option<(u64, Action)>> {
        let Some((_, stroke)) = &mut self.pen else {
            return Ok(None);
        };
        if matches!(sample.phase, Phase::Cancel | Phase::Leave) {
            self.cancel_gesture();
            return Ok(None);
        }
        if let Err(error) = stroke.append(sample) {
            self.cancel_gesture();
            return Err(error);
        }
        self.preview_revision.advance();
        if sample.phase == Phase::Up {
            let (token, stroke) = self.pen.take().expect("captured pen");
            return Ok(Some((token, Action::PenStroke(stroke))));
        }
        Ok(None)
    }
}
