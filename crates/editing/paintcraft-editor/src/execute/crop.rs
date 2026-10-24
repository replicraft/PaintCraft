//! Destructive canvas crop with compiled, source-bound sparse pixel mappings.
use super::*;
use paintcraft_raster::CropPlan;
use std::sync::Arc;
const READS_MAX: u64 = 16_777_216;

enum LayerCrop {
    Keep,
    Empty,
    Clip(Rect),
}
fn mapping(layer: &Layer, rect: Rect) -> (LayerCrop, (i64, i64)) {
    match layer.extent().intersect(&rect) {
        None => (LayerCrop::Empty, (0, 0)),
        Some(intersection) => {
            let origin = (intersection.x0 - rect.x0, intersection.y0 - rect.y0);
            if intersection == layer.extent() {
                (LayerCrop::Keep, origin)
            } else {
                (
                    LayerCrop::Clip(
                        Rect::new(
                            intersection.x0 - layer.origin.0,
                            intersection.y0 - layer.origin.1,
                            intersection.width(),
                            intersection.height(),
                        )
                        .unwrap(),
                    ),
                    origin,
                )
            }
        }
    }
}
fn clipped_raster(source: &RasterLayer, area: Rect) -> RasterLayer {
    RasterLayer::new(
        area.width() as u32,
        area.height() as u32,
        source.default_pixel(),
    )
}
impl Stage<'_> {
    fn validate_crop(&self, rect: Rect) -> Result<()> {
        if self.doc.canvas_rect().intersect(&rect) != Some(rect)
            || rect.x0 >= rect.x1
            || rect.y0 >= rect.y1
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "crop rectangle must be inside the canvas",
            ));
        }
        Ok(())
    }
    pub(super) fn crop_layout(&mut self, rect: Rect) -> Result<()> {
        self.validate_crop(rect)?;
        for layer in self.doc.layers.values_mut() {
            (self.check)()?;
            let (action, origin) = mapping(layer, rect);
            match action {
                LayerCrop::Keep => {}
                LayerCrop::Empty => layer.raster = RasterLayer::new(1, 1, Rgba::TRANSPARENT),
                LayerCrop::Clip(area) => layer.raster = clipped_raster(&layer.raster, area),
            }
            layer.origin = origin;
        }
        self.doc.canvas = (rect.width() as u32, rect.height() as u32);
        self.selection = Selection::Inactive;
        Ok(())
    }
    pub(super) fn crop_canvas(&mut self, rect: Rect) -> Result<()> {
        self.validate_crop(rect)?;
        for id in self.doc.roots.clone() {
            (self.check)()?;
            let layer = &self.doc.layers[&id];
            let (action, origin) = mapping(layer, rect);
            match action {
                LayerCrop::Keep => {}
                LayerCrop::Empty => {
                    if let Some(planning) = &mut self.planning {
                        planning.remove(id);
                    }
                    self.doc.layers.get_mut(&id).unwrap().raster =
                        RasterLayer::new(1, 1, Rgba::TRANSPARENT);
                }
                LayerCrop::Clip(area) => {
                    let planning = self.planning.as_mut().expect("crops compile before replay");
                    let source = planning.snapshot(id, &layer.raster, &self.adm, self.check)?;
                    let plan = Arc::new(CropPlan::prepare_limited(
                        source,
                        area,
                        &self.adm,
                        READS_MAX - self.transform_reads,
                        self.check,
                    )?);
                    self.transform_reads += plan.source_reads();
                    let target = clipped_raster(&layer.raster, area);
                    planning.replace_crop(
                        id,
                        &target,
                        plan,
                        self.operation,
                        &self.adm,
                        self.check,
                    )?;
                    self.doc.layers.get_mut(&id).unwrap().raster = target;
                }
            }
            self.doc.layers.get_mut(&id).unwrap().origin = origin;
        }
        self.doc.canvas = (rect.width() as u32, rect.height() as u32);
        self.selection = Selection::Inactive;
        Ok(())
    }
}
