//! PCND2 attachment framing. Exact ICC bytes are semantic metadata, not tiles.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::source_metadata::{self as attachment, SourceMetadata};

pub(super) fn write<W: Write>(
    source: &SourceMetadata,
    output: &mut Output<W>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    output.bytes(&source.wire_header())?;
    if let Some(profile) = source.profile() {
        for chunk in profile.chunks(65536) {
            check()?;
            output.bytes(chunk)?;
        }
    }
    check()
}
pub(super) fn read(
    input: &mut Input<'_>,
    limit: u64,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<SourceMetadata> {
    check()?;
    if limit < attachment::WIRE_HEADER_BYTES as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND source metadata allowance",
        ));
    }
    let header = input.take(attachment::WIRE_HEADER_BYTES)?;
    let profile_bytes = u64::from_be_bytes(header[112..120].try_into().unwrap());
    let extent = profile_bytes
        .checked_add(attachment::WIRE_HEADER_BYTES as u64)
        .filter(|n| *n <= limit.min(attachment::BYTES_MAX))
        .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "PCND source metadata extent"))?;
    let profile = input.take((extent - attachment::WIRE_HEADER_BYTES as u64) as usize)?;
    // Borrowed framing is checked before reserving/copying any profile bytes.
    SourceMetadata::from_wire(header, profile, limit, budget, check)
}
