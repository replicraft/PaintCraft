//! PCND4 canonical finite A8 bands. No canvas-sized coverage buffer is encoded.
use super::*;
use crate::pcnd::coverage::CoverageReader;
use paintcraft_core::types::Rect;
use paintcraft_document::FilterMask;
use paintcraft_raster::coverage_mask::{Run, Visit};
mod decode;

pub(super) const CODEC: u8 = 1; // a8-bands-v1
pub(super) fn write<W: Write>(
    mask: Option<&FilterMask>,
    output: &mut Output<W>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    output.bytes(&[u8::from(mask.is_some())])?;
    let Some(mask) = mask else { return Ok(()) };
    output.bytes(&[u8::from(mask.enabled()), CODEC])?;
    let coverage = mask.coverage();
    let domain = coverage.domain();
    for value in [domain.x0, domain.y0, domain.x1, domain.y1] {
        output.signed(value)?;
    }
    output.number(coverage.stats().bands as u64)?;
    coverage.visit_bands_checked(u64::MAX, check, |event| match event {
        Visit::Band { y0, y1, runs } => {
            output.number((y0 - domain.y0) as u64)?;
            output.number((y1 - y0) as u64)?;
            output.number(runs as u64)
        }
        Visit::Run(run) => {
            output.number((run.x0 - domain.x0) as u64)?;
            output.number((run.x1 - run.x0) as u64)?;
            output.bytes(&[run.coverage])
        }
    })
}
#[derive(Clone, Copy)]
struct Header {
    domain: Rect,
    bands: usize,
    runs: usize,
    enabled: bool,
    bytes: usize,
}
fn invalid() -> Error {
    codec("invalid or noncanonical PCND A8 bands")
}
fn resource() -> Error {
    Error::new(ErrorCode::ResourceLimit, "PCND aggregate mask extent")
}
fn positive(input: &mut Input<'_>, maximum: u64) -> Result<u64> {
    let value = input.number(maximum)?;
    if value == 0 {
        return Err(invalid());
    }
    Ok(value)
}
fn add(origin: i64, value: u64) -> Result<i64> {
    origin
        .checked_add(i64::try_from(value).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}
/// Inspect all borrowed bytes and canonical shape before allocating staging/final data.
fn inspect(
    input: &mut Input<'_>,
    allowance: usize,
    reader: &CoverageReader<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<Header>> {
    reader.step(check)?;
    let initial = input.remaining();
    match input.byte()? {
        0 => return Ok(None),
        1 => {}
        _ => return Err(invalid()),
    }
    let enabled = match input.byte()? {
        0 => false,
        1 => true,
        _ => return Err(invalid()),
    };
    if input.byte()? != CODEC {
        return Err(unsupported("unknown PCND filter mask codec/version"));
    }
    let (x0, y0, x1, y1) = (
        input.signed()?,
        input.signed()?,
        input.signed()?,
        input.signed()?,
    );
    let domain = Rect::new(
        x0,
        y0,
        x1.checked_sub(x0).ok_or_else(invalid)?,
        y1.checked_sub(y0).ok_or_else(invalid)?,
    )
    .ok_or_else(invalid)?;
    let bands = usize::try_from(input.number(u64::MAX)?).map_err(|_| resource())?;
    let (remaining_bands, remaining_runs) = reader.remaining();
    if bands > remaining_bands {
        return Err(resource());
    }
    let (mut runs, mut last_y) = (0usize, domain.y0);
    let mut prior: Option<(i64, &[u8])> = None;
    for _ in 0..bands {
        reader.step(check)?;
        let y0 = add(domain.y0, input.number(domain.height() as u64)?)?;
        let y1 = add(y0, positive(input, domain.height() as u64)?)?;
        if y0 < last_y || y1 > domain.y1 {
            return Err(invalid());
        }
        let count = usize::try_from(positive(input, u64::MAX)?).map_err(|_| resource())?;
        runs = runs
            .checked_add(count)
            .filter(|n| *n <= remaining_runs)
            .ok_or_else(resource)?;
        let start = input.rest();
        let (mut last_x, mut last_coverage) = (domain.x0, 0u8);
        for _ in 0..count {
            reader.step(check)?;
            let x0 = add(domain.x0, input.number(domain.width() as u64)?)?;
            let x1 = add(x0, positive(input, domain.width() as u64)?)?;
            let coverage = input.byte()?;
            if coverage == 0
                || x0 < last_x
                || x1 > domain.x1
                || (x0 == last_x && coverage == last_coverage)
            {
                return Err(invalid());
            }
            last_x = x1;
            last_coverage = coverage;
            if initial - input.remaining() > allowance {
                return Err(resource());
            }
        }
        let encoded = &start[..start.len() - input.remaining()];
        if prior.is_some_and(|(end, bytes)| end == y0 && bytes == encoded) {
            return Err(invalid());
        }
        prior = Some((y1, encoded));
        last_y = y1;
    }
    let bytes = initial - input.remaining();
    if bytes > allowance {
        return Err(resource());
    }
    Ok(Some(Header {
        domain,
        bands,
        runs,
        enabled,
        bytes,
    }))
}
pub(super) fn read(
    input: &mut Input<'_>,
    allowance: usize,
    reader: &mut CoverageReader<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<FilterMask>> {
    let mut probe = *input;
    let Some(header) = inspect(&mut probe, allowance, reader, check)? else {
        *input = probe;
        return Ok(None);
    };
    reader.counts(header.bands, header.runs)?;
    let mask = decode::materialize(input, header, reader, check)?;
    Ok(Some(FilterMask::new(header.enabled, mask)))
}
pub(super) fn coarse_bytes(mask: Option<&FilterMask>) -> Option<u64> {
    let Some(mask) = mask else { return Some(1) };
    let stats = mask.coverage().stats();
    // Exact near-cap paths use the real selected writer, not this overestimate.
    3u64.checked_add(40)?
        .checked_add(10)?
        .checked_add((stats.bands as u64).checked_mul(30)?)?
        .checked_add((stats.runs as u64).checked_mul(21)?)
}

pub(super) fn decode_quote(bands: usize, runs: usize) -> Result<u64> {
    let retained = if bands == 0 {
        0
    } else {
        paintcraft_raster::coverage_mask::retained_quote(bands, runs)?
    };
    decode::scratch(bands, runs)?
        .checked_add(retained)
        .ok_or_else(resource)
}
