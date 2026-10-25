//! Frozen external-RGBA placement onto a finite existing layer.
use super::*;
use paintcraft_core::types::{
    clipboard::Image,
    policy::{CANVAS_SIDE_MAX, COORD_ABS_MAX},
    resources::BudgetAccount,
};
use std::mem::size_of;

const READS_MAX: u64 = 67_108_864;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Same,
    Default,
    Tile,
}
/// Private, source-bound paste metadata. No decoded tiles allocate until replay
/// has reserved the complete output peak. Image clones retain its admitted bytes.
#[derive(Debug)]
pub struct RgbaPastePlan {
    source: RasterLayer,
    image: Image,
    destination: Rect,
    old: Rect,
    size: (u32, u32),
    shift: (i64, i64),
    mode: BlendMode,
    key: Option<Rgba>,
    tiles: HashMap<TileCoord, Change>,
    reads: u64,
    _charge: BudgetAccount,
}
impl RgbaPastePlan {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        source: &RasterLayer,
        image: Image,
        origin: (i64, i64),
        mode: BlendMode,
        key: Option<Rgba>,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if origin.0.unsigned_abs() > COORD_ABS_MAX as u64
            || origin.1.unsigned_abs() > COORD_ABS_MAX as u64
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "paste coordinates exceed policy",
            ));
        }
        if source.width() == 0 || source.height() == 0 {
            return Err(limit());
        }
        let area = Rect::from_origin_size(origin.0, origin.1, image.width(), image.height())
            .ok_or_else(limit)?;
        let ox = area.x0.min(0);
        let oy = area.y0.min(0);
        let width = area.x1.max(i64::from(source.width())) - ox;
        let height = area.y1.max(i64::from(source.height())) - oy;
        if width <= 0
            || height <= 0
            || width > i64::from(CANVAS_SIDE_MAX)
            || height > i64::from(CANVAS_SIDE_MAX)
        {
            return Err(limit());
        }
        let shift = (-ox, -oy);
        let mut plan = Self {
            source: source.clone(),
            image,
            destination: area.translated(shift.0, shift.1).ok_or_else(limit)?,
            old: source
                .bounds()
                .translated(shift.0, shift.1)
                .ok_or_else(limit)?,
            size: (width as u32, height as u32),
            shift,
            mode,
            key,
            tiles: HashMap::new(),
            reads: 0,
            _charge: admission.paint_metadata_account(),
        };
        plan._charge.resize(size_of::<Self>() as u64)?;
        let mut metadata =
            metadata::Metadata::new(size_of::<Self>(), admission.paint_metadata_limit())?;
        // Same-origin unchanged tiles remain COW-shared. A rebased source maps
        // only explicit tiles; implicit finite defaults need no source scans.
        for coord in source.tiles.keys() {
            if shift != (0, 0) {
                if let Some(r) = tile_rect(*coord).intersect(&source.bounds()) {
                    plan.add(
                        r.translated(shift.0, shift.1).ok_or_else(limit)?,
                        &mut metadata,
                        check,
                    )?;
                }
            } else if let Some(r) = tile_rect(*coord).intersect(&plan.bounds())
                && r.intersect(&source.bounds()) != Some(r)
            {
                plan.add(r, &mut metadata, check)?;
            }
        }
        // Outside the old finite extent is transparent, regardless of its default.
        if source.default_pixel() != Rgba::TRANSPARENT {
            let b = plan.bounds();
            let o = plan.old;
            for r in [
                Rect::new(0, 0, b.width(), o.y0),
                Rect::new(0, o.y1, b.width(), b.y1 - o.y1),
                Rect::new(0, o.y0, o.x0, o.height()),
                Rect::new(o.x1, o.y0, b.x1 - o.x1, o.height()),
            ]
            .into_iter()
            .flatten()
            {
                plan.add(r, &mut metadata, check)?;
            }
        }
        plan.add(plan.destination, &mut metadata, check)?;
        let bounds = plan.bounds();
        for (&coord, change) in &mut plan.tiles {
            let region = tile_rect(coord).intersect(&bounds).expect("clipped tile");
            let mut differs = false;
            let mut nondefault = false;
            for y in region.y0..region.y1 {
                check()?;
                for x in region.x0..region.x1 {
                    if x % 64 == 0 {
                        check()?;
                    }
                    let color = sample(
                        &plan.source,
                        &plan.image,
                        plan.old,
                        plan.destination,
                        plan.mode,
                        plan.key,
                        x,
                        y,
                    );
                    let baseline = if plan.shift == (0, 0) {
                        plan.source
                            .tiles
                            .get(&coord)
                            .map_or(plan.source.default_pixel(), |tile| {
                                tile.at(
                                    (x % i64::from(TILE_EDGE)) as u32,
                                    (y % i64::from(TILE_EDGE)) as u32,
                                )
                            })
                    } else {
                        plan.source.default_pixel()
                    };
                    differs |= color != baseline;
                    nondefault |= color != plan.source.default_pixel();
                }
            }
            *change = if !differs {
                Change::Same
            } else if nondefault {
                Change::Tile
            } else {
                Change::Default
            };
        }
        check()?;
        Ok(plan)
    }
    fn bounds(&self) -> Rect {
        Rect::from_origin_size(0, 0, self.size.0, self.size.1).unwrap()
    }
    fn add(
        &mut self,
        area: Rect,
        metadata: &mut metadata::Metadata,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let Some(area) = area.intersect(&self.bounds()) else {
            return Ok(());
        };
        for ty in area.y0 / i64::from(TILE_EDGE)..=(area.y1 - 1) / i64::from(TILE_EDGE) {
            check()?;
            for tx in area.x0 / i64::from(TILE_EDGE)..=(area.x1 - 1) / i64::from(TILE_EDGE) {
                let coord = TileCoord { tx, ty };
                if !self.tiles.contains_key(&coord) {
                    let r = tile_rect(coord).intersect(&self.bounds()).unwrap();
                    self.reads = self
                        .reads
                        .checked_add(r.width() as u64 * r.height() as u64)
                        .ok_or_else(limit)?;
                    if self.reads > READS_MAX {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "paste sample budget exceeded",
                        ));
                    }
                    metadata.table(&mut self.tiles, &mut self._charge)?;
                    self.tiles.insert(coord, Change::Same);
                }
            }
        }
        Ok(())
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.size
    }
    /// Relative to the original layer's origin; add this to its document origin.
    pub fn origin_offset(&self) -> (i64, i64) {
        (-self.shift.0, -self.shift.1)
    }
    pub fn required_tiles(&self) -> u64 {
        self.tiles.values().filter(|c| **c == Change::Tile).count() as u64
    }
    pub fn sampled_pixels(&self) -> u64 {
        self.reads
    }
    pub fn replay(
        &self,
        admission: &mut Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<RasterLayer> {
        check()?;
        admission.reserve_tiles(self.required_tiles())?;
        let mut output = if self.shift == (0, 0) {
            let mut source = self.source.clone();
            source.expand_to(self.size.0, self.size.1)?;
            source
        } else {
            RasterLayer::new(self.size.0, self.size.1, self.source.default_pixel())
        };
        output
            .tiles
            .try_reserve(self.required_tiles() as usize)
            .map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot allocate paste tile index")
            })?;
        for (&coord, change) in &self.tiles {
            check()?;
            match change {
                Change::Same => {}
                Change::Default => {
                    output.tiles.remove(&coord);
                }
                Change::Tile => {
                    let mut tile = Tile::filled(self.source.default_pixel(), admission)?;
                    tile.mark_all();
                    let region = tile_rect(coord).intersect(&self.bounds()).unwrap();
                    for y in region.y0..region.y1 {
                        check()?;
                        for x in region.x0..region.x1 {
                            if x % 64 == 0 {
                                check()?;
                            }
                            let color = sample(
                                &self.source,
                                &self.image,
                                self.old,
                                self.destination,
                                self.mode,
                                self.key,
                                x,
                                y,
                            );
                            let at = ((y % i64::from(TILE_EDGE)) * i64::from(TILE_EDGE)
                                + x % i64::from(TILE_EDGE))
                                as usize
                                * 4;
                            tile.pixels[at..at + 4].copy_from_slice(&color.0);
                        }
                    }
                    output.tiles.insert(coord, Arc::new(tile));
                }
            }
        }
        check()?;
        Ok(output)
    }
}
#[allow(clippy::too_many_arguments)]
fn sample(
    source: &RasterLayer,
    image: &Image,
    old: Rect,
    destination: Rect,
    mode: BlendMode,
    key: Option<Rgba>,
    x: i64,
    y: i64,
) -> Rgba {
    let dst = if old.contains(x, y) {
        source.get(x - old.x0, y - old.y0)
    } else {
        Rgba::TRANSPARENT
    };
    if !destination.contains(x, y) {
        return dst;
    }
    let at = (((y - destination.y0) as u64 * u64::from(image.width())
        + (x - destination.x0) as u64)
        * 4) as usize;
    let src = Rgba(image.rgba()[at..at + 4].try_into().expect("admitted image"));
    if key.is_some_and(|key| key.0[..3] == src.0[..3]) {
        dst
    } else {
        blend(mode, src, dst)
    }
}
fn tile_rect(coord: TileCoord) -> Rect {
    Rect::new(
        coord.tx * i64::from(TILE_EDGE),
        coord.ty * i64::from(TILE_EDGE),
        i64::from(TILE_EDGE),
        i64::from(TILE_EDGE),
    )
    .unwrap()
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "paste exceeds layer extent policy",
    )
}
#[cfg(test)]
mod tests;
