//! Own fresh zeros here: no cache/prefix/caller-populated buffer can enter.
use super::{Seed, limit, missing};
#[cfg(test)]
pub(super) use crate::blend::compose::seed_pixel as pixel;
use paintcraft_core::types::Result;
pub(crate) fn fresh_output(
    seed: &Seed<'_>,
    bytes: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let doc = seed.document();
    let region = seed.region();
    if crate::region_bytes(doc, region)? != bytes {
        return Err(limit());
    }
    let mut out = crate::output_buffer(bytes)?;
    if seed.index == usize::MAX {
        crate::compose_roots_into(doc, region, 0..doc.roots.len(), &mut out, &check)?;
    } else {
        let layer = doc.layers.get(&doc.roots[seed.index]).ok_or_else(missing)?;
        let area = region.intersect(&layer.extent()).ok_or_else(missing)?;
        if crate::spatial::active(layer) {
            crate::blend::compose::spatial_shared(layer, area, region, &mut out, None, check)?;
        } else {
            let width = region.width() as usize;
            for y in area.y0..area.y1 {
                check()?;
                let start = ((y - region.y0) as usize * width + (area.x0 - region.x0) as usize) * 4;
                crate::blend::compose::row_shared(
                    layer,
                    y,
                    area.x0,
                    &mut out[start..start + area.width() as usize * 4],
                    None,
                    check,
                )?;
            }
        }
        // Earlier roots provably write nothing in this ROI. Above-root order
        // and every intermediate RGBA8 boundary remain unchanged.
        crate::compose_roots_into(
            doc,
            region,
            seed.index + 1..doc.roots.len(),
            &mut out,
            &check,
        )?;
    }
    Ok(out)
}
