//! Sparse straight-RGBA8 layers.
//!
//! A layer has finite bounds and an exact implicit default pixel inside them;
//! outside the bounds it contributes nothing. Pixels live in copy-on-write
//! 256×256 tiles addressed with Euclidean division. A missing tile equals the
//! default everywhere, including hidden RGB under alpha zero. Every allocation
//! (new tile or copy-on-write copy) is charged to an [`Admission`] so exhaustion
//! is an explicit `resource.limit` error rather than an abort.

use paintcraft_core::types::resources::{
    AllocationClass, BudgetAccount, BudgetReservation, BudgetScope, SharedBudget,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

mod paint;
pub use paint::{
    CropPlan, DEFAULT_PAINT_METADATA_BYTES, DEFAULT_RESIDENT_PAINT_METADATA_BYTES, MappedPlan,
    PaintOverlay, PaintPlan, PaintProjection, PaintReader, PaintSnapshot, SimulatedLayer,
    TileFootprint, TilePlanner,
};
mod tile;
use tile::Tile;

use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};

mod rows;
mod uniform;
pub use rows::RowSpan;
mod leases;
pub use leases::TileLease;
mod regions;
pub use regions::{REGION_EDGE, RegionCoord, RegionVersion};
mod versions;
pub use versions::TileVersion;

pub const TILE_EDGE: u32 = 256;
/// Default process-wide decoded tile payload capacity; excludes other resources.
pub const DEFAULT_RESIDENT_TILE_BYTES: u64 = 2 << 30;
const TILE_PIXELS: usize = (TILE_EDGE as usize) * (TILE_EDGE as usize);

