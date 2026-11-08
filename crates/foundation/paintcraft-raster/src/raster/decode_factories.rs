//! Explicit reserved decode allocation factories. No default realm or raw escape.
use super::*;
use std::sync::Mutex;

/// Caller supplies every factory, even those unused by block decode.
#[derive(Clone, Debug)]
pub struct ReservedDecodeFactories {
    pub selection_budget: SharedBudget,
    pub coverage_budget: SharedBudget,
    pub paint_metadata_scope: BudgetScope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservedDecodeControlQuote {
    pub gauge_arc_bytes: u64,
    pub control_arc_bytes: u64,
    pub fixed_bytes: u64,
    /// Inline returned owner, charged by its containing source/reader state.
    pub handle_bytes: u64,
    /// Actual Tile Arc header/data, EXCLUDING pixels; separate source census.
    /// Constructor fixed_bytes does NOT fund these bytes.
    pub tile_header_bytes: u64,
}
/// No Arc/Weak escape. into_inner frees the last control Arc before releasing
/// its account; all owners use this same last-owner path, including races.
#[derive(Clone, Debug, Default)]
pub(super) struct DecodeControls(Option<Arc<BudgetAccount>>);
impl DecodeControls {
    fn new(account: BudgetAccount) -> Self {
        Self(Some(Arc::new(account)))
    }
}
impl Drop for DecodeControls {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take()
            && let Some(account) = Arc::into_inner(owner)
        {
            drop(account);
        }
    }
}
/// Admission ends before control credit; allocations carry the same control
/// owner after this wrapper drops. No Deref/AsRef/into/clone Admission escape.
#[derive(Debug)]
pub struct ReservedDecodeAdmission {
    admission: Admission,
    controls: DecodeControls,
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn arc_bytes<T>() -> Result<u64> {
    let alignment = std::mem::align_of::<T>().max(std::mem::align_of::<usize>());
    let offset = (2 * std::mem::size_of::<usize>())
        .checked_next_multiple_of(std::mem::align_of::<T>())
        .ok_or_else(|| limit("decode control offset"))?;
    let bytes = offset
        .checked_add(std::mem::size_of::<T>())
        .and_then(|n| n.checked_next_multiple_of(alignment))
        .ok_or_else(|| limit("decode control layout"))?;
    u64::try_from(bytes).map_err(|_| limit("decode control extent"))
}
impl ReservedDecodeAdmission {
    pub fn control_quote() -> Result<ReservedDecodeControlQuote> {
        let gauge_arc_bytes = arc_bytes::<Mutex<tile::Scope>>()?;
        let control_arc_bytes = arc_bytes::<BudgetAccount>()?;
        Ok(ReservedDecodeControlQuote {
            gauge_arc_bytes,
            control_arc_bytes,
            fixed_bytes: gauge_arc_bytes
                .checked_add(control_arc_bytes)
                .ok_or_else(|| limit("decode controls overflow"))?,
            handle_bytes: std::mem::size_of::<Self>() as u64,
            tile_header_bytes: arc_bytes::<Tile>()?,
        })
    }
    pub(super) fn new(
        limit_pixels: u64,
        reservation: BudgetReservation,
        factories: ReservedDecodeFactories,
        control_scope: &BudgetScope,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let quote = Self::control_quote()?;
        let mut charge = control_scope.account();
        charge.resize(quote.fixed_bytes)?;
        check()?;
        // Local drop order on cancellation: gauge first, then controls/account.
        let controls = DecodeControls::new(charge);
        let gauge = Arc::new(Mutex::new(tile::Scope::default()));
        check()?;
        Ok(Self {
            admission: Admission {
                remaining_pixels: limit_pixels,
                paint_metadata_limit: DEFAULT_PAINT_METADATA_BYTES,
                paint_metadata_scope: factories.paint_metadata_scope,
                flood_scratch_limit: crate::flood::DEFAULT_FLOOD_SCRATCH_BYTES,
                stroke_scratch_limit: crate::shape::DEFAULT_STROKE_SCRATCH_BYTES,
                residency: reservation.budget(), //clone ORIGINAL realm; no reserve fallback
                selection_budget: factories.selection_budget,
                coverage_budget: factories.coverage_budget,
                reservation: Some(reservation),
                allocation_scope: Some(gauge),
            },
            controls,
        })
    }
    pub fn reserve_tiles(&mut self, tiles: u64) -> Result<()> {
        // Some is invariant and cannot be cleared through this public wrapper.
        self.admission.reserve_tiles(tiles)
    }
    pub fn reservation_capacity(&self) -> u64 {
        self.admission
            .reservation
            .as_ref()
            .expect("explicit decode reservation")
            .capacity()
    }
    pub fn remaining_pixels(&self) -> u64 {
        self.admission.remaining_pixels
    }
    /// Each decoded block gets fresh admitted storage, including replacements.
    /// Metadata/index and Tile Arc headers need the reader's separate declaration.
    /// Cancellation after local allocation never exposes an unowned tile.
    pub fn write_block(
        &mut self,
        layer: &mut RasterLayer,
        bx: u32,
        by: u32,
        data: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        let bad = |m: &str| Error::new(ErrorCode::CodecFailed, m.to_owned());
        if bx.checked_mul(TILE_EDGE).is_none_or(|x| x >= layer.width)
            || by.checked_mul(TILE_EDGE).is_none_or(|y| y >= layer.height)
        {
            return Err(bad("block outside layer"));
        }
        let w = (layer.width - bx * TILE_EDGE).min(TILE_EDGE);
        let h = (layer.height - by * TILE_EDGE).min(TILE_EDGE);
        if data.len() != (w * h * 4) as usize {
            return Err(bad("block length does not match its extent"));
        }
        let coord = TileCoord {
            tx: i64::from(bx),
            ty: i64::from(by),
        };
        if !layer.tiles.contains_key(&coord) {
            layer
                .tiles
                .try_reserve(1)
                .map_err(|_| limit("cannot allocate decode raster index"))?;
        }
        let mut tile = Tile::filled(layer.default, &mut self.admission)?;
        tile.attach_decode_controls(
            self.admission
                .allocation_scope
                .as_ref()
                .expect("explicit decode gauge"),
            &self.controls,
        );
        check()?;
        tile.mark_all();
        for y in 0..h {
            let (s, d) = ((y * w * 4) as usize, (y * TILE_EDGE * 4) as usize);
            tile.pixels[d..d + (w * 4) as usize].copy_from_slice(&data[s..s + (w * 4) as usize]);
        }
        layer.tiles.insert(coord, Arc::new(tile));
        Ok(())
    }
}
#[cfg(test)]
mod tests;
