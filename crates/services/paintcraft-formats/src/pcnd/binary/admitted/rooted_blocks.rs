//! Explicit rooted resource amendment; ordinary blocks/sort remain untouched.
use super::*;
use paintcraft_raster::RetainedTileTableQuote;
const SCALAR_WORK: u64 = 64;
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
/// Additional nonempty collector upper AFTER scalar work has been paid.
/// No authority, allocation or source walk is created by this private math.
fn remaining(quote: RetainedTileTableQuote) -> Result<u64> {
    let n = quote.live_entries as u64;
    if n == 0 {
        return Ok(0);
    }
    let width = size_of::<(TileCoord, TileLease)>() as u64;
    let swaps = if n <= 1 {
        0
    } else {
        let height = u64::from(u64::BITS - (n - 1).leading_zeros());
        add(n - 1, mul(add(n / 2, n - 1)?, height)?)?
    };
    let handles = mul(n, add(width, 2)?)?; // Tuple upper + strong increment/decrement.
    let moves = mul(mul(3, width)?, swaps)?;
    add(quote.scan_work_upper, add(handles, moves)?)
}
pub(super) fn collect(
    raster: &RasterLayer,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
    emit: &mut BlockEmitter<'_>,
) -> Result<()> {
    spend(SCALAR_WORK)?;
    let quote = raster.retained_tile_table_quote()?;
    // Do not even construct the opaque iterator/index for allocated-empty maps.
    if quote.live_entries == 0 {
        return Ok(());
    }
    spend(remaining(quote)?)?;
    // Existing visit/comparison/pixel/codec/output prices remain distinct.
    super::blocks(raster, scope, check, spend, emit)
}
#[cfg(test)]
mod tests;
