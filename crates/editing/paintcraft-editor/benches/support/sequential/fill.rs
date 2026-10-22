//! Data-dependent classification observes all earlier private edits.
use super::*;
impl Stage<'_> {
    pub(super) fn flood_fill(
        &mut self,
        reference: &LayerRef,
        at: (i64, i64),
        color: Rgba,
        mode: Mode,
        tolerance: u8,
    ) -> Result<()> {
        let id = self.resolve(reference)?;
        let layer = &self.doc.layers[&id];
        let Some(area) = self.editable(layer) else {
            return Ok(());
        };
        let (ox, oy) = layer.origin;
        let local = area
            .translated(-ox, -oy)
            .expect("area lies inside the layer");
        let spans = paintcraft_raster::flood::region_with_scratch_limit_checked(
            &layer.raster,
            (at.0 - ox, at.1 - oy),
            &local,
            tolerance,
            self.adm.flood_scratch_limit(),
            self.check,
        )?;
        if mode == Mode::SourceOver && color.a() == 0 {
            return Ok(());
        }
        let mut plan = self.adm.paint_plan();
        // The classifier already returns clipped layer-local spans. Do not copy
        // them into an intermediate document-space vector only to translate back.
        for span in spans {
            (self.check)()?;
            plan.rect(
                Rect {
                    x0: span.x0,
                    x1: span.x1 + 1,
                    y0: span.y,
                    y1: span.y + 1,
                },
                color,
                mode,
            )?;
        }
        self.apply_paint(id, &plan)
    }
}
