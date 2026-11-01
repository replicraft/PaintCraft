//! Foundational Export8 facts only: no codec/header activation or execution grant.
use super::*;
use super::{
    noise_workload::Workload,
    prepared_capabilities::{RegistryContract, Requirements},
};
use paintcraft_core::types::filters::Descriptor;
use paintcraft_render::noise_work::{self, NoiseAdmission, WorkComponents};
use sha2::{Digest, Sha256};
mod program;
mod region;
pub use program::SourceNoiseProgramContext;
pub use region::SourceNoiseRegionContext;
const REGION_BYTES: usize = 128;
const PROGRAM_BYTES: usize = 256;
const STORED_MAX: u32 = 8 * 256;
const PAYLOAD_MAX: u64 = 192 << 20;
fn bad() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Scoped export context framing/facts",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Scoped export context exceeds profile",
    )
}
/// Query-only same-target sizes; no scope/counter/allocation is constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseContextControlQuote {
    pub region_value_bytes: u64,
    pub program_value_bytes: u64,
    pub wire_copy_bytes: u64,
    pub counter_bytes: u64,
    pub handle_bytes: u64,
}
impl NoiseContextControlQuote {
    pub fn capture() -> Result<Self> {
        let control = NoiseAdmission::control_quote()?;
        Ok(Self {
            region_value_bytes: std::mem::size_of::<SourceNoiseRegionContext>() as u64,
            program_value_bytes: std::mem::size_of::<SourceNoiseProgramContext>() as u64,
            wire_copy_bytes: 2 * (REGION_BYTES + PROGRAM_BYTES) as u64,
            counter_bytes: control.counter_bytes,
            handle_bytes: control.handle_bytes,
        })
    }
    /// Two decoded/captured pairs, request/echo wire bytes and one retained handle.
    /// Counter allocation is excluded here: it is live inside workspace peaks.
    pub fn metadata_bytes(self) -> Result<u64> {
        self.region_value_bytes
            .checked_add(self.program_value_bytes)
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(self.wire_copy_bytes))
            .and_then(|n| n.checked_add(self.handle_bytes))
            .ok_or_else(limit)
    }
}
fn header(bytes: &[u8], size: usize) -> Result<()> {
    if bytes.len() != size
        || bytes[..2] != 8u16.to_be_bytes()
        || bytes[2..4] != 1u16.to_be_bytes()
        || bytes[4..6] != 6u16.to_be_bytes()
        || bytes[6..8] != 2u16.to_be_bytes()
    {
        return Err(bad());
    }
    Ok(())
}
fn write_header(bytes: &mut [u8]) {
    for (at, value) in [(0, 8u16), (2, 1), (4, 6), (6, 2)] {
        bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
    }
}
fn n32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn n64(bytes: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap())
}
/// Same additive v2 metadata grammar as Source6. These bytes grant no work.
fn capability(
    required: &Requirements,
    workload: Workload,
    check: &dyn Fn() -> Result<()>,
) -> Result<[u8; 32]> {
    required.validate_contract(RegistryContract::ScopedNoise2, check)?;
    let (r, s) = required.digests(check)?;
    let count = u16::try_from(required.len()).map_err(|_| limit())?;
    let mut h = Sha256::new();
    h.update(b"paintcraft.worker-capabilities/v2\0");
    h.update(2u16.to_be_bytes());
    h.update(r);
    h.update(s);
    h.update(workload.digest()?);
    h.update(count.to_be_bytes());
    for (profile, codec) in required.pairs() {
        check()?;
        let entry =
            prepared_capabilities::resolve_for(RegistryContract::ScopedNoise2, profile, codec)?;
        let (class, rev) = entry.workload_class();
        for id in [profile, codec, class] {
            h.update(u16::try_from(id.len()).map_err(|_| limit())?.to_be_bytes());
            h.update(id.as_bytes());
        }
        h.update(rev.to_be_bytes());
    }
    Ok(h.finalize().into())
}
#[cfg(test)]
mod tests;
