//! Conservative metadata proof of uniform color; never scans or owns source pixels.
use super::*;
impl RasterLayer {
    /// A known uniform layer-local region, or None when writes prevent a proof.
    /// False negatives are permitted; unchanged cells retain their exact initial
    /// color, which may differ from the layer's current implicit default.
    pub fn uniform_region(
        &self,
        area: Rect,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Rgba>> {
        check()?;
        if area.x0 < 0
            || area.y0 < 0
            || area.x1 <= area.x0
            || area.y1 <= area.y0
            || area.x1 > i64::from(self.width)
            || area.y1 > i64::from(self.height)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "uniform region outside layer",
            ));
        }
        if self.tiles.is_empty() {
            return Ok(Some(self.default));
        }
        let edge = i64::from(TILE_EDGE);
        let (x0, y0, x1, y1) = (
            area.x0 / edge,
            area.y0 / edge,
            (area.x1 - 1) / edge,
            (area.y1 - 1) / edge,
        );
        let count = (x1 - x0 + 1) as u64 * (y1 - y0 + 1) as u64;
        let mut color = None;
        let mut agree = |next| {
            let matches = color.is_none_or(|old| old == next);
            color = Some(next);
            matches
        };
        let known = |coord: TileCoord, tile: &Tile| {
            let local = area
                .intersect(&Rect::new(coord.tx * edge, coord.ty * edge, edge, edge).unwrap())?
                .translated(-coord.tx * edge, -coord.ty * edge)?;
            let region = i64::from(REGION_EDGE);
            for ry in local.y0 / region..=(local.y1 - 1) / region {
                for rx in local.x0 / region..=(local.x1 - 1) / region {
                    if tile.regions[ry as usize * regions::REGIONS_PER_EDGE + rx as usize] != 0 {
                        return None;
                    }
                }
            }
            Some(tile.initial)
        };
        if count <= self.tiles.len() as u64 {
            for ty in y0..=y1 {
                for tx in x0..=x1 {
                    check()?;
                    let coord = TileCoord { tx, ty };
                    let pixel = match self.tiles.get(&coord) {
                        Some(tile) => known(coord, tile),
                        None => Some(self.default),
                    };
                    if !pixel.is_some_and(&mut agree) {
                        return Ok(None);
                    }
                }
            }
        } else {
            let mut present = 0u64;
            for (&coord, tile) in &self.tiles {
                check()?;
                if coord.tx < x0 || coord.tx > x1 || coord.ty < y0 || coord.ty > y1 {
                    continue;
                }
                present += 1;
                if !known(coord, tile).is_some_and(&mut agree) {
                    return Ok(None);
                }
            }
            if present < count && !agree(self.default) {
                return Ok(None);
            }
        }
        Ok(color)
    }
}
