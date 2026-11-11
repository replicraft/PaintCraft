//! PCND9-only ordered style intentions. Old1..8 have no bytes in this slot.
use super::*;
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode};
use paintcraft_document::styles::STYLES_MAX;
use paintcraft_document::{StyleDescriptor, StyleInstance, StyleStack};
use std::mem::size_of;
const STYLE_RECORD_BYTES: usize = 23;

pub(super) fn metadata_bytes(stack: &StyleStack) -> Result<u64> {
    if stack.len() > STYLES_MAX {
        return Err(limit());
    }
    Ok(2 + STYLE_RECORD_BYTES as u64 * stack.len() as u64)
}

pub(super) fn write(
    stack: &StyleStack,
    output: &mut Output<impl Write>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if stack.len() > STYLES_MAX {
        return Err(limit());
    }
    output.bytes(&(stack.len() as u16).to_be_bytes())?;
    for style in stack.iter() {
        check()?;
        if style.id().is_nil() {
            return Err(codec("nil PCND9 style ID"));
        }
        output.bytes(style.id().as_bytes())?;
        match style.descriptor() {
            StyleDescriptor::ColorOverlayRgbV1(policy) => {
                output.bytes(&[u8::from(style.enabled()), 1])?;
                output.bytes(&policy.semantic_bytes())?;
            }
        }
    }
    Ok(())
}
/// Ordinary native reader's bounded metadata path, not a Worker9 admission.
/// Unknown tags/modes and disabled unknown styles still refuse. Retained vector
/// storage is bounded by actual target StyleInstance size and existing metadata
/// allowance before reservation; no replacement budget is created here.
/// Actual vector payload may not exceed its23-per-entry wire contribution,
/// so the aggregate8MiB wire ceiling also bounds all retained style vectors.
pub(super) fn read(
    input: &mut Input<'_>,
    allowance: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<StyleStack> {
    let count = u16::from_be_bytes(input.take(2)?.try_into().unwrap()) as usize;
    if count > STYLES_MAX {
        return Err(limit());
    }
    let wire = count
        .checked_mul(STYLE_RECORD_BYTES)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(limit)?;
    let payload = count
        .checked_mul(size_of::<StyleInstance>())
        .ok_or_else(limit)?;
    if wire > allowance || payload > count * STYLE_RECORD_BYTES {
        return Err(limit());
    }
    if count == 0 {
        return Ok(StyleStack::empty());
    }
    let mut instances = Vec::new();
    instances.try_reserve_exact(count).map_err(|_| limit())?;
    if instances.capacity() > STYLES_MAX
        || instances
            .capacity()
            .checked_mul(size_of::<StyleInstance>())
            .is_none_or(|n| n > count * STYLE_RECORD_BYTES)
    {
        return Err(limit());
    }
    for _ in 0..count {
        check()?;
        let id = input.id()?;
        if id.is_nil() || instances.iter().any(|s: &StyleInstance| s.id() == id) {
            return Err(codec("nil or duplicate PCND9 style ID"));
        }
        let enabled = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(codec("invalid PCND9 style enabled flag")),
        };
        if input.byte()? != 1 {
            return Err(unsupported("unknown PCND9 required style tag"));
        }
        let p = input.take(5)?;
        let mode = EffectBlendMode::from_code(p[4])
            .map_err(|_| codec("invalid PCND9 effect blend mode"))?;
        let descriptor = StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            [p[0], p[1], p[2]],
            p[3],
            mode,
        ));
        instances.push(StyleInstance::new(id, enabled, descriptor)?);
    }
    StyleStack::new_checked(instances, check)
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "PCND9 style metadata allowance")
}

#[cfg(test)]
#[path = "styles/tests.rs"]
mod tests;
