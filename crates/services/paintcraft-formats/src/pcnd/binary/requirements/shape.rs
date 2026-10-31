//! Borrowed wire residency, independent of live allocations and writer elision.
use super::*;

pub(super) fn tile_records(
    verified: &CheckedContainer<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    let version = verified.version;
    if !matches!(version, 1..=6) {
        return Err(unsupported("unknown native document version"));
    }
    // Work is finite independently of caller cancellation. Controls are scalar
    // plus the fixed borrowed manifest; no vector, decoder or new realm exists.
    let remaining = Cell::new(paintcraft_raster::coverage_mask::Limits::default().work);
    let step = || {
        check()?;
        remaining.set(remaining.get().checked_sub(1).ok_or_else(resource)?);
        Ok(())
    };
    step()?;
    let mut input = Input::new(verified.body);
    let initial = input
        .remaining()
        .checked_add(MAGIC.len())
        .ok_or_else(resource)?;
    let catalog_count = if version == 6 {
        // Reuse canonical borrowed manifest validation; the second tiny prefix
        // read extracts only its count, with no second source authentication.
        let mut prefix = input;
        let length = prefix.number(65_536)? as usize;
        let mut prelude = Input::new(prefix.take(length)?);
        let count = prelude.number(256)?;
        catalog::Manifest::read(&mut input, false, &step)?;
        Some(count)
    } else {
        None
    };
    input.take(32)?;
    input.dimension()?;
    input.dimension()?;
    if flag(&mut input)? {
        string(&mut input, 4096)?;
    }
    let has_source = if version >= 3 {
        flag(&mut input)?
    } else {
        version == 2
    };
    if has_source {
        source(&mut input)?;
    }
    metadata(initial, input.remaining(), 0, DOCUMENT_MAX as usize)?;
    let layers = input.number(LAYER_COUNT_MAX as u64)?;
    let mut masks = MaskScan::new(Default::default());
    let mut tiles = 0u64;
    let mut payload = 0usize;
    for _ in 0..layers {
        step()?;
        input.take(16)?;
        string(&mut input, 1024)?;
        flag(&mut input)?;
        input.byte()?;
        input.signed()?;
        input.signed()?;
        let width = input.dimension()?;
        let height = input.dimension()?;
        input.take(4)?;
        if version >= 3 {
            let count = input.number(paintcraft_document::filters::FILTERS_MAX as u64)?;
            for _ in 0..count {
                step()?;
                input.take(16)?;
                flag(&mut input)?;
                if let Some(entries) = catalog_count {
                    let index = input.number(255)?;
                    if index >= entries {
                        return Err(codec("invalid PCND catalog framing"));
                    }
                    let length = input.number(65_536)? as usize;
                    input.take(length)?;
                } else {
                    scalar(&mut input, version)?;
                }
                if version >= 4 {
                    masks.skip(&mut input, &step)?;
                }
                metadata(initial, input.remaining(), payload, DOCUMENT_MAX as usize)?;
            }
        }
        loop {
            step()?;
            let encoding = input.byte()?;
            if encoding == 0 {
                break;
            }
            if encoding > 5 {
                return Err(codec("unknown compact block encoding"));
            }
            let x = input.number(u64::from(width.div_ceil(TILE_EDGE) - 1))? as u32;
            let y = input.number(u64::from(height.div_ceil(TILE_EDGE) - 1))? as u32;
            let decoded = ((width - x * TILE_EDGE).min(TILE_EDGE)
                * (height - y * TILE_EDGE).min(TILE_EDGE)
                * 4) as usize;
            let length = input.number(BLOCK_BYTES_MAX)? as usize;
            if length == 0 || (encoding == 1 && length != decoded) {
                return Err(codec("invalid stored block length"));
            }
            tiles = tiles
                .checked_add(1)
                .filter(|n| *n <= STAGED_PIXELS_MAX / (u64::from(TILE_EDGE) * u64::from(TILE_EDGE)))
                .ok_or_else(resource)?;
            input.take(length)?;
            payload = payload.checked_add(length).ok_or_else(resource)?;
            metadata(initial, input.remaining(), payload, DOCUMENT_MAX as usize)?;
        }
        metadata(initial, input.remaining(), payload, DOCUMENT_MAX as usize)?;
    }
    if input.remaining() != 0 {
        return Err(codec("trailing compact PCND data"));
    }
    step()?;
    Ok(tiles)
}

#[cfg(test)]
mod tests;
