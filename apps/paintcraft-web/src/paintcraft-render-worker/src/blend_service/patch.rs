//! Compiled discovery uses only fixed metadata, never a realm/source/counter.
use super::*;
/// Scalar READY fact sequence defined by exported-api-before-wrapper.json.
pub fn runtime_fact(index: u32) -> Result<u64> {
    let i = index as usize;
    if i < controls::VERSIONS.len() {
        return Ok(u64::from(controls::VERSIONS[i]));
    }
    controls::LIMITS
        .get(i - controls::VERSIONS.len())
        .copied()
        .ok_or_else(bad)
}
/// Three exact defining control components; original H is their checked sum.
pub fn control_fact(index: u32) -> Result<u64> {
    LayerBlendService::control_quote()?
        .get(index as usize)
        .copied()
        .ok_or_else(bad)
}
/// Exact compiled profile binding, not source/admission authority.
pub fn profile_word(index: u32) -> Result<u32> {
    if index >= 8 {
        return Err(bad());
    }
    let p = LayerBlendService::compiled_profile()?;
    let i = index as usize * 4;
    Ok(u32::from_le_bytes(
        p[i..i + 4].try_into().expect("fixed profile index"),
    ))
}
