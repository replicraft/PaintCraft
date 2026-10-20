//! Exact CPU partitions run exclusively on an explicitly supplied bounded pool.
use crate::{compose_roots_into, output_buffer, region_bytes};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result};
use paintcraft_document::Document;
use rayon::{ThreadPool, prelude::*};

mod tiles;

#[derive(Clone, Copy, Debug)]
pub enum Partition {
    Bands { rows: u32 },
    Tiles { edge: u32 },
}

/// Full-resolution output, disjoint writes, immutable inputs and cooperative
/// cancellation. The caller owns thread/admission policy; no global pool is used.
/// `view_bytes` limits temporary tile-view metadata (not pixels or pool stacks).
/// Caller-owned output has the ordinary regional admission limit. Bands need no
/// tile-view allocation. Errors discard the entire private output.
pub fn render_in_pool(
    pool: &ThreadPool,
    doc: &Document,
    region: Rect,
    partition: Partition,
    view_bytes: usize,
    check: impl Fn() -> Result<()> + Sync,
) -> Result<Vec<u8>> {
    check()?;
    let bytes = region_bytes(doc, region)?;
    let edge = match partition {
        Partition::Bands { rows } => rows,
        Partition::Tiles { edge } => edge,
    };
    if !(1..=1024).contains(&edge) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "partition size must be 1..=1024",
        ));
    }
    if matches!(partition, Partition::Tiles { .. }) {
        tiles::admit(region, edge, view_bytes)?;
    }
    let mut pixels = output_buffer(bytes)?;
    pool.install(|| match partition {
        Partition::Bands { rows } => {
            let stride = region.width() as usize * 4;
            let chunk_bytes = stride * (rows as usize).min(region.height() as usize);
            pixels
                .par_chunks_mut(chunk_bytes)
                .enumerate()
                .try_for_each(|(index, out)| {
                    check()?;
                    let y0 = region.y0 + index as i64 * i64::from(rows);
                    let part = Rect {
                        y0,
                        y1: (y0 + i64::from(rows)).min(region.y1),
                        ..region
                    };
                    compose_roots_into(doc, part, 0..doc.roots.len(), out, &check)
                })
        }
        Partition::Tiles { edge } => {
            tiles::compose(doc, region, edge, view_bytes, &mut pixels, &check)
        }
    })?;
    check()?;
    Ok(pixels)
}
