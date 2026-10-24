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
    needs_pixels: bool,
}
impl Compiler {
    pub fn new(
        admission: &Admission,
        batch: &Batch,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut needs_pixels = false;
        for operations in batch.operations.chunks(1024) {
            check()?;
            if operations.iter().any(|op| kind(op) == Kind::ReadWrite) {
                needs_pixels = true;
                break;
            }
        }
        Ok(Self {
            layers: Store::new(admission),
            paints: Store::new(admission),
            planner: None,
            needs_pixels,
        })
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
        // Share only immutable descriptor storage. Keep every replay record and
        // source-program append: equal translucent operations must blend again.
        // All plans in this compiler use the same admission realm/policy. Avoid
        // replacing a smaller freshly compiled payload with a larger capacity.
        let retained = if let Some(previous) = self.paints.values.last()
            && previous.plan.descriptor_bytes() <= plan.descriptor_bytes()
            && previous.plan.same_writes_checked(plan, check)?
        {
            previous.plan.clone()
        } else {
            plan.clone()
        };
        if self.needs_pixels && logical.pixels.is_none() {
            logical.pixels =
                Some(PaintSnapshot::capture(source, admission, check)?.fork(admission, check)?);
        }
        if let Some(pixels) = logical.pixels.take() {
            logical.pixels = Some(pixels.append_checked(retained.clone(), check)?);
        }
        self.paints.push(Paint {
            operation,
            layer: id,
            plan: retained,
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
        if !self.needs_pixels {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "source read absent from batch dependency plan",
            ));
        }
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Structural,
    Write,
    ReadWrite,
}
fn kind(op: &Op) -> Kind {
    match op {
        Op::FillRect { .. }
        | Op::PencilPath { .. }
        | Op::StrokePath { .. }
        | Op::Shape { .. }
        | Op::Contour { .. }
        | Op::MaskSpans { .. }
        | Op::Text { .. } => Kind::Write,
        Op::FloodFill { .. } => Kind::ReadWrite,
        Op::LayerAdd { .. }
        | Op::LayerDuplicate { .. }
        | Op::LayerDelete { .. }
        | Op::LayerSetProperties { .. }
        | Op::LayerReorder { .. }
        | Op::LayerExpand { .. }
        | Op::CanvasClear { .. }
        | Op::CanvasResize { .. }
        | Op::SelectionSetRect { .. }
        | Op::SelectionClear => Kind::Structural,
    }
}
pub(super) fn is_paint(op: &Op) -> bool {
    kind(op) != Kind::Structural
}
