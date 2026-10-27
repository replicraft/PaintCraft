//! One ordered tile index for replay and read-only transaction projection.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;

pub(super) struct Index {
    pub tiles: HashMap<TileCoord, Vec<usize>>,
    pub bytes: usize,
    pub charge: BudgetAccount,
}

pub(super) trait Target {
    fn bounds(&self) -> Rect;
    fn skip_empty(&self, write: Write) -> bool;
    fn skip_missing(&self, coord: TileCoord, write: Write) -> Result<bool>;
    fn admit_tile(&mut self, coord: TileCoord) -> Result<()>;
}

pub(super) struct RasterTarget<'a> {
    layer: &'a mut RasterLayer,
    admission: &'a Admission,
    pub needed: u64,
}
impl<'a> RasterTarget<'a> {
    pub fn new(layer: &'a mut RasterLayer, admission: &'a Admission) -> Self {
        Self {
            layer,
            admission,
            needed: 0,
        }
    }
}
impl Target for RasterTarget<'_> {
    fn bounds(&self) -> Rect {
        self.layer.bounds()
    }
    fn skip_empty(&self, write: Write) -> bool {
        self.layer.tiles.is_empty()
            && write.mode == BlendMode::Replace
            && !write.destination_out
            && write.samples == write.sample_scale
            && write.color == self.layer.default
    }
    fn skip_missing(&self, coord: TileCoord, write: Write) -> Result<bool> {
        Ok(!self.layer.tiles.contains_key(&coord)
            && write.pixel(self.layer.default) == self.layer.default)
    }
    fn admit_tile(&mut self, coord: TileCoord) -> Result<()> {
        if !self.layer.private_tile(coord) {
            self.needed += 1;
            // Bound oversized sparse requests before indexing their full canvas.
            self.admission.planned_bytes(self.needed)?;
        }
        Ok(())
    }
}

impl PaintPlan {
    pub(super) fn index_checked(
        &self,
        limit: usize,
        mut charge: BudgetAccount,
        target: &mut impl Target,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Index> {
        check()?;
        let mut metadata =
            metadata::Metadata::new(self.descriptor_bytes(), self.metadata_limit.min(limit))?;
        // Reverse local drop order frees storage before notifying charge release.
        let mut tiles: HashMap<TileCoord, Vec<usize>> = HashMap::new();
        for (index, &write) in self.writes.iter().enumerate() {
            append_write_checked(
                index,
                write,
                &mut tiles,
                &mut metadata,
                &mut charge,
                target,
                check,
            )?;
        }
        check()?;
        Ok(Index {
            tiles,
            bytes: metadata.used,
            charge,
        })
    }
}

/// Index one descriptor, shared by initial construction and incremental programs.
pub(super) fn append_write_checked(
    index: usize,
    write: Write,
    tiles: &mut HashMap<TileCoord, Vec<usize>>,
    metadata: &mut metadata::Metadata,
    charge: &mut BudgetAccount,
    target: &mut impl Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    let Some(area) = write.area.intersect(&target.bounds()) else {
        return Ok(());
    };
    if tiles.is_empty() && target.skip_empty(write) {
        return Ok(());
    }
    for ty in area.y0 / i64::from(TILE_EDGE)..=(area.y1 - 1) / i64::from(TILE_EDGE) {
        for tx in area.x0 / i64::from(TILE_EDGE)..=(area.x1 - 1) / i64::from(TILE_EDGE) {
            check()?;
            let coord = TileCoord { tx, ty };
            if tiles.len() == tiles.capacity() && !tiles.contains_key(&coord) {
                if target.skip_missing(coord, write)? {
                    continue;
                }
                metadata.table(tiles, charge)?;
            }
            let writes = match tiles.entry(coord) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    if target.skip_missing(coord, write)? {
                        continue;
                    }
                    target.admit_tile(coord)?;
                    entry.insert(Vec::new())
                }
            };
            metadata.vector(writes, charge)?;
            writes.push(index);
        }
    }
    Ok(())
}
