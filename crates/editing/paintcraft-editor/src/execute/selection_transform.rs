//! Immediate exact selected-pixel transforms on one captured target layer.
use super::*;
use paintcraft_core::types::{
    orthogonal::OrthogonalTransform as Transform, policy::CANVAS_SIDE_MAX,
};
use paintcraft_raster::MappedPlan;
use std::sync::Arc;
#[derive(Clone)]
pub(super) struct Geometry {
    pub(super) source: Rect,
    pub(super) origin: (i64, i64),
    pub(super) size: (u32, u32),
    pub(super) selection: Selection,
}
impl Stage<'_> {
    pub(super) fn selected_area(&mut self) -> Result<Option<Rect>> {
        match &self.selection {
            Selection::Inactive => Err(Error::new(
                ErrorCode::InputSchema,
                "selection operation requires an active selection",
            )),
            Selection::Empty => Ok(None),
            Selection::Mask(mask) => mask.bounds_with_remaining_checked(
                self.doc.canvas_rect(),
                &mut self.selection_work,
                self.check,
            ),
            Selection::Rect(rect) => Ok(rect.intersect(&self.doc.canvas_rect())),
        }
    }

    fn selection_geometry(&mut self, id: Uuid, t: Transform) -> Result<Option<Geometry>> {
        let Some(area) = self.selected_area()? else {
            return Ok(None);
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
        let selection = if let Selection::Mask(mask) = &self.selection {
            let mapped = mask.transformed_with_remaining_checked(
                area,
                t,
                &self.adm.selection_budget(),
                paintcraft_raster::selection::MaskLimits::default(),
                &mut self.selection_work,
                self.check,
            )?;
            mapped
                .clipped(self.doc.canvas_rect())?
                .map_or(Selection::Empty, Selection::Mask)
        } else {
            destination
                .intersect(&self.doc.canvas_rect())
                .map_or(Selection::Empty, Selection::Rect)
        };
        Ok(Some(Geometry {
            source: area.translated(-l.origin.0, -l.origin.1).unwrap(),
            origin,
            size: (width as u32, height as u32),
            selection,
        }))
    }
    pub(super) fn install_selection_geometry(&mut self, id: Uuid, g: &Geometry) -> Result<()> {
        let origin = self.doc.layers[&id].origin;
        self.map_filter_masks(
            id,
            filter_masks::Mapping::Rebase {
                offset: (origin.0 - g.origin.0, origin.1 - g.origin.1),
            },
        )?;
        let l = self.doc.layers.get_mut(&id).unwrap();
        l.raster = RasterLayer::new(g.size.0, g.size.1, l.raster.default_pixel());
        l.origin = g.origin;
        self.selection = g.selection.clone();
        Ok(())
    }
    pub(super) fn selection_geometry_layout(&mut self, layer: &LayerRef) -> Result<()> {
        let id = self.resolve(layer)?;
        let geometry = self
            .selection_geometries
            .values
            .iter()
            .find(|(index, _)| *index == self.operation)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "compiled selection geometry was not retained",
                )
            })?
            .1
            .clone();
        if let Some(g) = geometry {
            self.install_selection_geometry(id, &g)?;
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
        let geometry = self.selection_geometry(id, t)?;
        self.selection_geometries
            .push((self.operation, geometry.clone()))?;
        let Some(g) = geometry else {
            return Ok(());
        };
        let l = &self.doc.layers[&id];
        let planning = self
            .planning
            .as_mut()
            .expect("selection transforms compile before replay");
        let source = planning.snapshot(id, &l.raster, &self.adm, self.check)?;
        let (plan, offset) = if let Selection::Mask(mask) = &self.selection {
            let local = mask.translated(-l.origin.0, -l.origin.1)?;
            MappedPlan::prepare_masked_patch(
                source,
                g.source,
                local,
                t,
                clear,
                &self.adm,
                16_777_216 - self.transform_reads,
                self.check,
            )?
        } else {
            MappedPlan::prepare_patch(
                source,
                g.source,
                t,
                clear,
                &self.adm,
                16_777_216 - self.transform_reads,
                self.check,
            )?
        };
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
        self.install_selection_geometry(id, &g)?;
        Ok(())
    }
}
