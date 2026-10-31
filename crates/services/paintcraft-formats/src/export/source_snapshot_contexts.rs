//! Actual generic7 snapshot facts, authenticated before any decoder adoption.
use paintcraft_core::types::{Error, ErrorCode, Result};

/// Producer context from the same actual immutable PCND snapshot. The header
/// binds these fields; no context or hash needs a second source traversal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSnapshotContexts {
    pub region: [u8; 64],
    pub programs: [u8; 128],
    pub source_sha256: [u8; 32],
}

pub(crate) fn snapshot_resident_bytes(
    source: &[u8],
    resident_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<(u64, [u8; 32])> {
    let (verified, source_sha256) =
        crate::pcnd::CheckedContainer::verify_with_source_hash(source, check)?;
    let records = verified.snapshot_tile_records_checked(check)?;
    // Count ALL serialized records, including a valid explicit default-valued
    // raw tile. The adopting reader retains it; projection from pixels would
    // underprice its actual storage. No tile is inflated during this scan.
    let resident = records
        .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
        .filter(|bytes| *bytes <= resident_limit)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Generic snapshot resident allowance",
            )
        })?;
    Ok((resident, source_sha256))
}
