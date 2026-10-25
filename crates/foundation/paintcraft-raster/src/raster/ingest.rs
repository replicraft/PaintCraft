//! Cancellable external-pixel ingestion, with complete tile admission before decode.
use super::*;

impl RasterLayer {
    /// Build exact straight RGBA tiles without a second packed image or tile scratch.
    /// Scan once to reserve the complete private tile peak before allocation.
    /// The existing codec constructor is unchanged pending performance qualification.
    pub fn from_rgba_checked(
        width: u32,
        height: u32,
        default: Rgba,
        rgba: &[u8],
        admission: &mut Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || bytes != Some(rgba.len() as u64) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "pixel buffer does not match the layer size",
            ));
        }
        let row = |tx: u32, ty: u32, y: u32| {
            let w = (width - tx * TILE_EDGE).min(TILE_EDGE) as usize;
            let start = ((u64::from(ty * TILE_EDGE + y) * u64::from(width)
                + u64::from(tx * TILE_EDGE))
                * 4) as usize;
            &rgba[start..start + w * 4]
        };
        let populated = |tx: u32, ty: u32| -> Result<bool> {
            for y in 0..(height - ty * TILE_EDGE).min(TILE_EDGE) {
                check()?;
                if row(tx, ty, y)
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| *pixel != default.0)
                {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        let mut tiles = 0u64;
        for ty in 0..height.div_ceil(TILE_EDGE) {
            for tx in 0..width.div_ceil(TILE_EDGE) {
                if populated(tx, ty)? {
                    tiles += 1;
                }
            }
        }
        admission.reserve_tiles(tiles)?;
        let mut layer = Self::new(width, height, default);
        layer
            .tiles
            .try_reserve(
                usize::try_from(tiles)
                    .map_err(|_| Error::new(ErrorCode::ResourceLimit, "tile count overflows"))?,
            )
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "cannot allocate raster tile index",
                )
            })?;
        for ty in 0..height.div_ceil(TILE_EDGE) {
            for tx in 0..width.div_ceil(TILE_EDGE) {
                if !populated(tx, ty)? {
                    continue;
                }
                check()?;
                let mut tile = Tile::filled(default, admission)?;
                tile.mark_all();
                for y in 0..(height - ty * TILE_EDGE).min(TILE_EDGE) {
                    check()?;
                    let source = row(tx, ty, y);
                    let start = (y * TILE_EDGE * 4) as usize;
                    tile.pixels[start..start + source.len()].copy_from_slice(source);
                }
                layer.tiles.insert(
                    TileCoord {
                        tx: i64::from(tx),
                        ty: i64::from(ty),
                    },
                    Arc::new(tile),
                );
            }
        }
        check()?;
        Ok(layer)
    }
}

#[cfg(test)]
mod tests;
