//! Compile/admit immutable selections once and reuse exact views during replay.
use super::*;
use paintcraft_raster::selection::{BinaryMask, MaskLimits};

impl Stage<'_> {
    pub(super) fn mask_selection(&mut self, index: usize, op: &Op) -> Result<()> {
        if self.planning.is_none() {
            self.selection = self
                .selections
                .values
                .iter()
                .find(|(operation, _)| *operation == index)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvariantViolation,
                        "compiled selection was not retained",
                    )
                })?
                .1
                .clone();
            return Ok(());
        }
        let domain = self.doc.canvas_rect();
        let budget = self.adm.selection_budget();
        let limits = MaskLimits::default();
        let mask = match op {
            Op::SelectionSetPolygon { points } => {
                BinaryMask::polygon(domain, points, &budget, limits, self.check)?
            }
            Op::SelectionInvert => {
                let mask = match &self.selection {
                    // With no active selection, Invert selects the whole canvas.
                    Selection::Inactive | Selection::Empty => BinaryMask::uniform(domain, false)?,
                    Selection::Rect(rect) => BinaryMask::uniform(*rect, true)?,
                    Selection::Mask(mask) => mask.clone(),
                };
                mask.reframed(domain, &budget, limits, self.check)?
                    .inverted()
            }
            _ => unreachable!("selection compiler operation"),
        };
        self.selection = Selection::Mask(mask);
        self.selections.push((index, self.selection.clone()))
    }
    pub(super) fn resize_selection(&mut self, dx: i64, dy: i64) -> Result<()> {
        self.selection = match &self.selection {
            Selection::Rect(rect) => rect
                .translated(dx, dy)
                .and_then(|rect| rect.intersect(&self.doc.canvas_rect()))
                .map_or(Selection::Empty, Selection::Rect),
            Selection::Mask(mask) => mask
                .translated(dx, dy)?
                .clipped(self.doc.canvas_rect())?
                .map_or(Selection::Empty, Selection::Mask),
            other => other.clone(),
        };
        Ok(())
    }
}
