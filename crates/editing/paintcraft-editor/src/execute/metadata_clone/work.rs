//! Resource2 scans/handles/lifecycles, distinct from old copy/hash byte phases.
use super::*;
use paintcraft_raster::raster::metadata_clone::FreshMapLayout;
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limit)
}
fn sum(values: &[u64]) -> Result<u64> {
    values.iter().try_fold(0, |n, v| add(n, *v))
}
pub(super) fn scan(layout: FreshMapLayout) -> Result<u64> {
    if layout.capacity == 0 {
        return Ok(0);
    }
    let buckets = match layout.capacity {
        n if n <= 7 => n.checked_add(1).ok_or_else(limit)?,
        n => (n / 7).checked_mul(8).ok_or_else(limit)?,
    } as u64;
    add(layout.bytes, buckets)
}
pub(super) fn target(operation: MetadataOp<'_>) -> Result<u64> {
    let width = match operation {
        MetadataOp::Filter(_) => size_of::<FilterInstance>(),
        MetadataOp::Style(_) => size_of::<StyleInstance>(),
    } as u64;
    let pass = add(mul(515, width)?, 2050)?;
    let mut total = mul(2, pass)?;
    if operation.alias().is_some() {
        let alias = fresh_map_layout::<String, Option<(Uuid, Uuid)>>(1)?;
        let per_pass = sum(&[
            2 * (4 + 65),
            size_of::<String>() as u64,
            64,
            scan(alias)?,
            4,
            1,
        ])?;
        total = add(total, add(mul(2, per_pass)?, 257)?)?;
    }
    Ok(total)
}
pub(super) fn lifecycle(
    doc: &Document,
    operation: MetadataOp<'_>,
    filters: u64,
    tiles: u64,
    tile_scans: u64,
) -> Result<u64> {
    let layers = doc.roots.len() as u64;
    let metadata = u64::from(doc.source_metadata.is_some());
    let layer_scan = scan(fresh_map_layout::<Uuid, Layer>(doc.roots.len())?)?;
    let clone = sum(&[
        layer_scan,
        tile_scans,
        tiles,
        mul(2, filters)?,
        layers,
        metadata,
    ])?;
    let parent = sum(&[layer_scan, mul(2, layers)?, mul(2, filters)?, metadata])?;
    let retire = sum(&[parent, tile_scans, tiles])?;
    let introduced = u64::from(matches!(
        operation,
        MetadataOp::Filter(FilterOp::Add { .. } | FilterOp::Duplicate { .. })
            | MetadataOp::Style(StyleOp::Add { .. } | StyleOp::Duplicate { .. })
    ));
    let seed_aux = sum(&[
        size_of::<Document>() as u64,
        mul(2, filters)?,
        layers,
        metadata,
    ])?;
    // Seed's raster retirement is prepaid in Raster, not here a second time.
    sum(&[
        seed_aux,
        mul(2, clone)?,
        parent,
        mul(2, add(retire, mul(2, introduced)?)?)?,
        target(operation)?,
    ])
}
