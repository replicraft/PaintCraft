use super::*;
use crate::raster::portable_pixels::decode::FixedBytes;
use crate::raster::portable_pixels::{EncodedTilePayload, Meter, PixelGeometry};
impl ReservedDecodeAdmission {
    pub(in crate::raster) fn portable_pending(
        &mut self,
        input: (Option<FixedBytes>, EncodedTilePayload<'_>),
        geometry: PixelGeometry,
        default: Rgba,
        meter: &mut Meter<'_>,
    ) -> Result<tile::PendingTile> {
        tile::PendingTile::new(
            &mut self.admission,
            &self.controls,
            input,
            geometry,
            default,
            meter,
        )
    }
}
