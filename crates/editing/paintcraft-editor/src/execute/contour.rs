//! One admitted paint plan for a polygon fill and its once-covered outline.
use super::*;

impl Stage<'_> {
    pub(super) fn contour(
        &mut self,
        layer: &LayerRef,
        points: &[(i64, i64)],
        closed: bool,
        paint: (Option<(Rgba, u32)>, Option<Rgba>, Mode),
    ) -> Result<()> {
        let id = self.resolve(layer)?;
        let Some(clip) = self.editable(&self.doc.layers[&id]) else {
            return Ok(());
        };
        let (stroke, fill, mode) = paint;
        let mut plan = self.adm.paint_plan();
        if let Some(color) = fill {
            shape::visit_polygon(points, &clip, self.check, |span| {
                self.add_paint_spans(id, &[span], color, mode, &mut plan)
            })?;
        }
        if let Some((color, size)) = stroke {
            let mut path = Vec::new();
            path.try_reserve_exact(points.len() + usize::from(closed))
                .map_err(|_| {
                    Error::new(ErrorCode::ResourceLimit, "cannot reserve contour outline")
                })?;
            path.extend_from_slice(points);
            if closed && path.first() != path.last() {
                path.push(points[0]);
            }
            if let Some(spans) = self.stroke_spans(id, &path, size, true)? {
                spans.visit_spans(self.check, |span| {
                    self.add_paint_spans(id, &[span], color, mode, &mut plan)
                })?;
            }
        }
        self.apply_paint(id, &plan)
    }
}
