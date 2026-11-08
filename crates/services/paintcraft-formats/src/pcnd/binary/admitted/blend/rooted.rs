//! Root-ordered Source8 preparation. Never scan the full layer HashMap table.
//! Root/get slots are structural policy, not hash-probe/time or allocator bounds.
use super::*;
const ROOT_LOOKUP_WORK: u64 = 2048;
fn shape_count(doc: &Document) -> Result<usize> {
    if doc.roots.len() > LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len() {
        return Err(codec("invalid flat document roots"));
    }
    Ok(doc.roots.len())
}
fn root_work(count: usize, passes: u64) -> Result<u64> {
    (count as u64)
        .checked_mul(ROOT_LOOKUP_WORK)
        .and_then(|n| n.checked_mul(passes))
        .ok_or_else(limit)
}
/// Scope-funded root uniqueness/identity validation precedes every writer pass.
pub(super) fn preflight(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    check()?;
    let count = shape_count(doc)?;
    // Existing validator traverses/sorts the root Vec, not full table buckets.
    // Its own callback/index/sort charges remain; this prices its root lookups.
    spend(root_work(count, 1)?)?;
    let checked = || {
        check()?;
        spend(1)
    };
    crate::pcnd::layer_appearance::validate_flat_shape_checked(doc, scope, &checked, spend)?;
    // Four later lookup passes: this prewalk, coverage, catalog and output.
    spend(root_work(count, 4)?)?;
    spend(doc.title.as_ref().map_or(0, |t| t.len()) as u64)?;
    let mut non_normal = false;
    for id in &doc.roots {
        check()?;
        spend(1)?;
        let layer = doc
            .layers
            .get(id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        non_normal |= layer.blend_mode != paintcraft_document::LayerBlendMode::Normal;
        // Preserve the original770 structural slots: coverage+manifest visits,
        // and three <=256-entry catalog capture/sort/lookup policy declarations.
        spend(
            2u64.checked_add(
                (layer.filters.len() as u64)
                    .checked_mul(770)
                    .ok_or_else(limit)?,
            )
            .and_then(|n| n.checked_add(layer.name.len() as u64))
            .ok_or_else(limit)?,
        )?;
    }
    if !non_normal {
        return Err(unsupported(
            "Source8 requires stored nonNormal blend intent",
        ));
    }
    if let Some(source) = &doc.source_metadata {
        spend(
            (paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64)
                .checked_add(source.profile().map_or(0, |p| p.len()) as u64)
                .ok_or_else(limit)?,
        )?;
    }
    Ok(())
}
/// Same aggregate wire-use counts as legacy validation, with rooted lookups.
pub(super) fn validate_coverage_counts(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let limits = paintcraft_raster::coverage_mask::Limits::default();
    let (mut bands, mut runs) = (0usize, 0usize);
    let mut add = |mask: &paintcraft_raster::coverage_mask::A8Mask| -> Result<()> {
        check()?;
        let stats = mask.stats();
        bands = bands
            .checked_add(stats.bands)
            .filter(|n| *n <= limits.bands)
            .ok_or_else(coverage_limit)?;
        runs = runs
            .checked_add(stats.runs)
            .filter(|n| *n <= limits.runs)
            .ok_or_else(coverage_limit)?;
        Ok(())
    };
    for id in &doc.roots {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        for filter in layer.filters.iter() {
            if let Some(mask) = filter.mask() {
                add(mask.coverage())?;
            }
        }
        if let Some(mask) = &layer.mask {
            add(mask.coverage())?;
        }
    }
    Ok(())
}
fn coverage_limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "PCND aggregate coverage admission",
    )
}
/// Advisory, allocation-free quote. Caller prepays this rooted walk separately.
/// Full uniqueness/source admission is the writer's scope-funded preflight.
pub(super) fn scratch_quote(doc: &Document) -> Result<u64> {
    shape_count(doc)?;
    let mut max_tiles = 0usize;
    for id in &doc.roots {
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        max_tiles = max_tiles.max(layer.raster.resident_tiles());
    }
    let tile = 256usize * 256 * 4;
    let runs = tile / 4 * 6;
    let packed_runs = 4 + lz4_flex::block::get_maximum_output_size(runs);
    let packed_raw = lz4_flex::block::get_maximum_output_size(tile);
    let index = max_tiles
        .checked_mul(size_of::<(TileCoord, TileLease)>())
        .ok_or_else(limit)?;
    let root_phase = crate::pcnd::layer_appearance::flat_shape_scratch_quote(doc.roots.len())?;
    let tile_phase = [tile, runs, packed_runs, packed_raw, index, 4096 * 4]
        .into_iter()
        .try_fold(0u64, |n, v| n.checked_add(v as u64).ok_or_else(limit))?;
    Ok(root_phase.max(tile_phase))
}
