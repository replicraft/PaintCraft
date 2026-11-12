//! Compile bounded source-bound resampling, then replay only the admitted plans.
use super::*;
use paintcraft_core::types::stretch_skew::StretchSkew;
use paintcraft_raster::selection::{BinaryMask, MaskLimits};
use paintcraft_raster::{MappedPlan, StretchSkewMapping};
fn bounds_limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Stretch and skew layer extent limit",
    )
}
impl Stage<'_> {
    fn resample_selection(&mut self, g: StretchSkew, whole: bool) -> Result<Selection> {
        let source = match &self.selection {
            Selection::Inactive => return Ok(Selection::Inactive),
            Selection::Empty => return Ok(Selection::Empty),
            Selection::Rect(r) => BinaryMask::uniform(*r, true)?,
            Selection::Mask(m) => m.clone(),
        };
        let mask = source.stretch_skew_with_remaining_checked(
            g,
            &self.adm.selection_budget(),
            MaskLimits::default(),
            &mut self.selection_work,
            self.check,
        )?;
        let canvas = if whole {
            g.bounds(g.source())?
        } else {
            self.doc.canvas_rect()
        };
        Ok(mask
            .clipped(canvas)?
            .map_or(Selection::Empty, Selection::Mask))
    }
    pub(super) fn stretch_skew_canvas(
        &mut self,
        size: (u32, u32),
        shear: (i32, i32),
    ) -> Result<()> {
        let g = StretchSkew::new(self.doc.canvas_rect(), size, shear)?;
        if g.is_identity() {
            return Ok(());
        }
        for id in &self.doc.roots {
            self.refuse_noise_geometry(*id)?;
        }
        let selection = if self.planning.is_some() {
            let selected = self.resample_selection(g, true)?;
            self.selections.push((self.operation, selected.clone()))?;
            selected
        } else {
            self.selections
                .values
                .iter()
                .find(|(n, _)| *n == self.operation)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvariantViolation,
                        "Compiled resampled selection missing",
                    )
                })?
                .1
                .clone()
        };
        let mut masks = self.mask_mapping_counts(|_| true)?;
        for id in self.doc.roots.clone() {
            (self.check)()?;
            let old = self.doc.layers[&id].extent();
            let destination = g.bounds(old)?;
            self.map_filter_masks(id, filter_masks::Mapping::StretchSkew(g, old), &mut masks)?;
            let old_raster = &self.doc.layers[&id].raster;
            let target = RasterLayer::new(
                destination.width() as u32,
                destination.height() as u32,
                old_raster.default_pixel(),
            );
            if let Some(planning) = &mut self.planning {
                let source = planning.snapshot(id, old_raster, &self.adm, self.check)?;
                let request = StretchSkewMapping {
                    transform: g,
                    source_origin: (old.x0, old.y0),
                    destination,
                    selection: None,
                    clear: Rgba::TRANSPARENT,
                };
                let plan = MappedPlan::prepare_stretch_skew(
                    source,
                    request,
                    &self.adm,
                    16_777_216 - self.transform_reads,
                    self.check,
                )?;
                self.transform_reads += plan.source_reads();
                planning.replace_mapping(
                    id,
                    &target,
                    plan,
                    self.operation,
                    &self.adm,
                    self.check,
                )?;
            }
            let layer = self.doc.layers.get_mut(&id).expect("rooted layer");
            layer.origin = (destination.x0, destination.y0);
            layer.raster = target;
        }
        let canvas = g.bounds(g.source())?;
        self.doc.canvas = (canvas.width() as u32, canvas.height() as u32);
        self.selection = selection;
        Ok(())
    }
    pub(super) fn stretch_skew_selected(
        &mut self,
        layer: &LayerRef,
        size: (u32, u32),
        shear: (i32, i32),
        clear: Rgba,
    ) -> Result<()> {
        let id = self.resolve(layer)?;
        if self.planning.is_none() {
            return self.selection_geometry_layout(layer);
        }
        let Some(area) = self.selected_area()? else {
            self.selection_geometries.push((self.operation, None))?;
            return Ok(());
        };
        let g = StretchSkew::new(area, size, shear)?;
        if g.is_identity() {
            self.selection_geometries.push((self.operation, None))?;
            return Ok(());
        }
        self.refuse_noise_geometry(id)?;
        let old = self.doc.layers[&id].extent();
        let mapped = g.bounds(area)?;
        let destination = Rect {
            x0: old.x0.min(mapped.x0),
            y0: old.y0.min(mapped.y0),
            x1: old.x1.max(mapped.x1),
            y1: old.y1.max(mapped.y1),
        };
        use paintcraft_core::types::policy::CANVAS_SIDE_MAX;
        if destination.width() > i64::from(CANVAS_SIDE_MAX)
            || destination.height() > i64::from(CANVAS_SIDE_MAX)
        {
            return Err(bounds_limit());
        }
        let selection = self.resample_selection(g, false)?;
        let geometry = selection_transform::Geometry {
            source: area,
            origin: (destination.x0, destination.y0),
            size: (destination.width() as u32, destination.height() as u32),
            selection,
        };
        let request = StretchSkewMapping {
            transform: g,
            source_origin: (old.x0, old.y0),
            destination,
            selection: Some(self.selection.clone()),
            clear,
        };
        let l = &self.doc.layers[&id];
        let target = RasterLayer::new(geometry.size.0, geometry.size.1, l.raster.default_pixel());
        let planning = self.planning.as_mut().expect("compiler");
        let source = planning.snapshot(id, &l.raster, &self.adm, self.check)?;
        let plan = MappedPlan::prepare_stretch_skew(
            source,
            request,
            &self.adm,
            16_777_216 - self.transform_reads,
            self.check,
        )?;
        self.transform_reads += plan.source_reads();
        planning.replace_mapping(id, &target, plan, self.operation, &self.adm, self.check)?;
        self.selection_geometries
            .push((self.operation, Some(geometry.clone())))?;
        self.install_selection_geometry(id, &geometry)
    }
}
