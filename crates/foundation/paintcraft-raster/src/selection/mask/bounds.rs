//! Tight selected-sample bounds without materializing geometry or scanning empty space.
use super::*;
const EDGE: i64 = crate::TILE_EDGE as i64;

/// Shared host policy. Results retain only one immutable mask view and rectangle.
pub const BOUNDS_CACHE_WORK: u64 = 2_097_152;
#[derive(Default, Debug)]
pub struct BoundsCache {
    entry: Option<(BinaryMask, Rect, Result<Option<Rect>>)>,
}
impl BoundsCache {
    /// Release the retained mask when no selection/document is displayed.
    pub fn clear(&mut self) {
        self.entry = None;
    }

    /// Cache exact immutable-view results, including deterministic work denial.
    /// This uncancellable host helper never caches an externally cancelled request.
    pub fn get(&mut self, mask: &BinaryMask, clip: Rect) -> Result<Option<Rect>> {
        if let Some((key, area, result)) = &self.entry
            && key == mask
            && *area == clip
        {
            return result.clone();
        }
        let result = mask.bounds_checked(clip, BOUNDS_CACHE_WORK, &|| Ok(()));
        self.entry = Some((mask.clone(), clip, result.clone()));
        result
    }
}

impl BinaryMask {
    /// Smallest half-open rectangle containing selected samples inside `clip`.
    /// Full/empty fields cost one work unit. Sparse empty-default fields inspect
    /// stored tiles only; full-default fields search actual boundary lines and
    /// skip a zero tile band only when every crossing tile proves it empty.
    pub fn bounds_checked(
        &self,
        clip: Rect,
        work: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Rect>> {
        self.bounds_with_remaining_checked(clip, &mut { work }, check)
    }
    /// Share one traversal allowance across compound commands. Failure consumes
    /// work already attempted but never returns partial bounds or changes the mask.
    pub fn bounds_with_remaining_checked(
        &self,
        clip: Rect,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Rect>> {
        check()?;
        valid_domain(clip)?;
        spend(work, 1)?;
        let Some(area) = self.domain.intersect(&clip) else {
            return Ok(None);
        };
        let result = if self.storage.is_none() {
            (self.default_coverage() == 255).then_some(area)
        } else if self.default_coverage() == 0 {
            self.sparse_bounds(area, work, check)?
        } else {
            let Some(top) =
                self.boundary(true, true, area.y0, area.y1, area.x0, area.x1, work, check)?
            else {
                return Ok(None);
            };
            let bottom = self
                .boundary(true, false, top, area.y1, area.x0, area.x1, work, check)?
                .expect("top selected");
            let left = self
                .boundary(false, true, area.x0, area.x1, top, bottom + 1, work, check)?
                .expect("selected row");
            let right = self
                .boundary(false, false, left, area.x1, top, bottom + 1, work, check)?
                .expect("selected column");
            Some(Rect {
                x0: left,
                y0: top,
                x1: right + 1,
                y1: bottom + 1,
            })
        };
        check()?;
        Ok(result)
    }
    fn sparse_bounds(
        &self,
        area: Rect,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Rect>> {
        let mut result = None;
        for (coordinate, tile) in &self.storage.as_ref().expect("nonuniform").tiles {
            check()?;
            spend(work, 1)?;
            let x0 = coordinate.tx * EDGE + self.offset.0;
            let y0 = coordinate.ty * EDGE + self.offset.1;
            let Some(part) = area.intersect(&Rect {
                x0,
                y0,
                x1: x0 + EDGE,
                y1: y0 + EDGE,
            }) else {
                continue;
            };
            match tile {
                Tile::Solid(value) => {
                    if self.logical(*value) == 255 {
                        include(&mut result, part);
                    }
                }
                Tile::Dense(tile) => {
                    for y in part.y0..part.y1 {
                        check()?;
                        for x in part.x0..part.x1 {
                            if (x - part.x0) % 64 == 0 {
                                check()?;
                            }
                            spend(work, 1)?;
                            let index = ((y - y0) * EDGE + x - x0) as usize;
                            if self.logical(tile.values[index]) == 255 {
                                include(
                                    &mut result,
                                    Rect {
                                        x0: x,
                                        y0: y,
                                        x1: x + 1,
                                        y1: y + 1,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    #[allow(clippy::too_many_arguments)]
    fn boundary(
        &self,
        horizontal: bool,
        forward: bool,
        low: i64,
        high: i64,
        start: i64,
        end: i64,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<i64>> {
        let mut fixed = if forward { low } else { high - 1 };
        while (low..high).contains(&fixed) {
            check()?;
            spend(work, 1)?;
            let (selected, dense) = self.line(horizontal, fixed, start, end, work, check)?;
            if selected {
                return Ok(Some(fixed));
            }
            let offset = if horizontal {
                self.offset.1
            } else {
                self.offset.0
            };
            let (tile, _) = tile_split(fixed - offset);
            // A dense row proves only that row empty. A fully inspected line of
            // uniform empty tiles proves its entire fixed-axis tile band empty.
            fixed = if dense {
                if forward { fixed + 1 } else { fixed - 1 }
            } else if forward {
                (tile + 1) * EDGE + offset
            } else {
                tile * EDGE + offset - 1
            };
        }
        Ok(None)
    }
    fn line(
        &self,
        horizontal: bool,
        fixed: i64,
        mut at: i64,
        end: i64,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(bool, bool)> {
        let storage = self.storage.as_ref().expect("nonuniform");
        let mut dense = false;
        while at < end {
            check()?;
            spend(work, 1)?;
            let (x, y) = if horizontal { (at, fixed) } else { (fixed, at) };
            let (tx, ix) = tile_split(x - self.offset.0);
            let (ty, iy) = tile_split(y - self.offset.1);
            let next = if horizontal {
                (tx + 1) * EDGE + self.offset.0
            } else {
                (ty + 1) * EDGE + self.offset.1
            }
            .min(end);
            match storage.tiles.get(&TileCoord { tx, ty }) {
                None => return Ok((true, dense)), // This path has a selected default.
                Some(Tile::Solid(value)) => {
                    if self.logical(*value) == 255 {
                        return Ok((true, dense));
                    }
                }
                Some(Tile::Dense(tile)) => {
                    dense = true;
                    let mut index = iy as usize * EDGE as usize + ix as usize;
                    let stride = if horizontal { 1 } else { EDGE as usize };
                    for current in at..next {
                        if (current - at) % 64 == 0 {
                            check()?;
                        }
                        spend(work, 1)?;
                        if self.logical(tile.values[index]) == 255 {
                            return Ok((true, dense));
                        }
                        index += stride;
                    }
                }
            }
            at = next;
        }
        Ok((false, dense))
    }
}
fn include(result: &mut Option<Rect>, area: Rect) {
    *result = Some(result.map_or(area, |old| Rect {
        x0: old.x0.min(area.x0),
        y0: old.y0.min(area.y0),
        x1: old.x1.max(area.x1),
        y1: old.y1.max(area.y1),
    }));
}
