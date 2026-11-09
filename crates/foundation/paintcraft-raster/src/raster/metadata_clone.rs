//! Pinned Rust1.99/hashbrown0.17.1 fresh-map layout, never arbitrary old capacity.
use super::*;
use std::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshMapLayout {
    pub bytes: u64,
    pub capacity: usize,
}
/// Conservative allocation-layout upper with16 control bytes (supported group
/// widths<=16). Payload/align are actual target types. This is not allocator RSS.
/// Only fresh, deletion-free maps can use this policy. Toolchain changes require
/// reviewing pinned capacity_to_buckets/TableLayout/RawTable::clone sources.
pub fn fresh_map_layout<K, V>(entries: usize) -> Result<FreshMapLayout> {
    if entries == 0 {
        return Ok(FreshMapLayout {
            bytes: 0,
            capacity: 0,
        });
    }
    let width = size_of::<(K, V)>();
    if width < 4 {
        return Err(bad("metadata map entry width outside pinned policy"));
    }
    let buckets = if entries < 4 {
        4
    } else if entries < 8 {
        8
    } else if entries < 15 {
        16
    } else {
        entries
            .checked_mul(8)
            .map(|n| n / 7)
            .and_then(usize::checked_next_power_of_two)
            .ok_or_else(limit)?
    };
    let alignment = align_of::<(K, V)>().max(16);
    let payload = buckets.checked_mul(width).ok_or_else(limit)?;
    let control = payload.checked_add(alignment - 1).ok_or_else(limit)? & !(alignment - 1);
    let bytes = control
        .checked_add(buckets)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(limit)?;
    let capacity = if buckets <= 8 {
        buckets - 1
    } else {
        buckets / 8 * 7
    };
    Ok(FreshMapLayout {
        bytes: bytes as u64,
        capacity,
    })
}
impl RasterLayer {
    /// New tile-index layout only. Tile allocations/backings remain shared.
    pub fn compact_metadata_clone_quote(&self) -> Result<FreshMapLayout> {
        fresh_map_layout::<TileCoord, Arc<Tile>>(self.tiles.len())
    }
    /// Caller provenance MUST be actual decoder->immutable owner or this compact
    /// constructor; capacity equality alone cannot prove freshness of arbitrary
    /// live HashMaps. No old map's hidden bucket count is inferred here.
    pub fn retained_decoded_metadata_quote_checked(
        &self,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<u64> {
        check()?;
        spend(1)?;
        let layout = self.compact_metadata_clone_quote()?;
        if self.tiles.capacity() != layout.capacity {
            return Err(bad("decoded tile map differs from pinned fresh layout"));
        }
        Ok(layout.bytes)
    }
    /// Caller MUST retain the precharged account through actual result drop.
    /// The Editor checked owner is the production consumer; no account escapes
    /// from this helper and no new realm/scope is constructed inside it.
    pub fn compact_metadata_clone_precharged_checked(
        &self,
        charge: &BudgetAccount,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let layout = self.compact_metadata_clone_quote()?;
        if charge.bytes() < layout.bytes {
            return Err(limit());
        }
        spend(layout.bytes)?; // fresh table/control initialization upper
        let mut tiles = TileTable::new();
        tiles.try_reserve(self.tiles.len()).map_err(|_| limit())?;
        if tiles.capacity() != layout.capacity {
            return Err(bad("fresh tile map differs from pinned layout"));
        }
        for (coord, tile) in &self.tiles {
            check()?;
            spend((size_of::<(TileCoord, Arc<Tile>)>() + size_of::<TileCoord>()) as u64)?;
            tiles.insert_reserved(*coord, Arc::clone(tile))?;
        }
        Ok(Self {
            width: self.width,
            height: self.height,
            default: self.default,
            tiles,
        })
    }
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "metadata clone declaration exhausted",
    )
}
fn bad(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deletion_history_is_compacted_while_versions_and_hidden_padding_are_shared() {
        let mut source = RasterLayer::new(1_048_576, 256, Rgba::TRANSPARENT);
        source
            .blend_pixel(
                0,
                0,
                Rgba([7, 8, 9, 0]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let tile = source.tiles.values().next().unwrap().clone();
        for tx in 1..4096 {
            source.tiles.try_reserve(1).unwrap();
            source
                .tiles
                .insert_reserved(TileCoord { tx, ty: 0 }, tile.clone())
                .unwrap();
        }
        let allocated_capacity = source.tiles.capacity();
        for tx in 1..4096 {
            source.tiles.remove(&TileCoord { tx, ty: 0 });
        }
        assert!(allocated_capacity > 4096);
        let quote = source.compact_metadata_clone_quote().unwrap();
        let realm = SharedBudget::new(quote.bytes);
        let mut account = realm.account(AllocationClass::PaintMetadata);
        account.resize(quote.bytes).unwrap();
        let clone = source
            .compact_metadata_clone_precharged_checked(&account, &|| Ok(()), &|_| Ok(()))
            .unwrap();
        assert_eq!(clone.tiles.capacity(), 3);
        assert_eq!(clone.get(0, 0), Rgba([7, 8, 9, 0]));
        assert!(Arc::ptr_eq(
            source.tiles.values().next().unwrap(),
            clone.tiles.values().next().unwrap()
        ));
        assert_eq!(source.tiles.len(), 1);
        assert_eq!(source.width(), clone.width());
        drop(clone);
        drop(account);
        assert_eq!(realm.usage().reserved, 0);
    }
    #[test]
    fn precharge_denial_and_cancel_never_clone_tile_payload_or_modify_source() {
        let mut source = RasterLayer::new(1, 1, Rgba::TRANSPARENT);
        source
            .blend_pixel(
                0,
                0,
                Rgba([1, 2, 3, 255]),
                BlendMode::Replace,
                &mut Admission::new(65536),
            )
            .unwrap();
        let quote = source.compact_metadata_clone_quote().unwrap();
        let budget = SharedBudget::new(quote.bytes);
        let mut account = budget.account(AllocationClass::PaintMetadata);
        account.resize(quote.bytes - 1).unwrap();
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            source
                .compact_metadata_clone_precharged_checked(
                    &account,
                    &|| {
                        calls.set(calls.get() + 1);
                        Ok(())
                    },
                    &|_| Ok(())
                )
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(calls.get(), 1);
        account.resize(quote.bytes).unwrap();
        assert_eq!(
            source
                .compact_metadata_clone_precharged_checked(
                    &account,
                    &|| Err(Error::new(ErrorCode::OperationCancelled, "stop")),
                    &|_| Ok(())
                )
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(source.get(0, 0), Rgba([1, 2, 3, 255]));
    }
}

mod retained;
