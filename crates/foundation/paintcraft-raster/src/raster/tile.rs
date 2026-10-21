//! Decoded allocation ownership. Pixel storage drops before its byte lease.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetLease};
use std::sync::Mutex;
#[derive(Debug, Default)]
pub(super) struct Scope {
    live: u64,
    peak: u64,
}
#[derive(Debug)]
struct Allocation {
    budget: Option<BudgetLease>,
    scope: Arc<Mutex<Scope>>,
}
impl Allocation {
    fn new(budget: BudgetLease, scope: Arc<Mutex<Scope>>) -> Result<Self> {
        {
            let mut state = scope.lock().unwrap_or_else(|error| error.into_inner());
            state.live = state
                .live
                .checked_add(RasterLayer::TILE_BYTES as u64)
                .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "tile footprint overflow"))?;
            state.peak = state.peak.max(state.live);
        }
        Ok(Self {
            budget: Some(budget),
            scope,
        })
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        // The Vec has already been freed. Retire its attempt gauge before
        // returning shared capacity, and run callbacks outside the scope lock.
        self.scope
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .live -= RasterLayer::TILE_BYTES as u64;
        drop(self.budget.take());
    }
}

#[derive(Debug)]
pub(super) struct Tile {
    pub(super) pixels: Vec<u8>,
    _residency: Option<Allocation>,
}
impl Tile {
    /// Empty ownership-transfer sentinel; never exposed as a resident tile.
    pub(super) fn empty() -> Self {
        Self {
            pixels: Vec::new(),
            _residency: None,
        }
    }
    fn allocate(adm: &mut Admission) -> Result<(Vec<u8>, Allocation)> {
        adm.charge_tile()?;
        let lease = match &adm.reservation {
            Some(reservation) => reservation.reserve(RasterLayer::TILE_BYTES as u64),
            None => adm.residency.reserve(RasterLayer::TILE_BYTES as u64),
        }
        .map_err(|error| {
            let required = if let Some(scope) = &adm.allocation_scope {
                let state = scope.lock().unwrap_or_else(|error| error.into_inner());
                let Some(next) = state.live.checked_add(RasterLayer::TILE_BYTES as u64) else {
                    return Error::new(ErrorCode::ResourceLimit, "tile attempt footprint overflow");
                };
                next.max(state.peak)
            } else {
                RasterLayer::TILE_BYTES as u64
            };
            error
                .with_message("decoded tile residency limit exceeded")
                .with_allocation_requirement(AllocationClass::DecodedTiles, required)
        })?;
        let scope = adm
            .allocation_scope
            .get_or_insert_with(|| Arc::new(Mutex::new(Scope::default())))
            .clone();
        let lease = Allocation::new(lease, scope)?;
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