/// Euclidean split of a signed coordinate into (tile index, offset in tile).
pub fn tile_split(v: i64) -> (i64, u32) {
    let e = i64::from(TILE_EDGE);
    (v.div_euclid(e), v.rem_euclid(e) as u32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub tx: i64,
    pub ty: i64,
}

pub use paintcraft_core::types::policy::BlendMode;

fn blend(mode: BlendMode, src: Rgba, dst: Rgba) -> Rgba {
    match mode {
        BlendMode::Replace => src,
        BlendMode::SourceOver => src.source_over(dst),
    }
}

/// Byte budget for staged pixel storage; shared across one batch.
#[derive(Clone, Debug)]
pub struct Admission {
    remaining_pixels: u64,
    paint_metadata_limit: usize,
    paint_metadata_scope: BudgetScope,
    flood_scratch_limit: usize,
    stroke_scratch_limit: usize,
    residency: SharedBudget,
    reservation: Option<BudgetReservation>,
    allocation_scope: Option<Arc<std::sync::Mutex<tile::Scope>>>,
}

impl Admission {
    pub fn new(limit_pixels: u64) -> Self {
        static DEFAULT: OnceLock<SharedBudget> = OnceLock::new();
        Self::with_residency(
            limit_pixels,
            DEFAULT
                .get_or_init(|| SharedBudget::new(DEFAULT_RESIDENT_TILE_BYTES))
                .clone(),
        )
    }
    /// Explicit allocation realm, for callers with a separate admitted policy.
    /// Every normal `new` admission shares the process-default tile realm.
    pub fn with_residency(limit_pixels: u64, residency: SharedBudget) -> Self {
        Self {
            remaining_pixels: limit_pixels,
            paint_metadata_limit: DEFAULT_PAINT_METADATA_BYTES,
            paint_metadata_scope: paint::default_metadata_budget()
                .scope(AllocationClass::PaintMetadata),
            flood_scratch_limit: crate::flood::DEFAULT_FLOOD_SCRATCH_BYTES,
            stroke_scratch_limit: crate::shape::DEFAULT_STROKE_SCRATCH_BYTES,
            residency,
            reservation: None,
            allocation_scope: None,
        }
    }

    /// Consume a caller's upfront decoded reservation. No tile can borrow beyond it.
    /// Cloned admissions share the same reservation and allocation gauge.
    pub fn with_reserved_residency(limit_pixels: u64, reservation: BudgetReservation) -> Self {
        Self {
            remaining_pixels: limit_pixels,
            paint_metadata_limit: DEFAULT_PAINT_METADATA_BYTES,
            paint_metadata_scope: paint::default_metadata_budget()
                .scope(AllocationClass::PaintMetadata),
            flood_scratch_limit: crate::flood::DEFAULT_FLOOD_SCRATCH_BYTES,
            stroke_scratch_limit: crate::shape::DEFAULT_STROKE_SCRATCH_BYTES,
            residency: reservation.budget(),
            reservation: Some(reservation),
            allocation_scope: Some(Arc::new(std::sync::Mutex::new(tile::Scope::default()))),
        }
    }
    /// Set an independent fixed per-plan metadata allowance. This does not add
    /// retryable decoded demand or alter the cumulative staged-pixel policy.
    pub fn with_paint_metadata_limit(mut self, bytes: usize) -> Self {
        self.paint_metadata_limit = bytes;
        self
    }
    /// Share descriptor and index payload admission independently of decoded tiles.
    pub fn with_paint_metadata_budget(mut self, budget: SharedBudget) -> Self {
        self.paint_metadata_scope = budget.scope(AllocationClass::PaintMetadata);
        self
    }
    /// Transfer known metadata capacity into this preparation attempt. Factories
    /// retain unused capacity, while surviving storage retains only its own bytes.
    /// Growth beyond the live reservation is rejected rather than borrowing.
    pub fn with_reserved_paint_metadata(mut self, reservation: BudgetReservation) -> Self {
        self.paint_metadata_scope = reservation.scope(AllocationClass::PaintMetadata);
        self
    }
    pub fn paint_metadata_budget(&self) -> SharedBudget {
        self.paint_metadata_scope.budget()
    }
    pub fn paint_metadata_limit(&self) -> usize {
        self.paint_metadata_limit
    }
    /// Own one independently resizable preparation-metadata declaration.
    pub fn paint_metadata_account(&self) -> BudgetAccount {
        self.paint_metadata_scope.account()
    }
    /// Configure bitmap/frontier/output capacity for each flood classification.
    pub fn with_flood_scratch_limit(mut self, bytes: usize) -> Self {
        self.flood_scratch_limit = bytes;
        self
    }
    pub fn flood_scratch_limit(&self) -> usize {
        self.flood_scratch_limit
    }
    pub fn with_stroke_scratch_limit(mut self, bytes: usize) -> Self {
        self.stroke_scratch_limit = bytes;
        self
    }
    pub fn stroke_scratch_limit(&self) -> usize {
        self.stroke_scratch_limit
    }
    pub fn paint_plan(&self) -> PaintPlan {
        PaintPlan::with_metadata_scope(self.paint_metadata_limit, self.paint_metadata_scope.clone())
    }
    /// Reserve a complete known set of fresh tile allocations before materialization.
    /// This validates but does not consume the cumulative staged-pixel allowance.
    /// Existing explicit reservations are checked rather than silently expanded.
    pub fn reserve_tiles(&mut self, tiles: u64) -> Result<()> {
        let bytes = self.planned_bytes(tiles)?;
        let result = if let Some(reservation) = &self.reservation {
            reservation.ensure_capacity(bytes)
        } else if bytes == 0 {
            Ok(())
        } else {
            self.residency
                .reserve_capacity(bytes)
                .map(|reservation| self.reservation = Some(reservation))
        };
        result.map_err(|error| {
            error.with_allocation_requirement(
                paintcraft_core::types::resources::AllocationClass::DecodedTiles,
                bytes,
            )
        })
    }
    fn planned_bytes(&self, tiles: u64) -> Result<u64> {
        let pixels = tiles
            .checked_mul(TILE_PIXELS as u64)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "tile plan overflow"))?;
        if pixels > self.remaining_pixels {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "staged pixel budget exceeded",
            ));
        }
        pixels
            .checked_mul(4)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "tile plan byte overflow"))
    }
    fn with_tile_plan<T>(
        &mut self,
        tiles: u64,
        run: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if tiles != 0 && self.allocation_scope.is_none() {
            self.allocation_scope = Some(Arc::new(std::sync::Mutex::new(tile::Scope::default())));
        }
        let mut operation = self.clone();
        if let Err(error) = operation.reserve_tiles(tiles) {
            let mut required = tiles
                .checked_mul(RasterLayer::TILE_BYTES as u64)
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "paint plan footprint overflow")
                })?;
            if let Some(scope) = &self.allocation_scope {
                let scope = scope.lock().unwrap_or_else(|error| error.into_inner());
                required = scope
                    .live
                    .checked_add(required)
                    .ok_or_else(|| {
                        Error::new(ErrorCode::ResourceLimit, "paint attempt footprint overflow")
                    })?
                    .max(scope.peak);
            }
            return Err(error.with_allocation_requirement(
                paintcraft_core::types::resources::AllocationClass::DecodedTiles,
                required,
            ));
        }
        let result = run(&mut operation);
        self.remaining_pixels = operation.remaining_pixels;
        result
    }
    /// Restrict an explicit admission to the session's configured staged allowance.
    pub fn constrain_pixels(mut self, limit_pixels: u64) -> Self {
        self.remaining_pixels = self.remaining_pixels.min(limit_pixels);
        self
    }
    /// Share this allocation realm with a resource coordinator or wake observer.
    pub fn residency_budget(&self) -> SharedBudget {
        self.residency.clone()
    }
    /// Live counters for this allocation realm; not allocator/RSS measurements.
    pub fn residency_usage(&self) -> paintcraft_core::types::resources::BudgetUsage {
        self.residency.usage()
    }
    fn charge_tile(&mut self) -> Result<()> {
        let cost = TILE_PIXELS as u64;
        if self.remaining_pixels < cost {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "staged pixel budget exceeded",
            ));
        }
        self.remaining_pixels -= cost;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RasterLayer {
    width: u32,
    height: u32,
    default: Rgba,
    tiles: HashMap<TileCoord, Arc<Tile>>,
}

