//! Concrete lexical entry path; no Entry, visitor, callback buffer or raw map escapes.
use super::*;
use crate::raster::portable_pixels::{EncodedTilePayload, Meter, PixelGeometry, bad, limit};
impl TileTable {
    pub(in crate::raster) fn decode_portable(
        &mut self,
        mut g: PixelGeometry,
        payload: EncodedTilePayload<'_>,
        admission: &mut ReservedDecodeAdmission,
        scratch: &BudgetScope,
        default: Rgba,
        meter: &mut Meter<'_>,
    ) -> Result<()> {
        // Fresh no-delete provenance is private to the consuming builder.
        if self.len() >= self.capacity() {
            return Err(limit());
        }
        if g.coord
            != (TileCoord {
                tx: i64::from(g.bx),
                ty: i64::from(g.by),
            })
        {
            return Err(bad());
        }
        let first = self.values.is_empty();
        meter.step(16 + self.bucket_upper as u64)?;
        let hash_map::Entry::Vacant(entry) = self.values.entry(g.coord) else {
            return Err(bad());
        };
        meter.debit(64 + if first { self.bucket_upper as u64 } else { 0 })?;
        crate::raster::portable_pixels::decode::fence(payload, &mut g, meter)?;
        let decoded = crate::raster::portable_pixels::decode::expand(payload, g, scratch, meter)?;
        let pending = admission.portable_pending((decoded, payload), g, default, meter)?;
        meter.step(160)?;
        let regions = [crate::raster::regions::next_revision(); 16];
        // Keep PendingTile's payload-before-credit order across this last fallible
        // callback before splitting its ownership for infallible Arc construction.
        meter.step(64 + meter.tile_header_bytes)?;
        let (tile, scratch_account) = pending.into_tile(regions, default);
        let tile = Arc::new(tile);
        meter.step(24 + self.bucket_upper as u64)?;
        entry.insert(tile);
        meter.debit(32)?;
        drop(scratch_account);
        meter.step(32)?;
        Ok(())
    }
}
