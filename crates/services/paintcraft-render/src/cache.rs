//! Bounded, exact native-resolution composite tiles for immutable input revisions.
use crate::{output_buffer, region_bytes, render_region_cancellable};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result};
use paintcraft_document::Document;

pub(crate) mod key;
mod prefix;
pub(crate) mod storage;
use key::Key;
use storage::Partition;

#[derive(Clone, Copy, Debug)]
pub struct CacheLimits {
    pub tile_edge: u32,
    pub committed_bytes: usize,
    pub preview_bytes: usize,
    pub entries_per_domain: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Committed,
    Preview,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub cache_hits: u64,
    pub prefix_hits: u64,
    /// Tiles retaining unaffected output and recomposing only changed dependency cells.
    pub patched_tiles: u64,
    pub recomposed_tiles: u64,
    pub recomposed_pixels: u64,
    pub output_pixels: u64,
}

#[derive(Debug)]
pub struct CachedRegion {
    pub pixels: Vec<u8>,
    pub stats: RenderStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheUsage {
    /// Capacity of cached decoded output Vecs; caller-owned returned images excluded.
    pub pixel_bytes: usize,
    /// Container/key capacities plus a conservative weak-control-block charge.
    /// Allocator overhead and process RSS are separate measurements.
    pub metadata_charge: usize,
    pub entries: usize,
}
impl CacheUsage {
    pub fn weight(self) -> usize {
        self.pixel_bytes + self.metadata_charge
    }
}

#[derive(Debug)]
pub struct CompositeCache {
    limits: CacheLimits,
    committed: Partition,
    preview: Partition,
}

impl CompositeCache {
    pub fn new(limits: CacheLimits) -> Result<Self> {
        if !(1..=1024).contains(&limits.tile_edge)
            || !(1..=4096).contains(&limits.entries_per_domain)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "invalid composite cache tile/entry limits",
            ));
        }
        Ok(Self {
            committed: Partition::new(limits.committed_bytes, limits.entries_per_domain)?,
            preview: Partition::new(limits.preview_bytes, limits.entries_per_domain)?,
            limits,
        })
    }
    pub fn usage(&self, domain: Domain) -> CacheUsage {
        match domain {
            Domain::Committed => self.committed.usage(),
            Domain::Preview => self.preview.usage(),
        }
    }
    pub fn clear_preview(&mut self) {
        self.preview.clear();
    }
    pub fn clear(&mut self) {
        self.preview.clear();
        self.committed.clear();
    }

    /// Pointwise flat composition: input dependencies equal the full native-resolution
    /// output tile. No effects, halos, profiles, mipmaps or display overlays are cached.
    /// Complete cached tiles may warm before cancellation; partial tiles are never installed.
    pub fn render(
        &mut self,
        doc: &Document,
        region: Rect,
        domain: Domain,
        check: impl Fn() -> Result<()>,
    ) -> Result<CachedRegion> {
        let levels_caller_check = &check;
        let check = || levels_caller_check();
        crate::prepared::with_document_levels(doc, &check, || {
            check()?;
            if crate::spatial::relevant(doc, region) {
                let pixels = render_region_cancellable(doc, region, check)?;
                return Ok(CachedRegion {
                    pixels,
                    stats: RenderStats {
                        output_pixels: region.area() as u64,
                        recomposed_pixels: region.area() as u64,
                        recomposed_tiles: 1,
                        ..Default::default()
                    },
                });
            }
            let mut pixels = output_buffer(region_bytes(doc, region)?)?;
            let mut stats = RenderStats {
                output_pixels: region.area() as u64,
                ..Default::default()
            };
            if !doc.layers_in_order().any(|layer| {
                layer.visible && layer.opacity != 0 && region.intersect(&layer.extent()).is_some()
            }) {
                check()?;
                return Ok(CachedRegion { pixels, stats });
            }
            let edge = i64::from(self.limits.tile_edge);
            for ty in region.y0 / edge..=(region.y1 - 1) / edge {
                for tx in region.x0 / edge..=(region.x1 - 1) / edge {
                    check()?;
                    let rect = Rect {
                        x0: tx * edge,
                        y0: ty * edge,
                        x1: ((tx + 1) * edge).min(i64::from(doc.canvas.0)),
                        y1: ((ty + 1) * edge).min(i64::from(doc.canvas.1)),
                    };
                    let tile_bytes = rect.area() as usize * 4;
                    let selected = match domain {
                        Domain::Committed => &self.committed,
                        Domain::Preview => &self.preview,
                    };
                    let key_budget =
                        selected
                            .key_budget(tile_bytes)
                            .max(if domain == Domain::Preview {
                                self.committed.key_budget(tile_bytes)
                            } else {
                                0
                            });
                    let key = Key::build(doc, rect, key_budget)?;
                    let hit = key.as_ref().and_then(|key| {
                        if domain == Domain::Preview
                            && let Some(index) = self.committed.find(key)
                        {
                            return Some((Domain::Committed, index));
                        }
                        selected.find(key).map(|index| (domain, index))
                    });
                    if let Some((domain, index)) = hit {
                        let partition = match domain {
                            Domain::Committed => &mut self.committed,
                            Domain::Preview => &mut self.preview,
                        };
                        let source = partition.touch(index);
                        copy_tile(&mut pixels, region, rect, source);
                        stats.cache_hits += 1;
                        continue;
                    }
                    let patch = if let Some(key) = key.as_ref() {
                        let committed = if domain == Domain::Preview {
                            self.committed.patch_source(key)?
                        } else {
                            None
                        };
                        if committed.is_some() {
                            committed
                        } else {
                            match domain {
                                Domain::Committed => self.committed.patch_source(key)?,
                                Domain::Preview => self.preview.patch_source(key)?,
                            }
                        }
                    } else {
                        None
                    };
                    let source = if let Some((regions, mut source)) = patch {
                        for dirty in regions {
                            check()?;
                            let changed = render_region_cancellable(doc, dirty, check)?;
                            copy_tile(&mut source, rect, dirty, &changed);
                            stats.recomposed_pixels += dirty.area() as u64;
                        }
                        stats.patched_tiles += 1;
                        source
                    } else {
                        let source = self.render_miss(
                            doc,
                            rect,
                            domain,
                            key_budget,
                            key.is_some(),
                            &check,
                            &mut stats,
                        )?;
                        stats.recomposed_pixels += rect.area() as u64;
                        source
                    };
                    check()?;
                    copy_tile(&mut pixels, region, rect, &source);
                    stats.recomposed_tiles += 1;
                    if let Some(key) = key {
                        let partition = match domain {
                            Domain::Committed => &mut self.committed,
                            Domain::Preview => &mut self.preview,
                        };
                        partition.insert(key, source, false);
                    }
                }
            }
            check()?;
            Ok(CachedRegion { pixels, stats })
        })
    }
}

fn copy_tile(out: &mut [u8], region: Rect, tile: Rect, pixels: &[u8]) {
    let part = region
        .intersect(&tile)
        .expect("requested tile intersects the output");
    for y in part.y0..part.y1 {
        let dst = ((y - region.y0) * region.width() + part.x0 - region.x0) as usize * 4;
        let src = ((y - tile.y0) * tile.width() + part.x0 - tile.x0) as usize * 4;
        let bytes = part.width() as usize * 4;
        out[dst..dst + bytes].copy_from_slice(&pixels[src..src + bytes]);
    }
}
