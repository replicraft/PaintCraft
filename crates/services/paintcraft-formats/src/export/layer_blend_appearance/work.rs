//! Pure conservative byte/serialization policy; no allocation/account/debit.
use super::*;
const ADMISSION_BYTES: u64 =
    b"paintcraft.worker-admission/v8\0".len() as u64 + 2 + 4 * 8 + 2 + 2 + 2 * 8;
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn txt(s: &str) -> u64 {
    2 + s.len() as u64
}
fn registration_bytes(required: &Requirements8) -> Result<u64> {
    let m = required.mask_appearance();
    m.validate()?;
    let b = required.blend_appearance();
    b.validate()?;
    let mask = txt(m.feature_id) + txt(m.resource_contract_id) + 6 + txt(m.workload_class_id) + 2;
    let blend =
        txt(b.feature_id) + txt(b.resource_contract_id) + 6 + 24 + txt(b.workload_class_id) + 2;
    add(mask, blend)
}
/// Per ONE capture/from_frames + bounded metadata publication. Includes the
/// required/resolution/admission/capability hash inputs and a conservative JSON
/// envelope containing both registrations, full pairs and resolved subset rows.
/// This is structural byte work, not instructions/RSS. Callers precharge BEFORE
/// hashing/serialization and pass raw cancellation checks, not a second debit.
pub fn framing_work_quote(required: &Requirements8) -> Result<u64> {
    let mut numerical = b"paintcraft.worker-required/v1\0".len() as u64
        + 2
        + b"paintcraft.worker-resolution/v1\0".len() as u64
        + 4;
    // 4096 covers all fixed Fact/Union/own fields, decimal u64/hex UUID/SHA
    // lengths, both constant registrations and JSON punctuation. Each dynamic
    // numerical row additionally receives1024 +its ID byte extents; actual
    // endpoint control-size limits remain authoritative, never increased here.
    let mut serialization = 4096u64;
    for (p, c) in required.requirements().pairs() {
        let entry = super::super::prepared_capabilities::resolve_for(
            RegistryContract::LayerAppearance4,
            p,
            c,
        )?;
        let (resource, _) = entry.resource_contract();
        let (class, _) = entry.workload_class();
        numerical = add(numerical, 2 * (txt(p) + txt(c)) + txt(resource) + 4)?;
        serialization = add(
            serialization,
            1024 + 2 * txt(p) + 2 * txt(c) + txt(resource) + txt(class),
        )?;
    }
    // ABI text bound uses BOTH supported actual identifiers, not current target.
    let abi = txt(CoverageAbi::Native64V1.id()).max(txt(CoverageAbi::Wasm32V1.id()));
    let prefix = add(
        2 + 2 + 4 + 32 + 32 + 32 + abi,
        registration_bytes(required)?,
    )?;
    let fact = add(
        b"paintcraft.worker-capabilities/v8\0".len() as u64 + 8 + 24 + 4 * 32,
        prefix,
    )?;
    let union = add(
        b"paintcraft.worker-capability-union/v8\0".len() as u64 + 64,
        prefix,
    )?;
    let extent = add(
        add(numerical, ADMISSION_BYTES)?,
        add(fact.max(union), serialization)?,
    )?;
    extent
        .checked_mul(16)
        .and_then(|v| v.checked_add(4096))
        .and_then(|v| v.checked_add((required.requirements().len() as u64).checked_mul(512)?))
        .ok_or_else(limit)
}
/// Separate cached admission SHA only; capture quote already includes it.
pub const fn admission_work_quote() -> u64 {
    4096 + 16 * ADMISSION_BYTES
}
