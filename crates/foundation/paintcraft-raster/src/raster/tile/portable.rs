//! Only explicit original reserved decode ownership, no legacy allocation fallback.
use super::*;
use crate::raster::portable_pixels::decode::FixedBytes;
use crate::raster::portable_pixels::{
    EncodedTilePayload, Meter, PixelGeometry, PortablePixelEncoding, bad, limit,
};
#[repr(C)]
pub(in crate::raster) struct PendingTile {
    pixels: Vec<u8>,
    allocation: Allocation,
    scratch: Option<BudgetAccount>,
}
pub(super) fn allocation_bytes() -> usize {
    std::mem::size_of::<Allocation>()
}
impl PendingTile {
    pub(in crate::raster) fn new(
        adm: &mut Admission,
        controls: &super::super::decode_factories::DecodeControls,
        input: (Option<FixedBytes>, EncodedTilePayload<'_>),
        g: PixelGeometry,
        default: Rgba,
        meter: &mut Meter<'_>,
    ) -> Result<Self> {
        meter.step(64)?;
        adm.charge_tile()?;
        let reservation = adm.reservation.as_ref().ok_or_else(limit)?;
        let gauge = adm.allocation_scope.as_ref().ok_or_else(limit)?.clone();
        let lease = reservation
            .reserve(RasterLayer::TILE_BYTES as u64)
            .map_err(|error| {
                let state = gauge.lock().unwrap_or_else(|e| e.into_inner());
                let Some(next) = state.live.checked_add(RasterLayer::TILE_BYTES as u64) else {
                    return limit();
                };
                error.with_allocation_requirement(
                    AllocationClass::DecodedTiles,
                    next.max(state.peak),
                )
            })?;
        let mut allocation = Allocation::new(lease, gauge)?;
        allocation._decode_controls = controls.clone();
        let (decoded, payload) = input;
        if g.decoded == RasterLayer::TILE_BYTES && decoded.is_some() {
            meter.debit(64)?;
            let FixedBytes { bytes, account } = decoded.ok_or_else(bad)?;
            return Ok(Self {
                pixels: bytes,
                allocation,
                scratch: Some(account),
            });
        }
        meter.debit(64)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(RasterLayer::TILE_BYTES)
            .map_err(|_| limit())?;
        if pixels.capacity() != RasterLayer::TILE_BYTES {
            return Err(limit());
        }
        let data = decoded
            .as_ref()
            .map_or(payload.bytes, |d| d.bytes.as_slice());
        if g.decoded == RasterLayer::TILE_BYTES && payload.encoding == PortablePixelEncoding::Raw {
            meter.step(32 + g.decoded as u64)?;
            pixels.extend_from_slice(data);
        } else {
            meter.step(2048)?;
            let pattern: [[u8; 4]; 256] = [default.0; 256];
            let row = pattern.as_flattened();
            for y in 0..256usize {
                meter.step(1056)?;
                if RasterLayer::TILE_BYTES - pixels.len() < 1024 {
                    return Err(limit());
                }
                if y < g.h as usize {
                    let n = g.w as usize * 4;
                    pixels.extend_from_slice(&data[y * n..(y + 1) * n]);
                    pixels.extend_from_slice(&row[n..]);
                } else {
                    pixels.extend_from_slice(row);
                }
            }
        }
        if pixels.len() != RasterLayer::TILE_BYTES {
            return Err(bad());
        }
        let scratch = if let Some(d) = decoded {
            let FixedBytes { bytes, account } = d;
            drop(bytes);
            Some(account)
        } else {
            None
        };
        Ok(Self {
            pixels,
            allocation,
            scratch,
        })
    }
    pub(in crate::raster) fn into_tile(
        self,
        regions: [u64; 16],
        default: Rgba,
    ) -> (Tile, Option<BudgetAccount>) {
        (
            Tile {
                pixels: self.pixels,
                _residency: Some(self.allocation),
                initial: default,
                regions,
            },
            self.scratch,
        )
    }
}

#[cfg(test)]
mod tests;
