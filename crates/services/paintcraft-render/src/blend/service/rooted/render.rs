//! One runtime-selected owner installation; old debit and callback order remain.
use super::gateway;
use crate::blend::{
    limit,
    rooted::{self as controls, RootFacts},
    seed,
    service::{Programs, blend_programs, caller, seeded},
};
use paintcraft_core::types::{Rect, Result};
use paintcraft_document::Document;

// Keep one install closure/with_noise_workspace monomorph for both wrappers.
#[inline(never)]
pub(in crate::blend::service) fn run(
    doc: &Document,
    region: Rect,
    programs: &Programs,
    check: &dyn Fn() -> Result<()>,
    fresh_seed: bool,
) -> Result<Vec<u8>> {
    let resolved = caller::Resolved::new(programs.caller())?;
    blend_programs::validate(
        doc,
        &resolved.base,
        resolved.ambient.as_ref(),
        resolved.noise.is_some(),
        check,
        &|n| resolved.spend(n),
    )?;
    let witness_bytes = if fresh_seed {
        resolved.spend(64 + 2 * seed::CONTROL + 128)?;
        seeded::validate_controls()?;
        controls::control_bytes()
            .checked_add(seed::CONTROL)
            .ok_or_else(limit)?
    } else {
        controls::control_bytes()
    };
    let bytes = resolved
        .base
        .bytes_limit()
        .checked_sub(witness_bytes)
        .ok_or_else(limit)?;
    let mut witness_charge = resolved.base.scope().account();
    witness_charge.resize(witness_bytes)?;
    let result = {
        let facts = RootFacts::capture(doc, check, &|n| resolved.spend(n))?;
        // None creates no proof payload, capture, walk, debit or account. The
        // legacy route keeps its original witness bytes/minimum unchanged.
        let proof = if fresh_seed {
            Some(seed::Seed::capture(doc, region, check, &|n| {
                resolved.spend(n)
            })?)
        } else {
            None
        };
        let base = resolved.base.clone().with_byte_limit(bytes);
        let noise = resolved
            .noise
            .as_ref()
            .map(|n| n.for_workspace(&base))
            .transpose()?;
        let effective = caller::Resolved {
            base,
            noise,
            ambient: resolved.ambient.clone(),
        };
        effective.install(|| {
            gateway::render_shared(doc, region, &effective, &facts, proof.as_ref(), check)
        })
    };
    drop(witness_charge);
    result
}
