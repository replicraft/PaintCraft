//! Already-validated wire becomes charged parse scratch and immutable A8 storage.
use super::*;
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits};
use std::mem::size_of;

struct ParsedBand {
    y0: i64,
    y1: i64,
    start: usize,
    len: usize,
}
pub(in crate::pcnd) fn scratch(bands: usize, runs: usize) -> Result<u64> {
    bands
        .checked_mul(size_of::<ParsedBand>() + size_of::<Band<'_>>())
        .and_then(|n| {
            runs.checked_mul(size_of::<Run>())
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(resource)
}
fn reserve<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| resource())?;
    Ok(values)
}
pub(super) fn materialize(
    input: &mut Input<'_>,
    header: Header,
    reader: &mut CoverageReader<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<A8Mask> {
    let peak = decode_quote(header.bands, header.runs)?;
    let scope = reader.reserve(peak)?;
    // Account is declared before all buffers, so scratch drops before its charge.
    let mut charge = scope.account();
    charge.resize(scratch(header.bands, header.runs)?)?;
    let mut bands = reserve::<ParsedBand>(header.bands)?;
    let mut runs = reserve::<Run>(header.runs)?;
    let mut borrowed = reserve::<Band<'_>>(header.bands)?;
    let actual = bands
        .capacity()
        .checked_mul(size_of::<ParsedBand>())
        .and_then(|n| {
            runs.capacity()
                .checked_mul(size_of::<Run>())
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| {
            borrowed
                .capacity()
                .checked_mul(size_of::<Band<'_>>())
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(resource)?;
    charge.resize(actual)?;
    let mut wire = *input;
    wire.take(3)?; // presence, enabled, codec validated by borrowed pre-scan
    for _ in 0..4 {
        wire.signed()?;
    }
    wire.number(u64::MAX)?;
    for _ in 0..header.bands {
        reader.step(check)?;
        let y0 = add(header.domain.y0, wire.number(u64::MAX)?)?;
        let y1 = add(y0, wire.number(u64::MAX)?)?;
        let len = usize::try_from(wire.number(u64::MAX)?).map_err(|_| resource())?;
        let start = runs.len();
        for _ in 0..len {
            reader.step(check)?;
            let x0 = add(header.domain.x0, wire.number(u64::MAX)?)?;
            let x1 = add(x0, wire.number(u64::MAX)?)?;
            runs.push(Run {
                x0,
                x1,
                coverage: wire.byte()?,
            });
        }
        bands.push(ParsedBand { y0, y1, start, len });
    }
    for band in &bands {
        reader.step(check)?;
        borrowed.push(Band {
            y0: band.y0,
            y1: band.y1,
            runs: &runs[band.start..band.start + band.len],
        });
    }
    let limits = Limits {
        work: u64::MAX,
        bands: header.bands,
        runs: header.runs,
    };
    let mask = A8Mask::from_bands_in_scope(header.domain, &borrowed, &scope, limits, &|| {
        reader.step(check)
    })?;
    reader.retain(mask.storage_usage().map_or(0, |(_, bytes)| bytes))?;
    if input.remaining() - wire.remaining() != header.bytes {
        return Err(invalid());
    }
    *input = wire;
    Ok(mask)
}
