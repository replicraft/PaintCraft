//! Sparse straight-RGBA8 layers.
//!
//! A layer has finite bounds and an exact implicit default pixel inside them;
//! outside the bounds it contributes nothing. Pixels live in copy-on-write
//! 256×256 tiles addressed with Euclidean division. A missing tile equals the
//! default everywhere, including hidden RGB under alpha zero. Every allocation
//! (new tile or copy-on-write copy) is charged to an [`Admission`] so exhaustion
//! is an explicit `resource.limit` error rather than an abort.

use std::collections::HashMap;
use std::sync::Arc;

use paintcraft_types::{Error, ErrorCode, Rect, Result, Rgba};

pub const TILE_EDGE: u32 = 256;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Replace,
    SourceOver,
}

impl BlendMode {
    fn apply(self, src: Rgba, dst: Rgba) -> Rgba {
        match self {
            BlendMode::Replace => src,
            BlendMode::SourceOver => src.source_over(dst),
        }
    }
}

/// Byte budget for staged pixel storage; shared across one batch.
#[derive(Clone, Debug)]
pub struct Admission {
    remaining_pixels: u64,
}

impl Admission {
    pub fn new(limit_pixels: u64) -> Self {
        Self {
            remaining_pixels: limit_pixels,
        }
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
struct Tile(Vec<u8>);

impl Tile {
    fn filled(px: Rgba) -> Tile {
        Tile(px.0.iter().copied().cycle().take(TILE_PIXELS * 4).collect())
    }

    fn is_uniform(&self, px: Rgba) -> bool {
        self.0.as_chunks::<4>().0.iter().all(|c| *c == px.0)
    }

    fn at(&self, x: u32, y: u32) -> Rgba {
        let i = ((y * TILE_EDGE + x) * 4) as usize;
        Rgba([self.0[i], self.0[i + 1], self.0[i + 2], self.0[i + 3]])
    }

    fn put(&mut self, x: u32, y: u32, px: Rgba) {
        let i = ((y * TILE_EDGE + x) * 4) as usize;
        self.0[i..i + 4].copy_from_slice(&px.0);
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

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn default_pixel(&self) -> Rgba {
        self.default
    }

    /// Layer-local bounds.
    pub fn bounds(&self) -> Rect {
        Rect::from_origin_size(0, 0, self.width, self.height).expect("layers are non-empty")
    }

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

    fn tile_mut(&mut self, c: TileCoord, adm: &mut Admission) -> Result<&mut Tile> {
        use std::collections::hash_map::Entry;
        let default = self.default;
        match self.tiles.entry(c) {
            Entry::Occupied(e) => {
                let arc = e.into_mut();
                if Arc::strong_count(arc) > 1 {
                    adm.charge_tile()?; // copy-on-write allocates
                }
                Ok(Arc::make_mut(arc))
            }
            Entry::Vacant(e) => {
                adm.charge_tile()?;
                Ok(Arc::make_mut(e.insert(Arc::new(Tile::filled(default)))))
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
        if !self.tiles.contains_key(&c) && mode.apply(src, self.default) == self.default {
            return Ok(());
        }
        let tile = self.tile_mut(c, adm)?;
        let new = mode.apply(src, tile.at(x, y));
        tile.put(x, y, new);
        Ok(())
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
                if !self.tiles.contains_key(&c) && mode.apply(src, self.default) == self.default {
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
                let tile = self.tile_mut(c, adm)?;
                for y in part.y0..part.y1 {
                    for x in part.x0..part.x1 {
                        let (lx, ly) = (x.rem_euclid(edge) as u32, y.rem_euclid(edge) as u32);
                        let new = mode.apply(src, tile.at(lx, ly));
                        tile.put(lx, ly, new);
                    }
                }
            }
        }
        Ok(())
    }

    /// Every stored sample, row-major and tightly packed, one row per call.
    pub fn for_each_row(&self, mut sink: impl FnMut(&[u8])) {
        let mut row = vec![0u8; self.width as usize * 4];
        for y in 0..i64::from(self.height) {
            let (ty, ly) = tile_split(y);
            for (x, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (tx, lx) = tile_split(x as i64);
                let px = self
                    .tiles
                    .get(&TileCoord { tx, ty })
                    .map_or(self.default, |t| t.at(lx, ly));
                *out = px.0;
            }
            sink(&row);
        }
    }

    /// Non-default 256×256 blocks, in (by, bx) order, as `(bx, by, width, height, rgba)`.
    /// Blocks equal to the default (including hidden RGB) are omitted.
    pub fn blocks(&self) -> Vec<(u32, u32, u32, u32, Vec<u8>)> {
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
                    data.extend_from_slice(&tile.0[start..start + (w * 4) as usize]);
                }
                (bx, by, w, h, data)
            })
            .collect()
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
        let tile = self.tile_mut(
            TileCoord {
                tx: i64::from(bx),
                ty: i64::from(by),
            },
            adm,
        )?;
        for y in 0..h {
            let (s, d) = ((y * w * 4) as usize, (y * TILE_EDGE * 4) as usize);
            tile.0[d..d + (w * 4) as usize].copy_from_slice(&data[s..s + (w * 4) as usize]);
        }
        Ok(())
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
                Some(u) => Arc::ptr_eq(t, u) || t.0 == u.0,
                None => t.is_uniform(a.default),
            })
        };
        covers(self, other) && covers(other, self)
    }
}

