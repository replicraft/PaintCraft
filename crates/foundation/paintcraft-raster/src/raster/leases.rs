//! Explicit decoded tile ownership for recorded history; no mutable pixel aliases.
use super::*;

#[derive(Clone, Debug)]
pub struct TileLease(Arc<Tile>);
impl TileLease {
    pub fn bytes(&self) -> &[u8] {
        &self.0.pixels
    }
    pub fn version(&self) -> TileVersion {
        TileVersion(Arc::downgrade(&self.0))
    }
    /// Allocate one admitted full storage tile, initialize it, and privately write
    /// recorded content. Failed construction returns no lease or partial tile.
    pub fn materialize(
        default: Rgba,
        adm: &mut Admission,
        write: impl FnOnce(&mut [u8]) -> Result<()>,
    ) -> Result<Self> {
        let mut tile = Tile::filled(default, adm)?;
        tile.mark_all();
        write(&mut tile.pixels)?;
        Ok(Self(Arc::new(tile)))
    }
}
impl TileVersion {
    pub fn try_lease(&self) -> Option<TileLease> {
        self.0.upgrade().map(TileLease)
    }
    /// Actual decoded Vec capacity while this source allocation is still alive.
    pub fn resident_bytes(&self) -> usize {
        self.0.upgrade().map_or(0, |t| t.pixels.capacity())
    }
}
impl RasterLayer {
    pub fn tile_leases(&self) -> impl ExactSizeIterator<Item = (TileCoord, TileLease)> + '_ {
        self.tiles
            .iter()
            .map(|(c, t)| (*c, TileLease(Arc::clone(t))))
    }
    /// Install an already admitted immutable tile without copying its pixels.
    pub fn install_tile(&mut self, coord: TileCoord, lease: TileLease) -> Result<()> {
        if coord.tx < 0
            || coord.ty < 0
            || coord.tx >= i64::from(self.width.div_ceil(TILE_EDGE))
            || coord.ty >= i64::from(self.height.div_ceil(TILE_EDGE))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "recorded tile outside raster",
            ));
        }
        let valid_w = (i64::from(self.width) - coord.tx * i64::from(TILE_EDGE))
            .min(i64::from(TILE_EDGE)) as usize;
        let valid_h = (i64::from(self.height) - coord.ty * i64::from(TILE_EDGE))
            .min(i64::from(TILE_EDGE)) as usize;
        if (valid_w < TILE_EDGE as usize || valid_h < TILE_EDGE as usize)
            && lease
                .bytes()
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .any(|(i, px)| {
                    (i / TILE_EDGE as usize >= valid_h || i % TILE_EDGE as usize >= valid_w)
                        && *px != self.default.0
                })
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "recorded tile has invalid finite-edge padding",
            ));
        }
        self.tiles
            .try_reserve(1)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate tile index"))?;
        self.tiles.insert(coord, lease.0);
        Ok(())
    }
    /// Conservative index admission weight, distinct from pixel capacity or RSS.
    pub fn metadata_weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.tiles.capacity() * (std::mem::size_of::<(TileCoord, Arc<Tile>)>() + 32)
            + self.tiles.len() * std::mem::size_of::<Tile>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_decoded_leases_share_immutable_data_and_admit_cow_before_writing() {
        let lease =
            TileLease::materialize(Rgba([37, 19, 83, 0]), &mut Admission::new(65536), |data| {
                data[..4].copy_from_slice(&[1, 2, 3, 255]);
                Ok(())
            })
            .unwrap();
        let source = lease.version();
        let mut raster = RasterLayer::new(256, 256, Rgba([37, 19, 83, 0]));
        raster
            .install_tile(TileCoord { tx: 0, ty: 0 }, lease.clone())
            .unwrap();
        assert_eq!(
            raster
                .blend_pixel(
                    0,
                    0,
                    Rgba([4, 5, 6, 255]),
                    BlendMode::Replace,
                    &mut Admission::new(0)
                )
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        raster
            .blend_pixel(
                0,
                0,
                Rgba([4, 5, 6, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        assert_eq!(&lease.bytes()[..4], &[1, 2, 3, 255]);
        assert_eq!(raster.get(0, 0), Rgba([4, 5, 6, 255]));
        assert_eq!(source.resident_bytes(), RasterLayer::TILE_BYTES);
        drop(lease);
        assert_eq!(source.resident_bytes(), 0);
    }
    #[test]
    fn racing_weak_readers_cannot_cause_unadmitted_pixel_copies() {
        use std::sync::Barrier;
        for _ in 0..64 {
            let mut raster = RasterLayer::new(256, 256, Rgba::TRANSPARENT);
            raster
                .blend_pixel(
                    0,
                    0,
                    Rgba([1, 2, 3, 255]),
                    BlendMode::Replace,
                    &mut Admission::new(65536),
                )
                .unwrap();
            let source = raster.tile_leases().next().unwrap().1.version();
            let start = Barrier::new(2);
            let finish = Barrier::new(2);
            std::thread::scope(|scope| {
                let reader = scope.spawn(|| {
                    start.wait();
                    let lease = source.try_lease();
                    finish.wait();
                    if let Some(lease) = lease {
                        assert_eq!(&lease.bytes()[..4], &[1, 2, 3, 255]);
                        true
                    } else {
                        false
                    }
                });
                start.wait();
                let result = raster.blend_pixel(
                    0,
                    0,
                    Rgba([4, 5, 6, 255]),
                    BlendMode::Replace,
                    &mut Admission::new(0),
                );
                finish.wait();
                let pinned = reader.join().unwrap();
                match result {
                    Ok(()) => {
                        assert!(!pinned);
                        assert_eq!(source.resident_bytes(), 0);
                    }
                    Err(error) => {
                        assert_eq!(error.code, ErrorCode::ResourceLimit);
                        assert!(pinned);
                    }
                }
            });
        }
    }
}
