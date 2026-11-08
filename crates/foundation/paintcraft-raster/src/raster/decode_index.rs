//! New-empty index only. The external original M must retain the complete union.
use super::*;
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "fresh decode index admission")
}
impl RasterLayer {
    /// Pure fresh destination extent; caller prepays64 scalar slots, not a grant.
    /// Returned live_entries is zero because no block is inserted here.
    pub fn fresh_decode_index_quote(records: usize) -> Result<RetainedTileTableQuote> {
        let layout = metadata_clone::fresh_map_layout::<TileCoord, Arc<Tile>>(records)?;
        let buckets = match layout.capacity {
            0 => 0,
            n if n <= 7 => n.checked_add(1).ok_or_else(limit)?,
            n => (n / 7).checked_mul(8).ok_or_else(limit)?,
        };
        Ok(RetainedTileTableQuote {
            allocation_upper: layout.bytes,
            bucket_upper: buckets,
            live_entries: 0,
            scan_work_upper: layout.bytes.checked_add(buckets as u64).ok_or_else(limit)?,
        })
    }
    /// Construct only a new, empty tracked map under an already assigned account.
    /// Caller MUST retain M after the returned Raster and all aliases end, and
    /// fund all simultaneous source indices/headers/metadata rather than reuse
    /// this account's bytes as multiple independent grants. This primitive is
    /// not a closed source lease. It accepts no existing Raster or bucket claim.
    /// Callbacks are raw trusted SAME-owner callbacks; this helper bills each
    /// actual check's512 structural slot before invoking it.
    pub fn new_decode_index_precharged_checked(
        width: u32,
        height: u32,
        default: Rgba,
        records: usize,
        metadata: &BudgetAccount,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        let step = |n: u64| {
            spend(n.checked_add(512).ok_or_else(limit)?)?;
            check()
        };
        step(64)?;
        if width == 0 || height == 0 || width > 1048576 || height > 1048576 {
            return Err(Error::new(ErrorCode::InputSchema, "fresh decode geometry"));
        }
        let positions = u64::from(width.div_ceil(256)) * u64::from(height.div_ceil(256));
        if records as u64 > positions {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "fresh decode record count",
            ));
        }
        let expected = Self::fresh_decode_index_quote(records)?;
        if metadata.bytes() < expected.allocation_upper {
            return Err(limit());
        }
        let mut raster = Self::new(width, height, default); // private new-empty provenance
        step(expected.allocation_upper)?;
        raster.tiles.try_reserve(records)?;
        if raster.tiles.retained_quote()? != expected {
            return Err(limit());
        }
        Ok(raster)
    }
}
#[cfg(test)]
mod tests;
