//! Shared ordered coverage descriptors for painting operations.
use super::*;
impl Stage<'_> {
    pub(super) fn add_paint_spans(
        &self,
        id: Uuid,
        spans: &[Span],
        color: Rgba,
        mode: Mode,
        plan: &mut PaintPlan,
    ) -> Result<()> {
        if mode == Mode::SourceOver && color.a() == 0 {
            return Ok(());
        }
        let (ox, oy) = self.doc.layers[&id].origin;
        for span in spans {
            plan.rect(
                Rect {
                    x0: span.x0 - ox,
                    x1: span.x1 + 1 - ox,
                    y0: span.y - oy,
                    y1: span.y + 1 - oy,
                },
                color,
                mode,
            )?;
        }
        Ok(())
    }
    pub(super) fn apply_paint(&mut self, id: Uuid, plan: &PaintPlan) -> Result<()> {
        if let Some(planning) = &mut self.planning {
            // Filter once before retaining logical/replay descriptors. Reapplying
            // coverage at replay could change overlap and replacement semantics.
            let filtered;
            let plan = if let Selection::Mask(mask) = &self.selection {
                filtered = plan.masked_checked(
                    mask,
                    self.doc.layers[&id].origin,
                    &mut self.selection_work,
                    self.check,
                )?;
                &filtered
            } else {
                plan
            };
            return planning.paint(
                id,
                &self.doc.layers[&id].raster,
                plan,
                self.operation,
                &self.adm,
                self.check,
            );
        }
        plan.apply_checked(
            &mut self.doc.layers.get_mut(&id).expect("resolved").raster,
            &mut self.adm,
            self.check,
        )
    }
}
