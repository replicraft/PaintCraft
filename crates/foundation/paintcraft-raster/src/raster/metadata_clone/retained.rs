//! Explicit retained-source work amendment; the legacy compact helper is unchanged.
use super::*;
use crate::raster::RetainedTileTableQuote;
const SCALAR_WORK: u64 = 64;
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
fn remaining(source: RetainedTileTableQuote, destination: FreshMapLayout) -> Result<u64> {
    if source.live_entries == 0 {
        return Ok(0);
    }
    let buckets = match destination.capacity {
        0 => 0,
        n if n <= 7 => n.checked_add(1).ok_or_else(limit)?,
        n => (n / 7).checked_mul(8).ok_or_else(limit)?,
    } as u64;
    // insert_reserved performs absent get_mut and the distinct raw insertion.
    // The old40/entry already includes one key copy/hash input. This adds the
    // second key input and one Arc strong increment, plus both probe extents.
    let per_entry = add(mul(buckets, 2)?, size_of::<TileCoord>() as u64 + 1)?;
    add(
        source.scan_work_upper,
        mul(source.live_entries as u64, per_entry)?,
    )
}
fn retirement(entries: usize, destination: FreshMapLayout) -> Result<u64> {
    if entries == 0 {
        return Ok(0);
    }
    let buckets = match destination.capacity {
        n if n <= 7 => n.checked_add(1).ok_or_else(limit)?,
        n => (n / 7).checked_mul(8).ok_or_else(limit)?,
    } as u64;
    add(add(destination.bytes, buckets)?, entries as u64)
}
impl RasterLayer {
    /// Future construction plus one local retirement, not a grant or an actual scan. Callers
    /// must fund their own scalar calculation before requesting this forecast.
    /// Retirement covers non-final local seed/error cleanup while the source is
    /// alive. It does not bound a direct caller's eventual final output cleanup.
    pub fn compact_metadata_clone_retained_work_quote(&self) -> Result<u64> {
        add(SCALAR_WORK, {
            let source = self.retained_tile_table_quote()?;
            let layout = self.compact_metadata_clone_quote()?;
            add(
                remaining(source, layout)?,
                retirement(source.live_entries, layout)?,
            )?
        })
    }
    /// Resource-contract2 compact seed. The caller retains the original account
    /// through result drop. No new scope, fresh budget or legacy fallback exists.
    /// The returned independent Raster may outlive `self`. Retirement prepayment
    /// covers partial error cleanup and a local Stage seed retired while its base
    /// is alive; final output/backing destruction remains the caller owner phase.
    pub fn compact_metadata_clone_retained_precharged_checked(
        &self,
        charge: &BudgetAccount,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        check()?;
        spend(SCALAR_WORK)?;
        let layout = self.compact_metadata_clone_quote()?;
        if charge.bytes() < layout.bytes {
            return Err(limit());
        }
        let source = self.retained_tile_table_quote()?;
        if source.live_entries == 0 {
            // Even allocated-empty/tombstoned tables never construct an iterator.
            return Ok(Self {
                width: self.width,
                height: self.height,
                default: self.default,
                tiles: TileTable::new(),
            });
        }
        spend(remaining(source, layout)?)?;
        // One owner retires either on partial failure OR later seed drop.
        spend(retirement(source.live_entries, layout)?)?;
        spend(layout.bytes)?;
        let mut tiles = TileTable::new();
        tiles.try_reserve(self.tiles.len()).map_err(|_| limit())?;
        if tiles.capacity() != layout.capacity {
            return Err(bad("fresh tile map differs from pinned layout"));
        }
        for (coord, tile) in &self.tiles {
            check()?;
            spend((size_of::<(TileCoord, Arc<Tile>)>() + size_of::<TileCoord>()) as u64)?;
            tiles.insert_reserved(*coord, Arc::clone(tile))?;
        }
        Ok(Self {
            width: self.width,
            height: self.height,
            default: self.default,
            tiles,
        })
    }
}
#[cfg(test)]
mod tests;