impl Eq for RasterLayer {}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = Rgba([255, 0, 0, 255]);

    fn adm() -> Admission {
        Admission::new(1 << 24)
    }

    #[test]
    fn euclidean_tile_addressing_handles_negatives_and_edges() {
        assert_eq!(tile_split(0), (0, 0));
        assert_eq!(tile_split(255), (0, 255));
        assert_eq!(tile_split(256), (1, 0));
        assert_eq!(tile_split(-1), (-1, 255));
        assert_eq!(tile_split(-256), (-1, 0));
        assert_eq!(tile_split(-257), (-2, 255));
    }

    #[test]
    fn two_by_two_layer_stores_hidden_rgb() {
        let mut l = RasterLayer::new(2, 2, Rgba([0, 0, 255, 255]));
        let mut a = adm();
        l.blend_pixel(0, 0, Rgba([255, 0, 0, 0]), BlendMode::Replace, &mut a)
            .unwrap();
        assert_eq!(l.get(0, 0), Rgba([255, 0, 0, 0]));
        assert_eq!(l.get(1, 1), Rgba([0, 0, 255, 255]));
        let mut samples = Vec::new();
        l.for_each_row(|r| samples.extend_from_slice(r));
        assert_eq!(
            samples,
            [255, 0, 0, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255]
        );
    }

    #[test]
    fn writes_across_tile_boundaries_and_out_of_bounds_are_clipped() {
        let mut l = RasterLayer::new(300, 3, Rgba::TRANSPARENT);
        let mut a = adm();
        l.fill_rect(
            &Rect::new(254, 0, 4, 1).unwrap(),
            RED,
            BlendMode::Replace,
            &mut a,
        )
        .unwrap();
        l.blend_pixel(300, 0, RED, BlendMode::Replace, &mut a)
            .unwrap();
        l.blend_pixel(-1, 0, RED, BlendMode::Replace, &mut a)
            .unwrap();
        for x in 254..258 {
            assert_eq!(l.get(x, 0), RED);
        }
        assert_eq!(
            (l.get(253, 0), l.get(258, 0), l.get(299, 0)),
            (Rgba::TRANSPARENT, Rgba::TRANSPARENT, Rgba::TRANSPARENT)
        );
        assert_eq!(l.resident_tiles(), 2);
        let blocks = l.blocks();
        assert_eq!(
            blocks
                .iter()
                .map(|b| (b.0, b.1, b.2, b.3))
                .collect::<Vec<_>>(),
            [(0, 0, 256, 3), (1, 0, 44, 3)]
        );
    }

    #[test]
    fn default_valued_writes_do_not_allocate_and_equal_empty_layers() {
        let mut l = RasterLayer::new(512, 512, Rgba::TRANSPARENT);
        let mut a = adm();
        l.fill_rect(
            &l.bounds(),
            Rgba([9, 9, 9, 0]),
            BlendMode::SourceOver,
            &mut a,
        )
        .unwrap();
        assert_eq!(l.resident_tiles(), 0);
        l.blend_pixel(10, 10, RED, BlendMode::Replace, &mut a)
            .unwrap();
        l.blend_pixel(10, 10, Rgba::TRANSPARENT, BlendMode::Replace, &mut a)
            .unwrap();
        assert_eq!(l, RasterLayer::new(512, 512, Rgba::TRANSPARENT));
        assert!(l.blocks().is_empty());
        // Hidden RGB is not the default.
        l.blend_pixel(10, 10, Rgba([5, 0, 0, 0]), BlendMode::Replace, &mut a)
            .unwrap();
        assert_ne!(l, RasterLayer::new(512, 512, Rgba::TRANSPARENT));
        assert_eq!(l.blocks().len(), 1);
    }

    #[test]
    fn clones_share_tiles_until_written() {
        let mut a = adm();
        let mut original = RasterLayer::new(10, 10, Rgba::TRANSPARENT);
        original
            .blend_pixel(1, 1, RED, BlendMode::Replace, &mut a)
            .unwrap();
        let before = a.remaining_pixels;
        let mut copy = original.clone();
        assert_eq!(copy, original);
        copy.blend_pixel(2, 2, RED, BlendMode::Replace, &mut a)
            .unwrap();
        assert_eq!(
            before - a.remaining_pixels,
            TILE_PIXELS as u64,
            "copy-on-write is charged"
        );
        assert_eq!(original.get(2, 2), Rgba::TRANSPARENT);
        assert_ne!(copy, original);
    }

    #[test]
    fn exhausting_the_budget_is_a_resource_error() {
        let mut l = RasterLayer::new(1024, 1024, Rgba::TRANSPARENT);
        let mut a = Admission::new((TILE_PIXELS * 2) as u64);
        let e = l
            .fill_rect(&l.bounds(), RED, BlendMode::Replace, &mut a)
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::ResourceLimit);
    }

    #[test]
    fn blocks_round_trip_through_set_block() {
        let mut a = adm();
        let mut l = RasterLayer::new(300, 260, Rgba([1, 2, 3, 4]));
        l.fill_rect(
            &Rect::new(255, 255, 2, 2).unwrap(),
            RED,
            BlendMode::Replace,
            &mut a,
        )
        .unwrap();
        let mut m = RasterLayer::new(300, 260, Rgba([1, 2, 3, 4]));
        for (bx, by, w, h, data) in l.blocks() {
            assert_eq!(data.len() as u32, w * h * 4);
            m.set_block(bx, by, &data, &mut a).unwrap();
        }
        assert_eq!(l, m);
        assert!(m.set_block(2, 0, &[0; 4], &mut a).is_err());
        assert!(m.set_block(0, 0, &[0; 4], &mut a).is_err());
    }
}
