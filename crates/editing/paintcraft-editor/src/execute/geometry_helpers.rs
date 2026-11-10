//! Existing stage lookup/clipping/stroke accounting, relocated without body changes.
use super::*;
impl Stage<'_> {
    pub(super) fn resolve(&self, r: &LayerRef) -> Result<Uuid> {
        match r {
            LayerRef::Alias(a) => match self.aliases.get(a) {
                Some(Some(id)) => Ok(*id),
                _ => Err(Error::new(
                    ErrorCode::ReferenceUnknownAlias,
                    format!("unknown alias {a:?}"),
                )),
            },
            LayerRef::Id(id) if self.doc.layers.contains_key(id) => Ok(*id),
            LayerRef::Id(_) => Err(Error::new(
                ErrorCode::ReferenceUnknownId,
                "unknown layer id",
            )),
        }
    }

    /// Document-space area an edit of `layer` may touch: canvas ∩ layer ∩ selection.
    pub(super) fn editable(&self, layer: &Layer) -> Option<Rect> {
        clip(
            &self.selection,
            self.doc.canvas_rect().intersect(&layer.extent()),
        )
    }

    pub(super) fn charge_stroke(
        &mut self,
        points: &[(i64, i64)],
        size: u32,
        scale: u32,
    ) -> Result<()> {
        let length = path::length_scaled(points, scale, self.check)?;
        self.pencil_pixels += 1 + length;
        self.pencil_segments += (points.len() as i128 - 1).max(1);
        self.stroke_rows += (1 + length) * i128::from(size);
        if self.pencil_pixels > PENCIL_PIXELS_MAX
            || self.pencil_segments > PENCIL_SEGMENTS_MAX
            || self.stroke_rows > STROKE_ROWS_MAX
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "stroke work budget exceeded",
            ));
        }
        Ok(())
    }
}
