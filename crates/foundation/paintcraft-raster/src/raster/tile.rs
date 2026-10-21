//! Decoded allocation ownership. Pixel storage drops before its byte lease.
use super::*;
use paintcraft_core::types::resources::BudgetLease;

#[derive(Debug)]
pub(super) struct Tile {
    pub(super) pixels: Vec<u8>,
    _residency: Option<BudgetLease>,
}
impl Tile {
    /// Empty ownership-transfer sentinel; never exposed as a resident tile.
    pub(super) fn empty() -> Self {
        Self {
            pixels: Vec::new(),
            _residency: None,
        }
    }
    fn allocate(adm: &mut Admission) -> Result<(Vec<u8>, BudgetLease)> {
        adm.charge_tile()?;
        let lease = adm
            .residency
            .reserve(RasterLayer::TILE_BYTES as u64)
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "decoded tile residency limit exceeded",
                )
            })?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(RasterLayer::TILE_BYTES)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate raster tile"))?;
        if bytes.capacity() != RasterLayer::TILE_BYTES {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "raster allocator returned unexpected tile capacity",
            ));
        }
        Ok((bytes, lease))
    }
    pub(super) fn filled(px: Rgba, adm: &mut Admission) -> Result<Self> {
        let (mut bytes, lease) = Self::allocate(adm)?;
        bytes.resize(RasterLayer::TILE_BYTES, 0);
        if px != Rgba::TRANSPARENT {
            bytes.as_chunks_mut::<4>().0.fill(px.0);
        }
        Ok(Self {
            pixels: bytes,
            _residency: Some(lease),
        })
    }
    pub(super) fn copied(source: &Self, adm: &mut Admission) -> Result<Self> {
        let (mut bytes, lease) = Self::allocate(adm)?;
        bytes.extend_from_slice(&source.pixels);
        Ok(Self {
            pixels: bytes,
            _residency: Some(lease),
        })
    }
    pub(super) fn is_uniform(&self, px: Rgba) -> bool {
        self.pixels.as_chunks::<4>().0.iter().all(|c| *c == px.0)
    }
    pub(super) fn at(&self, x: u32, y: u32) -> Rgba {
        let i = ((y * TILE_EDGE + x) * 4) as usize;
        Rgba([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }
    pub(super) fn put(&mut self, x: u32, y: u32, px: Rgba) {
        let i = ((y * TILE_EDGE + x) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&px.0);
    }
}
