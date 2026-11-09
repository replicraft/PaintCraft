//! Portable encoded pixel intake. Encoding descriptions are not source authority.
use super::*;
use paintcraft_core::types::resources::BudgetLease;
use std::mem::size_of;
pub(super) mod decode;
#[cfg(test)]
mod tests;
pub const PORTABLE_PIXEL_CONTROL_BYTES: u64 = 4096;
// Conservative control declaration; std Entry uses an unspecified Rust layout.
const ENTRY_CONTROL_ENVELOPE: usize = 40;
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PortableLayerConfig {
    pub width: u32,
    pub height: u32,
    pub default: Rgba,
    pub records: usize,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortablePixelEncoding {
    Raw,
    Runs,
    Lz4,
    RunsLz4,
}
/// Borrowed encoding description only, never an authenticated Source8 recipe.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EncodedTilePayload<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) encoding: PortablePixelEncoding,
}
impl<'a> EncodedTilePayload<'a> {
    pub fn new(encoding: PortablePixelEncoding, bytes: &'a [u8]) -> Self {
        Self { bytes, encoding }
    }
}
impl std::fmt::Debug for EncodedTilePayload<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncodedTilePayload")
            .field("encoding", &self.encoding)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct PixelGeometry {
    pub bx: u32,
    pub by: u32,
    pub w: u32,
    pub h: u32,
    pub decoded: usize,
    pub encoded: usize,
    pub runs_extent: usize,
    pub coord: TileCoord,
}
/// Defining inline geometry control, not an allocation grant.
pub fn portable_pixel_geometry_bytes() -> u64 {
    size_of::<PixelGeometry>() as u64
}
pub(super) fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "portable pixel admission")
}
pub(super) fn bad() -> Error {
    Error::new(ErrorCode::CodecFailed, "invalid portable pixel payload")
}
#[repr(C)]
pub(super) struct Meter<'a> {
    pub check: &'a dyn Fn() -> Result<()>,
    pub spend: &'a dyn Fn(u64) -> Result<()>,
    pub charged: u64,
    pub tile_header_bytes: u64,
}
impl Meter<'_> {
    pub fn debit(&mut self, n: u64) -> Result<()> {
        let next = self.charged.checked_add(n).ok_or_else(limit)?;
        (self.spend)(n)?;
        self.charged = next;
        Ok(())
    }
    pub fn step(&mut self, n: u64) -> Result<()> {
        self.debit(n.checked_add(512).ok_or_else(limit)?)?;
        (self.check)()
    }
}
/// No Vec/account/mutable sink escape. External M must outlive every returned
/// Raster/Tile/Weak alias and fund the complete simultaneous source union.
/// This standalone primitive is not a closed R3 source carrier.
/// ```compile_fail
/// use paintcraft_raster::PortableLayerBuilder;
/// fn escape(b: PortableLayerBuilder<'_>) { let _ = b.into_vec(); }
/// ```
#[repr(C)]
pub struct PortableLayerBuilder<'a> {
    raster: RasterLayer,
    admission: &'a mut ReservedDecodeAdmission,
    scratch: &'a BudgetScope,
    metadata: &'a BudgetAccount,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    records: usize,
    written: usize,
    index_allocation: u64,
    index_buckets: usize,
    _control: BudgetAccount,
}
impl std::fmt::Debug for PortableLayerBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableLayerBuilder")
            .field("records", &self.records)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}
