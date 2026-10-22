//! Immutable planned pixels for data-dependent preparation, without decoded tiles.
use super::*;

/// An owned plan and bounded read index, with no source pixel ownership.
/// Reads evaluate ordered writes against the caller's preceding logical source.
/// This is derived geometry, never a published document or a decoded reservation.
#[derive(Debug)]
pub struct PaintProjection {
    plan: PaintPlan,
    bounds: Rect,
    tiles: HashMap<TileCoord, Vec<usize>>,
    bytes: usize,
}
struct ReadTarget(Rect);
impl index::Target for ReadTarget {
    fn bounds(&self) -> Rect {
        self.0
    }
    // A write that is a no-op on one base may change another. Do not discard it
    // or inspect/pin source pixels while building a source-independent index.
    fn skip_empty(&self, _: Write) -> bool {
        false
    }
    fn skip_missing(&self, _: TileCoord, _: Write) -> bool {
        false
    }
    fn admit_tile(&mut self, _: TileCoord) -> Result<()> {
        Ok(())
    }
}
impl PaintPlan {
    /// Consume this plan into a bounded read-only projection over layer-local
    /// bounds. No pixel allocations, source reads or decoded charges occur here.
    pub fn into_projection_checked(
        self,
        bounds: Rect,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintProjection> {
        check()?;
        if bounds.x0 != 0
            || bounds.y0 != 0
            || bounds.x1 < 0
            || bounds.y1 < 0
            || bounds.x1 > i64::from(u32::MAX)
            || bounds.y1 > i64::from(u32::MAX)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "projection needs layer-local bounds",
            ));
        }
        let prepared = self.index_checked(self.metadata_limit, &mut ReadTarget(bounds), check)?;
        Ok(PaintProjection {
            plan: self,
            bounds,
            tiles: prepared.tiles,
            bytes: prepared.bytes,
        })
    }
}
impl PaintProjection {
    /// Declared descriptors, usable map payload and write-index vector capacities.
    /// Excludes allocator/hash controls, input geometry and any preceding sources.
    pub fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn tile_coords(&self) -> impl Iterator<Item = TileCoord> + '_ {
        self.tiles.keys().copied()
    }
    /// Drop the read index and recover descriptors for ordinary admitted replay.
    pub fn into_plan(self) -> PaintPlan {
        self.plan
    }
    /// Read one exact pixel against an immutable preceding source. Outside this
    /// layer's bounds is transparent and never invokes the source. Long overlapping
    /// descriptor lists check cancellation every 1024 entries, even noncovering ones.
    /// Source errors propagate without a partial returned pixel.
    pub fn read_checked(
        &self,
        x: i64,
        y: i64,
        source: impl FnOnce(i64, i64) -> Result<Rgba>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Rgba> {
        check()?;
        if !self.bounds.contains(x, y) {
            return Ok(Rgba::TRANSPARENT);
        }
        let mut pixel = source(x, y)?;
        if let Some(writes) = self.tiles.get(&TileCoord {
            tx: x / i64::from(TILE_EDGE),
            ty: y / i64::from(TILE_EDGE),
        }) {
            for (work, &index) in writes.iter().enumerate() {
                if work.is_multiple_of(1024) {
                    check()?;
                }
                let write = self.plan.writes[index];
                if write.area.contains(x, y) {
                    pixel = write.pixel(pixel);
                }
            }
        }
        check()?;
        Ok(pixel)
    }
}
