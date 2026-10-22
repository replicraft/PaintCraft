//! Compile logical paint and model complete private decoded ownership before replay.
use super::*;
use paintcraft_raster::{PaintOverlay, PaintSnapshot, SimulatedLayer, TileFootprint, TilePlanner};
mod storage;
use storage::Store;

#[derive(Debug)]
pub(super) struct Paint {
    pub operation: usize,
    pub layer: Uuid,
    pub plan: PaintPlan,
}
#[derive(Debug)]
struct Logical {
    pixels: Option<PaintOverlay>,
    tiles: SimulatedLayer,
}
#[derive(Debug)]
pub(super) struct Compiler {
    layers: Store<(Uuid, Logical)>,
    paints: Store<Paint>,
    planner: Option<TilePlanner>,
}
impl Compiler {
    pub fn new(admission: &Admission) -> Self {
        Self {
            layers: Store::new(admission),
            paints: Store::new(admission),
            planner: None,
        }
    }
    fn position(&self, id: Uuid) -> Option<usize> {
        self.layers.values.iter().position(|(key, _)| *key == id)
    }
    fn ensure(
        &mut self,
        id: Uuid,
        source: &RasterLayer,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<usize> {
        if let Some(index) = self.position(id) {
            return Ok(index);
        }
        if self.planner.is_none() {
            self.planner = Some(TilePlanner::new(admission)?);
        }
        let tiles = self
            .planner
            .as_ref()
            .expect("created planner")
            .layer(source, check)?;
        let pixels = None;
        self.layers.push((id, Logical { pixels, tiles }))?;
        Ok(self.layers.values.len() - 1)
    }
    pub fn paint(
        &mut self,
        id: Uuid,
        source: &RasterLayer,
        plan: &PaintPlan,
        operation: usize,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if plan.is_empty() {
            return Ok(());
        }
        let index = self.ensure(id, source, admission, check)?;
        let logical = &mut self.layers.values[index].1;
        if !logical.tiles.apply_checked(plan, operation, check)? {
            return Ok(());
        }
        if logical.pixels.is_none() {
            logical.pixels =
                Some(PaintSnapshot::capture(source, admission, check)?.fork(admission, check)?);
        }
        logical.pixels = Some(
            logical
                .pixels
                .take()
                .expect("active logical branch")
                .append_checked(plan.clone(), check)?,
        );
        self.paints.push(Paint {
            operation,
            layer: id,
            plan: plan.clone(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn flood(
        &mut self,
        id: Uuid,
        source: &RasterLayer,
        seed: (i64, i64),
        area: Rect,
        tolerance: u8,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<Span>> {
        let index = self.ensure(id, source, admission, check)?;
        let Some(pixels) = self.layers.values[index].1.pixels.as_ref() else {
            return paintcraft_raster::flood::region_with_scratch_limit_checked(
                source,
                seed,
                &area,
                tolerance,
                admission.flood_scratch_limit(),
                check,
            );
        };
        let reader = pixels.reader(admission, check)?;
        paintcraft_raster::flood::region_from_source_checked(
            |x, y| reader.read_checked(x, y, check),
            seed,
            &area,
            tolerance,
            admission.flood_scratch_limit(),
            check,
        )
    }
    pub fn duplicate(
        &mut self,
        source: Uuid,
        id: Uuid,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let Some(index) = self.position(source) else {
            return Ok(());
        };
        let logical = &mut self.layers.values[index].1;
        let pixels = if let Some(pixels) = logical.pixels.take() {
            let snapshot = pixels.freeze_checked(admission, check)?;
            logical.pixels = Some(snapshot.fork(admission, check)?);
            Some(snapshot.fork(admission, check)?)
        } else {
            None
        };
        let copy = Logical {
            pixels,
            tiles: logical.tiles.duplicate_checked(check)?,
        };
        self.layers.push((id, copy))
    }
    pub fn remove(&mut self, id: Uuid) {
        if let Some(index) = self.position(id) {
            self.layers.values.swap_remove(index);
        }
    }
    pub fn clear(&mut self) {
        self.layers.values.clear();
    }
    pub fn resident_tiles(&self, id: Uuid, source: &RasterLayer) -> usize {
        self.position(id).map_or_else(
            || source.resident_tiles(),
            |i| self.layers.values[i].1.tiles.resident_tiles(),
        )
    }
    pub fn expand(
        &mut self,
        id: Uuid,
        width: u32,
        height: u32,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let Some(index) = self.position(id) else {
            return Ok(());
        };
        let logical = &mut self.layers.values[index].1;
        logical.tiles.expand_to(width, height)?;
        if let Some(pixels) = logical.pixels.take() {
            let snapshot = pixels.freeze_checked(admission, check)?;
            logical.pixels = Some(snapshot.fork_expanded(width, height, admission, check)?);
        }
        Ok(())
    }
    pub fn finish(self) -> (Store<Paint>, TileFootprint) {
        let footprint = self
            .planner
            .as_ref()
            .map_or_else(TileFootprint::default, TilePlanner::footprint);
        (self.paints, footprint)
    }
}
pub(super) fn is_paint(op: &Op) -> bool {
    match op {
        Op::FillRect { .. }
        | Op::PencilPath { .. }
        | Op::StrokePath { .. }
        | Op::FloodFill { .. }
        | Op::Shape { .. }
        | Op::MaskSpans { .. }
        | Op::Text { .. } => true,
        Op::LayerAdd { .. }
        | Op::LayerDuplicate { .. }
        | Op::LayerDelete { .. }
        | Op::LayerSetProperties { .. }
        | Op::LayerReorder { .. }
        | Op::LayerExpand { .. }
        | Op::CanvasClear { .. }
        | Op::CanvasResize { .. }
        | Op::SelectionSetRect { .. }
        | Op::SelectionClear => false,
    }
}
