//! Simulate private tile ownership before allocating decoded batch output.
use super::*;
use std::{mem::size_of, sync::Mutex};

/// Counts new decoded versions only; captured immutable inputs remain separately
/// charged in their original realms. Counts are not allocator/RSS measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileFootprint {
    pub live: u64,
    pub peak: u64,
    pub allocated: u64,
    pub peak_operation: usize,
}
#[derive(Debug)]
struct Trace {
    footprint: Mutex<TileFootprint>,
    admission: Admission,
    _charge: BudgetAccount,
}
#[derive(Debug)]
struct Version {
    trace: Arc<Trace>,
    _charge: BudgetAccount,
}
impl Drop for Version {
    fn drop(&mut self) {
        self.trace
            .footprint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .live -= 1;
    }
}
#[derive(Clone, Debug)]
enum Ownership {
    Base,
    Private(Arc<Version>),
}
/// One private batch's allocation model. Abort all its staging after an error.
/// Simulated layers duplicate versions with COW and release them on deletion.
#[derive(Debug)]
pub struct TilePlanner {
    trace: Arc<Trace>,
}
#[derive(Debug)]
pub struct SimulatedLayer {
    bounds: Rect,
    default: Rgba,
    tiles: HashMap<TileCoord, Ownership>,
    trace: Arc<Trace>,
    charge: BudgetAccount,
}
fn charge<T>(admission: &Admission) -> Result<BudgetAccount> {
    metadata::Metadata::new(size_of::<T>(), admission.paint_metadata_limit)?;
    let mut charge = admission.paint_metadata_account();
    charge.resize(size_of::<T>() as u64)?;
    Ok(charge)
}
impl TilePlanner {
    pub fn new(admission: &Admission) -> Result<Self> {
        let charge = charge::<Trace>(admission)?;
        Ok(Self {
            trace: Arc::new(Trace {
                footprint: Mutex::new(TileFootprint::default()),
                admission: admission.clone(),
                _charge: charge,
            }),
        })
    }
    pub fn footprint(&self) -> TileFootprint {
        *self
            .trace
            .footprint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
    pub fn layer(
        &self,
        source: &RasterLayer,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<SimulatedLayer> {
        check()?;
        let mut layer = SimulatedLayer {
            bounds: source.bounds(),
            default: source.default,
            tiles: HashMap::new(),
            trace: self.trace.clone(),
            charge: self.trace.admission.paint_metadata_account(),
        };
        let mut metadata = metadata::Metadata::new(0, self.trace.admission.paint_metadata_limit)?;
        for &coord in source.tiles.keys() {
            check()?;
            metadata.table(&mut layer.tiles, &mut layer.charge)?;
            layer.tiles.insert(coord, Ownership::Base);
        }
        Ok(layer)
    }
}
impl Trace {
    fn allocate(self: &Arc<Self>, operation: usize) -> Result<Arc<Version>> {
        // Admission callbacks may inspect footprint. Never retain its lock while
        // acquiring/releasing metadata or invoking caller cancellation checks.
        let charge = charge::<Version>(&self.admission)?;
        {
            let mut f = self.footprint.lock().unwrap_or_else(|e| e.into_inner());
            let allocated = f.allocated.checked_add(1).ok_or_else(metadata::exhausted)?;
            self.admission.planned_bytes(allocated)?;
            f.allocated = allocated;
            f.live += 1;
            if f.live > f.peak {
                f.peak = f.live;
                f.peak_operation = operation;
            }
        }
        Ok(Arc::new(Version {
            trace: self.clone(),
            _charge: charge,
        }))
    }
}
impl SimulatedLayer {
    pub fn resident_tiles(&self) -> usize {
        self.tiles.len()
    }
    pub fn duplicate_checked(&self, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        check()?;
        let mut layer = Self {
            bounds: self.bounds,
            default: self.default,
            tiles: HashMap::new(),
            trace: self.trace.clone(),
            charge: self.trace.admission.paint_metadata_account(),
        };
        let mut metadata = metadata::Metadata::new(0, self.trace.admission.paint_metadata_limit)?;
        for (&coord, state) in &self.tiles {
            check()?;
            metadata.table(&mut layer.tiles, &mut layer.charge)?;
            layer.tiles.insert(coord, state.clone());
        }
        Ok(layer)
    }
    pub fn expand_to(&mut self, width: u32, height: u32) -> Result<()> {
        let mut extent = RasterLayer::new(
            self.bounds.width() as u32,
            self.bounds.height() as u32,
            self.default,
        );
        extent.expand_to(width, height)?;
        self.bounds = extent.bounds();
        Ok(())
    }
    /// Model exactly the operation's sparse/default indexing and private/COW
    /// rules. Failure can alter this private model; discard the whole attempt.
    /// Returns whether the operation has indexed writes, not whether pixels differ.
    pub fn apply_checked(
        &mut self,
        plan: &PaintPlan,
        operation: usize,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<bool> {
        let trace = self.trace.clone();
        let index = plan.index_checked(
            trace.admission.paint_metadata_limit,
            trace.admission.paint_metadata_account(),
            &mut Target {
                layer: self,
                operation,
            },
            check,
        )?;
        check()?;
        Ok(!index.tiles.is_empty())
    }
}
struct Target<'a> {
    layer: &'a mut SimulatedLayer,
    operation: usize,
}
impl index::Target for Target<'_> {
    fn bounds(&self) -> Rect {
        self.layer.bounds
    }
    fn skip_empty(&self, write: Write) -> bool {
        self.layer.tiles.is_empty()
            && write.mode == BlendMode::Replace
            && write.samples == 16
            && write.color == self.layer.default
    }
    fn skip_missing(&self, coord: TileCoord, write: Write) -> Result<bool> {
        Ok(!self.layer.tiles.contains_key(&coord)
            && write.pixel(self.layer.default) == self.layer.default)
    }
    fn admit_tile(&mut self, coord: TileCoord) -> Result<()> {
        if matches!(self.layer.tiles.get(&coord),Some(Ownership::Private(version)) if Arc::strong_count(version)==1)
        {
            return Ok(());
        }
        let version = self.layer.trace.allocate(self.operation)?;
        if !self.layer.tiles.contains_key(&coord) {
            let mut metadata = metadata::Metadata::new(
                self.layer.charge.bytes() as usize,
                self.layer.trace.admission.paint_metadata_limit,
            )?;
            metadata.table(&mut self.layer.tiles, &mut self.layer.charge)?;
        }
        self.layer.tiles.insert(coord, Ownership::Private(version));
        Ok(())
    }
}
