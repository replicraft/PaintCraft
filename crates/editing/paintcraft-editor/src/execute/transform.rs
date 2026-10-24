//! Exact whole-document raster orientation, preserving complete layer extents.
use super::*;
use paintcraft_core::types::orthogonal::OrthogonalTransform;
use paintcraft_raster::MappedPlan;
use std::sync::Arc;
const READS_MAX: u64 = 16_777_216;

fn origin(t: OrthogonalTransform, canvas: (u32, u32), extent: Rect) -> Result<(i64, i64)> {
    let (w, h) = (i64::from(canvas.0), i64::from(canvas.1));
    use OrthogonalTransform::*;
    let p = match t {
        RotateRight => (h - extent.y1, extent.x0),
        RotateLeft => (extent.y0, w - extent.x1),
        Rotate180 => (w - extent.x1, h - extent.y1),
        FlipHorizontal => (w - extent.x1, extent.y0),
        FlipVertical => (extent.x0, h - extent.y1),
    };
    if p.0.unsigned_abs() > COORD_ABS_MAX as u64 || p.1.unsigned_abs() > COORD_ABS_MAX as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "transformed layer origin exceeds coordinate budget",
        ));
    }
    Ok(p)
}
fn raster(t: OrthogonalTransform, source: &RasterLayer) -> RasterLayer {
    let (w, h) = t.dimensions(source.width(), source.height());
    RasterLayer::new(w, h, source.default_pixel())
}
impl Stage<'_> {
    fn transform_selection(&mut self, t: OrthogonalTransform) -> Result<()> {
        self.selection = match self.selection {
            Selection::Rect(r) => match r.intersect(&self.doc.canvas_rect()) {
                None => Selection::Empty,
                Some(r) => {
                    let p = origin(t, self.doc.canvas, r)?;
                    let (w, h) = t.dimensions(r.width() as u32, r.height() as u32);
                    Selection::Rect(Rect::from_origin_size(p.0, p.1, w, h).unwrap())
                }
            },
            other => other,
        };
        self.doc.canvas = t.dimensions(self.doc.canvas.0, self.doc.canvas.1);
        Ok(())
    }
    pub(super) fn transform_layout(&mut self, t: OrthogonalTransform) -> Result<()> {
        for layer in self.doc.layers.values_mut() {
            (self.check)()?;
            layer.origin = origin(t, self.doc.canvas, layer.extent())?;
            layer.raster = raster(t, &layer.raster);
        }
        self.transform_selection(t)
    }
    pub(super) fn transform_canvas(&mut self, t: OrthogonalTransform) -> Result<()> {
        for id in self.doc.roots.clone() {
            (self.check)()?;
            let layer = &self.doc.layers[&id];
            let p = origin(t, self.doc.canvas, layer.extent())?;
            let target = raster(t, &layer.raster);
            let planning = self
                .planning
                .as_mut()
                .expect("transforms compile before replay");
            let source = planning.snapshot(id, &layer.raster, &self.adm, self.check)?;
            let plan = Arc::new(MappedPlan::prepare_transform(
                source,
                t,
                &self.adm,
                READS_MAX - self.transform_reads,
                self.check,
            )?);
            self.transform_reads += plan.source_reads();
            planning.replace_mapping(id, &target, plan, self.operation, &self.adm, self.check)?;
            let layer = self.doc.layers.get_mut(&id).unwrap();
            layer.origin = p;
            layer.raster = target;
        }
        self.transform_selection(t)
    }
}
