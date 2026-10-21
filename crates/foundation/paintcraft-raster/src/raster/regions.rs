//! Derived finite-region identities. No pixel hashing, leases or serialized state.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Candidate dependency granularity; authoritative storage remains 256x256.
pub const REGION_EDGE: u32 = 64;
pub(super) const REGIONS_PER_EDGE: usize = (TILE_EDGE / REGION_EDGE) as usize;
pub(super) const REGIONS_PER_TILE: usize = REGIONS_PER_EDGE * REGIONS_PER_EDGE;
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_revision() -> u64 {
    revision_from(&NEXT_REVISION)
}
fn revision_from(counter: &AtomicU64) -> u64 {
    // Never wrap or reuse an identity. Saturation makes affected tiles uncacheable.
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .unwrap_or(u64::MAX)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionCoord {
    pub rx: i64,
    pub ry: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionVersion(Version);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Version {
    Uniform(Rgba),
    Written(u64),
}
impl Tile {
    /// The caller must mark every potentially written pixel before mutating it.
    pub(super) fn mark_region(&mut self, local: Rect, revision: u64) {
        let edge = i64::from(REGION_EDGE);
        for ry in local.y0 / edge..=(local.y1 - 1) / edge {
            for rx in local.x0 / edge..=(local.x1 - 1) / edge {
                self.regions[ry as usize * REGIONS_PER_EDGE + rx as usize] = revision;
            }
        }
    }
    pub(super) fn mark_all(&mut self) {
        self.regions.fill(next_revision());
    }
}
impl RasterLayer {
    /// All dependency cells intersecting a checked layer-local region, including
    /// implicit defaults. COW preserves untouched cells. No source pixels are pinned.
    /// None means identity exhaustion: consumers must bypass derived caching.
    pub fn region_versions(
        &self,
        area: Rect,
    ) -> Result<Option<impl Iterator<Item = (RegionCoord, RegionVersion)> + '_>> {
        if area.x0 < 0
            || area.y0 < 0
            || area.x1 <= area.x0
            || area.y1 <= area.y0
            || area.x1 > i64::from(self.width)
            || area.y1 > i64::from(self.height)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "region version bounds outside layer",
            ));
        }
        let tile_edge = i64::from(TILE_EDGE);
        for ty in area.y0 / tile_edge..=(area.y1 - 1) / tile_edge {
            for tx in area.x0 / tile_edge..=(area.x1 - 1) / tile_edge {
                if self
                    .tiles
                    .get(&TileCoord { tx, ty })
                    .is_some_and(|t| t.regions.contains(&u64::MAX))
                {
                    return Ok(None);
                }
            }
        }
        let edge = i64::from(REGION_EDGE);
        Ok(Some((area.y0 / edge..=(area.y1 - 1) / edge).flat_map(
            move |ry| {
                (area.x0 / edge..=(area.x1 - 1) / edge).map(move |rx| {
                    let version = self
                        .tiles
                        .get(&TileCoord {
                            tx: rx / REGIONS_PER_EDGE as i64,
                            ty: ry / REGIONS_PER_EDGE as i64,
                        })
                        .map_or(Version::Uniform(self.default), |t| {
                            let index = (ry as usize % REGIONS_PER_EDGE) * REGIONS_PER_EDGE
                                + rx as usize % REGIONS_PER_EDGE;
                            match t.regions[index] {
                                0 => Version::Uniform(t.initial),
                                revision => Version::Written(revision),
                            }
                        });
                    (RegionCoord { rx, ry }, RegionVersion(version))
                })
            },
        )))
    }
}

#[cfg(test)]
mod tests;
