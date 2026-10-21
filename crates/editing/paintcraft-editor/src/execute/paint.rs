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
        plan.apply_checked(
            &mut self.doc.layers.get_mut(&id).expect("resolved").raster,
            &mut self.adm,
            self.check,
        )
    }
}
