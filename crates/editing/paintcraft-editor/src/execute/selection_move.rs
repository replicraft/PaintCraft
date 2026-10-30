//! Immediate integer cut/paste from a frozen selected source on one layer.
use super::selection_transform::Geometry;
use super::*;
use paintcraft_core::types::policy::CANVAS_SIDE_MAX;
use paintcraft_raster::MappedPlan;
use std::sync::Arc;

impl Stage<'_> {
    fn moved_geometry(&mut self, id: Uuid, offset: (i64, i64)) -> Result<Option<Geometry>> {
        let Some(area) = self.selected_area()? else {
            return Ok(None);
        };
        let destination = area.translated(offset.0, offset.1).ok_or_else(move_limit)?;
        if [
            destination.x0,
            destination.x1,
            destination.y0,
            destination.y1,
        ]
        .iter()
        .any(|value| value.unsigned_abs() > COORD_ABS_MAX as u64)
        {
            return Err(move_limit());
        }
        let layer = &self.doc.layers[&id];
        let old = layer.extent();
        let origin = (old.x0.min(destination.x0), old.y0.min(destination.y0));
        let width = old.x1.max(destination.x1) - origin.0;
        let height = old.y1.max(destination.y1) - origin.1;
        if width > i64::from(CANVAS_SIDE_MAX)
            || height > i64::from(CANVAS_SIDE_MAX)
            || origin.0.unsigned_abs() > COORD_ABS_MAX as u64
            || origin.1.unsigned_abs() > COORD_ABS_MAX as u64
        {
            return Err(move_limit());
        }
        let selection = match &self.selection {
            Selection::Mask(mask) => mask
                .clipped(area)?
                .expect("tight selected area intersects field")
                .translated(offset.0, offset.1)?
                .clipped(self.doc.canvas_rect())?
                .map_or(Selection::Empty, Selection::Mask),
            Selection::Rect(_) => destination
                .intersect(&self.doc.canvas_rect())
                .map_or(Selection::Empty, Selection::Rect),
            _ => unreachable!("selected area was nonempty"),
        };
        Ok(Some(Geometry {
            source: area
                .translated(-layer.origin.0, -layer.origin.1)
                .ok_or_else(move_limit)?,
            origin,
            size: (width as u32, height as u32),
            selection,
        }))
    }
    pub(super) fn selection_move_layout(
        &mut self,
        layer: &LayerRef,
        offset: (i64, i64),
    ) -> Result<()> {
        if offset == (0, 0) {
            return Ok(());
        }
        self.selection_geometry_layout(layer)
    }
    pub(super) fn move_selected(
        &mut self,
        layer: &LayerRef,
        offset: (i64, i64),
        clear: Rgba,
        key: Option<Rgba>,
    ) -> Result<()> {
        let id = self.resolve(layer)?;
        if matches!(self.selection, Selection::Inactive) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "selection movement requires an active selection",
            ));
        }
        if offset.0.unsigned_abs() > COORD_ABS_MAX as u64
            || offset.1.unsigned_abs() > COORD_ABS_MAX as u64
        {
            return Err(move_limit());
        }
        // No source capture, mask reconstruction or geometry retention, even if
        // keyed samples would otherwise be removed by cut/paste at the same place.
        if offset == (0, 0) {
            return Ok(());
        }
        let geometry = self.moved_geometry(id, offset)?;
        self.selection_geometries
            .push((self.operation, geometry.clone()))?;
        let Some(geometry) = geometry else {
            return Ok(());
        };
        let layer = &self.doc.layers[&id];
        let mask = match &self.selection {
            Selection::Mask(mask) => Some(mask.translated(-layer.origin.0, -layer.origin.1)?),
            _ => None,
        };
        let planning = self
            .planning
            .as_mut()
            .expect("selection moves compile before replay");
        let source = planning.snapshot(id, &layer.raster, &self.adm, self.check)?;
        let (plan, local_origin) = MappedPlan::prepare_moved_patch(
            source,
            geometry.source,
            mask,
            offset,
            clear,
            key,
            &self.adm,
            16_777_216 - self.transform_reads,
            self.check,
        )?;
        if plan.dimensions() != geometry.size
            || (
                layer.origin.0 + local_origin.0,
                layer.origin.1 + local_origin.1,
            ) != geometry.origin
        {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "selected move layout differs from its compiled geometry",
            ));
        }
        self.transform_reads += plan.source_reads();
        let target = RasterLayer::new(
            geometry.size.0,
            geometry.size.1,
            layer.raster.default_pixel(),
        );
        planning.replace_mapping(
            id,
            &target,
            Arc::new(plan),
            self.operation,
            &self.adm,
            self.check,
        )?;
        self.install_selection_geometry(id, &geometry)?;
        Ok(())
    }
}
fn move_limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "selection movement exceeds layer extent policy",
    )
}