impl RasterLayer {
    pub fn new(width: u32, height: u32, default: Rgba) -> RasterLayer {
        RasterLayer {
            width,
            height,
            default,
            tiles: HashMap::new(),
        }
    }

    /// A layer whose samples are `rgba` (row-major, `width*height*4` bytes).
    /// Tiles that equal the default are not allocated.
    pub fn from_rgba(
        width: u32,
        height: u32,
        default: Rgba,
        rgba: &[u8],
        adm: &mut Admission,
    ) -> Result<RasterLayer> {
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "pixel buffer does not match the layer size",
            ));
        }
        let mut layer = RasterLayer::new(width, height, default);
        let edge = TILE_EDGE as usize;
        for ty in 0..height.div_ceil(TILE_EDGE) {
            for tx in 0..width.div_ceil(TILE_EDGE) {
                let (x0, y0) = (tx as usize * edge, ty as usize * edge);
                let (w, h) = (
                    (width as usize - x0).min(edge),
                    (height as usize - y0).min(edge),
                );
                let uniform = (0..h).all(|y| {
                    rgba[((y0 + y) * width as usize + x0) * 4..][..w * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| *p == default.0)
                });
                if uniform {
                    continue;
                }
                let mut data = Vec::with_capacity(w * h * 4);
                for y in 0..h {
                    data.extend_from_slice(&rgba[((y0 + y) * width as usize + x0) * 4..][..w * 4]);
                }
                layer.set_block(tx, ty, &data, adm)?;
            }
        }
        Ok(layer)
    }

    /// Whether any sample hides colour under alpha zero (non-zero RGB, alpha 0).
    pub fn has_hidden_rgb(&self) -> bool {
        let hidden = |p: &[u8]| p[3] == 0 && p[..3] != [0, 0, 0];
        hidden(&self.default.0)
            || self
                .tiles
                .values()
                .any(|t| t.pixels.as_chunks::<4>().0.iter().any(|p| hidden(p)))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn default_pixel(&self) -> Rgba {
        self.default
    }

    /// Extend finite bounds with the existing implicit default. Tile padding is
    /// initialized to that default, so existing tile buffers remain shared.
    pub fn expand_to(&mut self, width: u32, height: u32) -> Result<()> {
        let max = paintcraft_core::types::policy::CANVAS_SIDE_MAX;
        if width < self.width
            || height < self.height
            || !(1..=max).contains(&width)
            || !(1..=max).contains(&height)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "invalid layer expansion",
            ));
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// Layer-local bounds.
    pub fn bounds(&self) -> Rect {
        Rect::from_origin_size(0, 0, self.width, self.height).expect("layers are non-empty")
    }

    /// Identity of every resident tile (the address of its shared buffer), for
    /// counting memory that several snapshots share exactly once.
    pub fn tile_identities(&self) -> impl Iterator<Item = usize> + '_ {
        self.tiles.values().map(|t| Arc::as_ptr(t) as usize)
    }

    /// Bytes held by one resident tile.
    pub const TILE_BYTES: usize = TILE_PIXELS * 4;

    pub fn resident_tiles(&self) -> usize {
        self.tiles.len()
    }

    /// Pixel at a layer-local coordinate; the default for any unwritten pixel and
    /// for coordinates outside the bounds (callers clip before asking).
    pub fn get(&self, lx: i64, ly: i64) -> Rgba {
        if !self.bounds().contains(lx, ly) {
            return self.default;
        }
        let ((tx, x), (ty, y)) = (tile_split(lx), tile_split(ly));
        self.tiles
            .get(&TileCoord { tx, ty })
            .map_or(self.default, |t| t.at(x, y))
    }

    fn with_tile_mut<R>(
        &mut self,
        c: TileCoord,
        adm: &mut Admission,
        write: impl FnOnce(&mut Tile) -> R,
    ) -> Result<R> {
        use std::collections::hash_map::Entry;
        if self.tiles.len() == self.tiles.capacity() && !self.tiles.contains_key(&c) {
            self.tiles.try_reserve(1).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot allocate raster index")
            })?;
        }
        let default = self.default;
        match self.tiles.entry(c) {
            Entry::Occupied(entry) => {
                let arc = entry.into_mut();
                if let Some(tile) = Arc::get_mut(arc) {
                    return Ok(write(tile));
                }
                {
                    // try_unwrap atomically arbitrates weak-reader upgrades. A
                    // successful transfer moves the existing Vec without copying;
                    // a racing/live reader requires admitted, fallible COW.
                    let original = std::mem::replace(arc, Arc::new(Tile::empty()));
                    match Arc::try_unwrap(original) {
                        Ok(tile) => *arc = Arc::new(tile),
                        Err(original) => {
                            *arc = original;
                            let tile = Tile::copied(arc, adm)?;
                            *arc = Arc::new(tile);
                        }
                    }
                }
                Ok(write(Arc::get_mut(arc).expect("private admitted tile")))
            }
            Entry::Vacant(entry) => {
                let tile = Tile::filled(default, adm)?;
                Ok(write(
                    Arc::get_mut(entry.insert(Arc::new(tile))).expect("new private tile"),
                ))
            }
        }
    }

    /// Blend one pixel; out-of-bounds coordinates are ignored. Unwritten tiles are
    /// not allocated when the result equals the default.
    pub fn blend_pixel(
        &mut self,
        lx: i64,
        ly: i64,
        src: Rgba,
        mode: BlendMode,
        adm: &mut Admission,
    ) -> Result<()> {
        if !self.bounds().contains(lx, ly) {
            return Ok(());
        }
        let ((tx, x), (ty, y)) = (tile_split(lx), tile_split(ly));
        let c = TileCoord { tx, ty };
        if !self.tiles.contains_key(&c) && blend(mode, src, self.default) == self.default {
            return Ok(());
        }
        self.with_tile_mut(c, adm, |tile| {
            let new = blend(mode, src, tile.at(x, y));
            if new != tile.at(x, y) {
                tile.mark_region(
                    Rect {
                        x0: i64::from(x),
                        y0: i64::from(y),
                        x1: i64::from(x) + 1,
                        y1: i64::from(y) + 1,
                    },
                    regions::next_revision(),
                );
                tile.put(x, y, new);
            }
        })
    }

    /// Blend `src` over every pixel of `area` (layer-local, clipped to bounds).
    pub fn fill_rect(
        &mut self,
        area: &Rect,
        src: Rgba,
        mode: BlendMode,
        adm: &mut Admission,
    ) -> Result<()> {
        let Some(area) = area.intersect(&self.bounds()) else {
            return Ok(());
        };
        let edge = i64::from(TILE_EDGE);
        let ((tx0, _), (tx1, _)) = (tile_split(area.x0), tile_split(area.x1 - 1));
        let ((ty0, _), (ty1, _)) = (tile_split(area.y0), tile_split(area.y1 - 1));
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let c = TileCoord { tx, ty };
                if !self.tiles.contains_key(&c) && blend(mode, src, self.default) == self.default {
                    continue;
                }
                let part = Rect {
                    x0: tx * edge,
                    y0: ty * edge,
                    x1: (tx + 1) * edge,
                    y1: (ty + 1) * edge,
                }
                .intersect(&area)
                .expect("tile range comes from the area");
                self.with_tile_mut(c, adm, |tile| {
                    tile.mark_region(
                        part.translated(-tx * edge, -ty * edge)
                            .expect("tile-local intersection"),
                        regions::next_revision(),
                    );
                    for y in part.y0..part.y1 {
                        for x in part.x0..part.x1 {
                            let (lx, ly) = (x.rem_euclid(edge) as u32, y.rem_euclid(edge) as u32);
                            let new = blend(mode, src, tile.at(lx, ly));
                            tile.put(lx, ly, new);
                        }
                    }
                })?;
            }
        }
        Ok(())
    }

    /// Copy the samples of layer-local row `ly`, columns `[x0, x0 + out.len()/4)`,
    /// into `out`. The span must lie inside the layer.
    pub fn read_span(&self, ly: i64, x0: i64, out: &mut [u8]) {
        let (ty, y) = tile_split(ly);
        let mut x = x0;
        let mut rest = &mut *out;
        while !rest.is_empty() {
            let (tx, lx) = tile_split(x);
            let n = ((TILE_EDGE - lx) as usize).min(rest.len() / 4);
            let (head, tail) = std::mem::take(&mut rest).split_at_mut(n * 4);
            match self.tiles.get(&TileCoord { tx, ty }) {
                Some(t) => {
                    let start = ((y * TILE_EDGE + lx) * 4) as usize;
                    head.copy_from_slice(&t.pixels[start..start + n * 4]);
                }
                None => head
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .for_each(|px| *px = self.default.0),
            }
            rest = tail;
            x += n as i64;
        }
    }

    /// Every stored sample, row-major and tightly packed, one row per call.
    pub fn for_each_row(&self, mut sink: impl FnMut(&[u8])) {
        let mut row = vec![0u8; self.width as usize * 4];
        for y in 0..i64::from(self.height) {
            self.read_span(y, 0, &mut row);
            sink(&row);
        }
    }

    /// Non-default 256×256 blocks, in (by, bx) order, as `(bx, by, width, height, rgba)`.
    /// Blocks equal to the default (including hidden RGB) are omitted.
    pub fn blocks(&self) -> Vec<(u32, u32, u32, u32, Vec<u8>)> {
        self.blocks_iter().collect()
    }

    /// Materialize one persistence block at a time. Only the coordinate index
    /// is retained across iterations, so sink backpressure bounds pixel scratch.
    pub fn blocks_iter(&self) -> impl Iterator<Item = (u32, u32, u32, u32, Vec<u8>)> + '_ {
        let mut coords: Vec<TileCoord> = self.tiles.keys().copied().collect();
        coords.sort_by_key(|c| (c.ty, c.tx));
        coords
            .into_iter()
            .filter(|c| !self.tiles[c].is_uniform(self.default))
            .map(|c| {
                let (bx, by) = (c.tx as u32, c.ty as u32);
                let w = (self.width - bx * TILE_EDGE).min(TILE_EDGE);
                let h = (self.height - by * TILE_EDGE).min(TILE_EDGE);
                let tile = &self.tiles[&c];
                let mut data = Vec::with_capacity((w * h * 4) as usize);
                for y in 0..h {
                    let start = (y * TILE_EDGE * 4) as usize;
                    data.extend_from_slice(&tile.pixels[start..start + (w * 4) as usize]);
                }
                (bx, by, w, h, data)
            })
    }

    /// Install a decoded block (reader side). `data` must be exactly `w*h*4` bytes.
    pub fn set_block(&mut self, bx: u32, by: u32, data: &[u8], adm: &mut Admission) -> Result<()> {
        let bad = |m: &str| Error::new(ErrorCode::CodecFailed, m.to_owned());
        if bx.checked_mul(TILE_EDGE).is_none_or(|x| x >= self.width)
            || by.checked_mul(TILE_EDGE).is_none_or(|y| y >= self.height)
        {
            return Err(bad("block outside layer"));
        }
        let w = (self.width - bx * TILE_EDGE).min(TILE_EDGE);
        let h = (self.height - by * TILE_EDGE).min(TILE_EDGE);
        if data.len() != (w * h * 4) as usize {
            return Err(bad("block length does not match its extent"));
        }
        self.with_tile_mut(
            TileCoord {
                tx: i64::from(bx),
                ty: i64::from(by),
            },
            adm,
            |tile| {
                tile.mark_all();
                for y in 0..h {
                    let (s, d) = ((y * w * 4) as usize, (y * TILE_EDGE * 4) as usize);
                    tile.pixels[d..d + (w * 4) as usize]
                        .copy_from_slice(&data[s..s + (w * 4) as usize]);
                }
            },
        )
    }
}

/// Semantic equality: a missing tile equals an all-default tile.
impl PartialEq for RasterLayer {
    fn eq(&self, other: &Self) -> bool {
        if (self.width, self.height, self.default) != (other.width, other.height, other.default) {
            return false;
        }
        let covers = |a: &RasterLayer, b: &RasterLayer| {
            a.tiles.iter().all(|(c, t)| match b.tiles.get(c) {
                Some(u) => Arc::ptr_eq(t, u) || t.pixels == u.pixels,
                None => t.is_uniform(a.default),
            })
        };
        covers(self, other) && covers(other, self)
    }
}

impl Eq for RasterLayer {}

#[cfg(test)]
mod tests;
