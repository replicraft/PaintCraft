//! Immediate rectangular pixel transforms on one captured target layer.
use super::*;
use paintcraft_core::types::{
    orthogonal::OrthogonalTransform as Transform, policy::CANVAS_SIDE_MAX,
};
use paintcraft_raster::MappedPlan;
use std::sync::Arc;
struct Geometry {
    source: Rect,
    destination: Rect,
    origin: (i64, i64),
    size: (u32, u32),
}
impl Stage<'_> {
    fn selection_geometry(&self, id: Uuid, t: Transform) -> Result<Option<Geometry>> {
        let area = match self.selection {
            Selection::Inactive => {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "selection transform requires an active selection",
                ));
            }
            Selection::Empty => return Ok(None),
            Selection::Mask(_) => {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "mask selection transforms are not implemented yet",
                ));
            }
            Selection::Rect(r) => match r.intersect(&self.doc.canvas_rect()) {
                Some(r) => r,
                None => return Ok(None),
            },
        };
        let l = &self.doc.layers[&id];
        let old = l.extent();
        let (w, h) = t.dimensions(area.width() as u32, area.height() as u32);
        let destination = Rect::from_origin_size(area.x0, area.y0, w, h).unwrap();
        let origin = (old.x0.min(destination.x0), old.y0.min(destination.y0));
        let width = old.x1.max(destination.x1) - origin.0;
        let height = old.y1.max(destination.y1) - origin.1;
        if width > i64::from(CANVAS_SIDE_MAX)
            || height > i64::from(CANVAS_SIDE_MAX)
            || origin.0.unsigned_abs() > COORD_ABS_MAX as u64
            || origin.1.unsigned_abs() > COORD_ABS_MAX as u64
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "selection transform exceeds layer extent policy",
            ));
        }
        Ok(Some(Geometry {
            source: area.translated(-l.origin.0, -l.origin.1).unwrap(),
            destination,
            origin,
            size: (width as u32, height as u32),
        }))
    }
    fn install_selection_geometry(&mut self, id: Uuid, g: &Geometry) {
        let l = self.doc.layers.get_mut(&id).unwrap();
        l.raster = RasterLayer::new(g.size.0, g.size.1, l.raster.default_pixel());
        l.origin = g.origin;
        self.selection = g
            .destination
            .intersect(&self.doc.canvas_rect())
            .map_or(Selection::Empty, Selection::Rect);
    }
    pub(super) fn selection_transform_layout(
        &mut self,
        layer: &LayerRef,
        t: Transform,
    ) -> Result<()> {
        let id = self.resolve(layer)?;
        if let Some(g) = self.selection_geometry(id, t)? {
            self.install_selection_geometry(id, &g);
        }
        Ok(())
    }
    pub(super) fn transform_selected(
        &mut self,
        layer: &LayerRef,
        t: Transform,
        clear: Rgba,
    ) -> Result<()> {
        let id = self.resolve(layer)?;
        let Some(g) = self.selection_geometry(id, t)? else {
            return Ok(());
        };
        let l = &self.doc.layers[&id];
        let planning = self
            .planning
            .as_mut()
            .expect("selection transforms compile before replay");
        let source = planning.snapshot(id, &l.raster, &self.adm, self.check)?;
        let (plan, offset) = MappedPlan::prepare_patch(
            source,
            g.source,
            t,
            clear,
            &self.adm,
            16_777_216 - self.transform_reads,
            self.check,
        )?;
        if plan.dimensions() != g.size || (l.origin.0 + offset.0, l.origin.1 + offset.1) != g.origin
        {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "selection patch layout differs from its compiled geometry",
            ));
        }
        self.transform_reads += plan.source_reads();
        let target = RasterLayer::new(g.size.0, g.size.1, l.raster.default_pixel());
        planning.replace_mapping(
            id,
            &target,
            Arc::new(plan),
            self.operation,
            &self.adm,
            self.check,
        )?;
        self.install_selection_geometry(id, &g);
        Ok(())
    }
}