fn control_sum() -> Result<usize> {
    let lz4 = 32 * size_of::<usize>()
        + 6 * size_of::<&[u8]>()
        + 16
        + 8
        + size_of::<lz4_flex::block::DecompressError>();
    [
        size_of::<PortableLayerBuilder<'static>>(),
        size_of::<EncodedTilePayload<'static>>(),
        size_of::<PixelGeometry>(),
        2 * size_of::<decode::FixedBytes>(),
        size_of::<tile::PendingTile>(),
        size_of::<std::collections::hash_map::Entry<'static, TileCoord, Arc<Tile>>>()
            .max(ENTRY_CONTROL_ENVELOPE),
        size_of::<Tile>(),
        size_of::<Arc<Tile>>(),
        tile::portable_allocation_bytes(),
        size_of::<BudgetAccount>(),
        1024,
        lz4,
        size_of::<Meter<'static>>(),
        size_of::<[u64; 8]>(),
        size_of::<[usize; 8]>(),
        2 * size_of::<Rgba>(),
        128,
        size_of::<lz4_flex::block::DecompressError>(),
        size_of::<PortableLayerConfig>(),
    ]
    .into_iter()
    .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(limit))
}
impl ReservedDecodeAdmission {
    pub fn portable_layer_builder_checked<'a>(
        &'a mut self,
        config: PortableLayerConfig,
        scratch: &'a BudgetScope,
        metadata: &'a BudgetAccount,
        check: &'a dyn Fn() -> Result<()>,
        spend: &'a dyn Fn(u64) -> Result<()>,
    ) -> Result<PortableLayerBuilder<'a>> {
        let mut meter = Meter {
            check,
            spend,
            charged: 0,
            tile_header_bytes: 0,
        };
        meter.debit(64)?;
        if control_sum()? as u64 > PORTABLE_PIXEL_CONTROL_BYTES {
            return Err(limit());
        }
        let mut control = scratch.account();
        control.resize(PORTABLE_PIXEL_CONTROL_BYTES)?;
        let raster = RasterLayer::new_decode_index_precharged_checked(
            config.width,
            config.height,
            config.default,
            config.records,
            metadata,
            check,
            spend,
        )?;
        meter.debit(32)?;
        let index = raster.retained_tile_table_quote()?;
        Ok(PortableLayerBuilder {
            raster,
            admission: self,
            scratch,
            metadata,
            check,
            spend,
            records: config.records,
            written: 0,
            index_allocation: index.allocation_upper,
            index_buckets: index.bucket_upper,
            _control: control,
        })
    }
}
impl PortableLayerBuilder<'_> {
    pub fn block(mut self, bx: u32, by: u32, payload: EncodedTilePayload<'_>) -> Result<Self> {
        let mut meter = Meter {
            check: self.check,
            spend: self.spend,
            charged: 0,
            tile_header_bytes: 0,
        };
        meter.step(64)?;
        let header = ReservedDecodeAdmission::control_quote()?.tile_header_bytes;
        meter.tile_header_bytes = header;
        let full = header
            .checked_add(BudgetLease::control_allocation_quote()?)
            .and_then(|n| n.checked_mul(self.records as u64))
            .and_then(|n| n.checked_add(self.index_allocation))
            .ok_or_else(limit)?;
        if self.metadata.bytes() < full {
            return Err(limit());
        }
        let current = self.raster.retained_tile_table_quote()?;
        if current.allocation_upper != self.index_allocation
            || current.bucket_upper != self.index_buckets
        {
            return Err(limit());
        }
        if self.written >= self.records {
            return Err(bad());
        }
        let x = bx
            .checked_mul(TILE_EDGE)
            .filter(|x| *x < self.raster.width)
            .ok_or_else(bad)?;
        let y = by
            .checked_mul(TILE_EDGE)
            .filter(|y| *y < self.raster.height)
            .ok_or_else(bad)?;
        let (w, h) = (
            (self.raster.width - x).min(TILE_EDGE),
            (self.raster.height - y).min(TILE_EDGE),
        );
        let g = PixelGeometry {
            bx,
            by,
            w,
            h,
            decoded: (w * h * 4) as usize,
            encoded: payload.bytes.len(),
            runs_extent: 0,
            coord: TileCoord {
                tx: i64::from(bx),
                ty: i64::from(by),
            },
        };
        let default = self.raster.default;
        self.raster.tiles.decode_portable(
            g,
            payload,
            &mut *self.admission,
            self.scratch,
            default,
            &mut meter,
        )?;
        self.written += 1;
        Ok(self)
    }
    pub fn finish(self) -> Result<RasterLayer> {
        let mut meter = Meter {
            check: self.check,
            spend: self.spend,
            charged: 0,
            tile_header_bytes: 0,
        };
        meter.step(32)?;
        if self.written != self.records {
            return Err(bad());
        }
        Ok(self.raster)
    }
}
