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
        let mask = match &self.selection {
            Selection::Mask(mask) => Some(mask),
            _ => None,
        };
        let predicate = |x: i64, y: i64| -> Result<bool> {
            Ok(mask.is_none_or(|mask| mask.coverage(x + ox, y + oy) == 255))
        };
        let allowed = mask.map(|_| &predicate as &dyn Fn(i64, i64) -> Result<bool>);
        // A blocked seed is empty without allocating a classifier or reading pixels.
        if let Some(allowed) = allowed
            && !allowed(at.0 - ox, at.1 - oy)?
        {
            return Ok(());
        }
        let spans = if let Some(planning) = &mut self.planning {
            planning.flood(
                id,
                &layer.raster,
                (at.0 - ox, at.1 - oy),
                local,
                tolerance,
                allowed,
                &self.adm,
                self.check,
            )?
        } else {
            planning::classify_flood(
                |x, y| Ok(layer.raster.get(x, y)),
                allowed,
                (at.0 - ox, at.1 - oy),
                local,
                tolerance,
                &self.adm,
                self.check,
            )?
        };
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
