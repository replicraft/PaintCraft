//! Normal-only root/coverage facts. Structural root/get policy, never table scan.
use super::*;
use paintcraft_core::types::{LayerBlendMode, filters::Descriptor};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
const ROOT: u64 = 2048;
pub(super) struct Facts {
    pub version: u8,
    pub filters: u64,
    // Own-mask presence, any filters, filter-mask presence, BoxBlur, catalog-only.
    flags: [bool; 5],
    // Facts/catalog must end before their original declaration releases.
    _control: paintcraft_core::types::resources::BudgetAccount,
}
pub(super) fn control_bytes() -> u64 {
    (size_of::<Facts>() + size_of::<catalog::Manifest<'static>>()) as u64
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
pub(super) fn root_work(count: usize, passes: u64) -> Result<u64> {
    (count as u64)
        .checked_mul(ROOT)
        .and_then(|n| n.checked_mul(passes))
        .ok_or_else(limit)
}
fn shape_count(doc: &Document) -> Result<usize> {
    if doc.roots.len() > LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len() {
        return Err(codec("invalid flat document roots"));
    }
    Ok(doc.roots.len())
}
fn normal(layer: &paintcraft_document::Layer) -> Result<()> {
    if layer.blend_mode != LayerBlendMode::Normal {
        return Err(unsupported(
            "Normal portable snapshot refuses stored nonNormal blend",
        ));
    }
    Ok(())
}
pub(super) fn prepare(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Facts> {
    check()?;
    let n = shape_count(doc)?;
    let mut control = scope.account();
    control.resize(control_bytes())?;
    spend(root_work(n, 1)?)?;
    let checked = || {
        check()?;
        spend(1)
    };
    crate::pcnd::layer_appearance::validate_flat_shape_checked(doc, scope, &checked, spend)?;
    spend(root_work(n, 1)?)?;
    spend(doc.title.as_ref().map_or(0, |t| t.len()) as u64)?;
    if let Some(source) = &doc.source_metadata {
        spend(add(
            paintcraft_document::source_metadata::WIRE_HEADER_BYTES as u64,
            source.profile().map_or(0, |p| p.len()) as u64,
        )?)?;
    }
    let mut facts = Facts {
        version: 1,
        filters: 0,
        flags: [false; 5],
        _control: control,
    };
    let limits = Limits::default();
    let (mut bands, mut runs) = (0usize, 0usize);
    let mut coverage = |mask: &A8Mask| -> Result<()> {
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
        spend(1)?;
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        normal(layer)?;
        let count = layer.filters.len() as u64;
        if count > 256 {
            return Err(limit());
        }
        spend(add(
            add(2, count.checked_mul(2).ok_or_else(limit)?)?,
            layer.name.len() as u64,
        )?)?;
        facts.filters = add(facts.filters, count)?;
        facts.flags[0] |= layer.mask.is_some();
        facts.flags[1] |= !layer.filters.is_empty();
        for filter in layer.filters.iter() {
            check()?;
            facts.flags[3] |= matches!(filter.descriptor(), Descriptor::BoxBlurV1(_));
            facts.flags[4] |= filter.descriptor().is_catalog_only();
            if let Some(mask) = filter.mask() {
                facts.flags[2] = true;
                coverage(mask.coverage())?;
            }
        }
        if let Some(mask) = &layer.mask {
            coverage(mask.coverage())?;
        }
    }
    facts.version = if facts.flags[0] {
        7
    } else if facts.flags[4] {
        6
    } else if facts.flags[3] {
        5
    } else if facts.flags[2] {
        4
    } else if facts.flags[1] {
        3
    } else if doc.source_metadata.is_some() {
        2
    } else {
        1
    };
    check()?;
    Ok(facts)
}
fn coverage_limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "PCND aggregate coverage admission",
    )
}
/// Separate advisory root pass. Full source/uniqueness admission is not a quote.
pub(super) fn scratch_quote(doc: &Document) -> Result<u64> {
    shape_count(doc)?;
    let mut max_tiles = 0usize;
    for id in &doc.roots {
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        normal(layer)?;
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
        .try_fold(0u64, |n, v| add(n, v as u64))?;
    add(control_bytes(), root_phase.max(tile_phase))
}
