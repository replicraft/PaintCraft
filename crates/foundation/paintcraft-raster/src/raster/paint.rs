//! Ordered paint descriptors with operation-wide tile admission.
use super::*;
use std::collections::hash_map::Entry;
mod metadata;

/// Provisional per-plan metadata safety policy, excluding allocator/hash overhead.
pub const DEFAULT_PAINT_METADATA_BYTES: usize = 64 << 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Write {
    area: Rect,
    color: Rgba,
    mode: BlendMode,
    samples: u8,
}
impl Write {
    fn pixel(self, dst: Rgba) -> Rgba {
        if self.mode == BlendMode::Replace && self.samples < 16 {
            let mut out = self.color;
            for (out, (&src, &dst)) in out.0.iter_mut().zip(self.color.0.iter().zip(dst.0.iter())) {
                *out = ((u16::from(src) * u16::from(self.samples)
                    + u16::from(dst) * u16::from(16 - self.samples)
                    + 8)
                    / 16) as u8;
            }
            out
        } else {
            blend(self.mode, self.color, dst)
        }
    }
    fn row(self, pixels: &mut [u8]) {
        if self.mode == BlendMode::Replace && self.samples == 16
            || self.mode == BlendMode::SourceOver && self.color.a() == 255
        {
            pixels.as_chunks_mut::<4>().0.fill(self.color.0);
        } else if self.mode == BlendMode::SourceOver {
            for dst in pixels.as_chunks_mut::<4>().0 {
                *dst = self.color.source_over(Rgba(*dst)).0;
            }
        } else {
            let samples = u16::from(self.samples);
            for dst in pixels.as_chunks_mut::<4>().0 {
                for (dst, &src) in dst.iter_mut().zip(self.color.0.iter()) {
                    *dst = ((u16::from(src) * samples + u16::from(*dst) * (16 - samples) + 8) / 16)
                        as u8;
                }
            }
        }
    }
}
/// Ordered clipped rectangles/runs; overlapping writes retain their original order.
#[derive(Clone, Debug)]
pub struct PaintPlan {
    writes: Vec<Write>,
    metadata_limit: usize,
}
impl Default for PaintPlan {
    fn default() -> Self {
        Self::with_metadata_limit(DEFAULT_PAINT_METADATA_BYTES)
    }
}
impl PaintPlan {
    /// Limit retained descriptors plus temporary tile-index payload capacities.
    /// This is a fixed per-plan policy, not decoded residency or process RSS.
    pub fn with_metadata_limit(bytes: usize) -> Self {
        Self {
            writes: Vec::new(),
            metadata_limit: bytes,
        }
    }
    pub fn descriptor_bytes(&self) -> usize {
        self.writes.capacity() * std::mem::size_of::<Write>()
    }
    pub fn rect(&mut self, area: Rect, color: Rgba, mode: BlendMode) -> Result<()> {
        self.push(Write {
            area,
            color,
            mode,
            samples: 16,
        })
    }
    /// Exact straight-channel 16-sample replacement coverage used by smooth erasing.
    pub fn coverage_rect(&mut self, area: Rect, color: Rgba, samples: u8) -> Result<()> {
        if samples > 16 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "paint coverage exceeds 16 samples",
            ));
        }
        self.push(Write {
            area,
            color,
            mode: BlendMode::Replace,
            samples,
        })
    }
    fn push(&mut self, write: Write) -> Result<()> {
        if write.area.x0 >= write.area.x1 || write.area.y0 >= write.area.y1 {
            return Ok(());
        }
        if let Some(previous) = self.writes.last_mut()
            && previous.color == write.color
            && previous.mode == write.mode
            && previous.samples == write.samples
            && previous.area.y0 == write.area.y0
            && previous.area.y1 == write.area.y1
            && previous.area.x1 == write.area.x0
        {
            previous.area.x1 = write.area.x1;
            return Ok(());
        }
        metadata::Metadata::new(self.descriptor_bytes(), self.metadata_limit)?
            .vector(&mut self.writes)?;
        self.writes.push(write);
        Ok(())
    }
    /// Apply to private raster staging. Quota admission precedes tile allocation/writes;
    /// allocator/index failures retain the caller-managed staging contract.
    pub fn apply(&self, layer: &mut RasterLayer, admission: &mut Admission) -> Result<()> {
        self.apply_checked(layer, admission, &|| Ok(()))
    }
    /// Cancellable private replay. Cancellation can leave private staging partially
    /// painted; the transaction owner must discard it rather than publish it.
    pub fn apply_checked(
        &self,
        layer: &mut RasterLayer,
        admission: &mut Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        if self.writes.is_empty() {
            return Ok(());
        }
        let mut metadata = metadata::Metadata::new(
            self.descriptor_bytes(),
            self.metadata_limit.min(admission.paint_metadata_limit),
        )?;
        let mut tiles: HashMap<TileCoord, Vec<usize>> = HashMap::new();
        let mut needed = 0u64;
        for (index, write) in self.writes.iter().enumerate() {
            check()?;
            let Some(area) = write.area.intersect(&layer.bounds()) else {
                continue;
            };
            if tiles.is_empty()
                && layer.tiles.is_empty()
                && write.mode == BlendMode::Replace
                && write.samples == 16
                && write.color == layer.default
            {
                continue;
            }
            for ty in area.y0 / i64::from(TILE_EDGE)..=(area.y1 - 1) / i64::from(TILE_EDGE) {
                for tx in area.x0 / i64::from(TILE_EDGE)..=(area.x1 - 1) / i64::from(TILE_EDGE) {
                    check()?;
                    let coord = TileCoord { tx, ty };
                    if tiles.len() == tiles.capacity() && !tiles.contains_key(&coord) {
                        if !layer.tiles.contains_key(&coord)
                            && write.pixel(layer.default) == layer.default
                        {
                            continue;
                        }
                        metadata.table(&mut tiles)?;
                    }
                    let writes = match tiles.entry(coord) {
                        Entry::Occupied(entry) => entry.into_mut(),
                        Entry::Vacant(entry) => {
                            if !layer.tiles.contains_key(&coord)
                                && write.pixel(layer.default) == layer.default
                            {
                                continue;
                            }
                            if !layer.private_tile(coord) {
                                needed += 1;
                                // Stop an oversized sparse operation before building an
                                // index for its entire logical canvas.
                                admission.planned_bytes(needed)?;
                            }
                            entry.insert(Vec::new())
                        }
                    };
                    metadata.vector(writes)?;
                    writes.push(index);
                }
            }
        }
        admission.with_tile_plan(needed, |admission| {
            let missing = tiles
                .keys()
                .filter(|coord| !layer.tiles.contains_key(coord))
                .count();
            layer.tiles.try_reserve(missing).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot prepare paint tile index")
            })?;
            for coord in tiles.keys() {
                check()?;
                layer.with_tile_mut(*coord, admission, |_| ())?;
            }
            let bounds = layer.bounds();
            let edge = i64::from(TILE_EDGE);
            for (coord, writes) in tiles {
                check()?;
                let tile_area = Rect {
                    x0: coord.tx * edge,
                    y0: coord.ty * edge,
                    x1: (coord.tx + 1) * edge,
                    y1: (coord.ty + 1) * edge,
                };
                let revision = super::regions::next_revision();
                layer.with_tile_mut(coord, admission, |tile| {
                    for index in writes {
                        check()?;
                        let write = self.writes[index];
                        let Some(area) = write
                            .area
                            .intersect(&tile_area)
                            .and_then(|area| area.intersect(&bounds))
                        else {
                            continue;
                        };
                        tile.mark_region(
                            area.translated(-tile_area.x0, -tile_area.y0)
                                .expect("tile-local paint"),
                            revision,
                        );
                        for y in area.y0..area.y1 {
                            if (y - area.y0) % 64 == 0 {
                                check()?;
                            }
                            let start = (((y - tile_area.y0) as usize * TILE_EDGE as usize)
                                + (area.x0 - tile_area.x0) as usize)
                                * 4;
                            let end = start + area.width() as usize * 4;
                            write.row(&mut tile.pixels[start..end]);
                        }
                    }
                    Ok::<(), Error>(())
                })??;
            }
            check()?;
            Ok(())
        })
    }
}
impl RasterLayer {
    /// Resolve weak-only identities without copying; a racing/live reader still
    /// requires admitted COW. No pixel is changed by this ownership normalization.
    fn private_tile(&mut self, coord: TileCoord) -> bool {
        let Some(tile) = self.tiles.get_mut(&coord) else {
            return false;
        };
        if Arc::get_mut(tile).is_some() {
            return true;
        }
        if Arc::strong_count(tile) == 1 {
            let original = std::mem::replace(tile, Arc::new(Tile::empty()));
            match Arc::try_unwrap(original) {
                Ok(owned) => {
                    *tile = Arc::new(owned);
                    return true;
                }
                Err(original) => *tile = original,
            }
        }
        false
    }
}
