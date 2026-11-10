//! Fresh-output variant; old quote/counter contracts and action order remain.
use super::{
    Caller, Programs, blend_programs, caller,
    rooted::{self, RootedCallQuote},
};
use crate::blend::{
    limit,
    rooted::{self as controls, RootFacts},
    seed,
};
use paintcraft_core::types::{Error, Rect, Result};
use paintcraft_document::Document;
use std::mem::size_of;
pub(super) fn validate_controls() -> Result<()> {
    let native = usize::BITS == 64;
    if size_of::<seed::Seed<'static>>() != if native { 48 } else { 40 }
        || size_of::<caller::Resolved>() > 320
        || size_of::<Result<seed::Seed<'static>>>() > 112
        || size_of::<Result<crate::blend::quote::Quote>>() > 112
        || size_of::<Result<caller::Resolved>>() > 320
        || size_of::<Result<Vec<u8>>>() > 112
        || size_of::<Result<()>>() > 112
        || size_of::<Error>() > 96
        || size_of::<RootedCallQuote>() != 80
        || size_of::<Result<RootedCallQuote>>() > 112
    {
        return Err(limit());
    }
    // Fixed bootstrap exists only before this new W account: no fresh heap,
    // factory or counter. Logical interception of caller-stack is not claimed.
    let bootstrap = size_of::<caller::Resolved>()
        + 320
        + size_of::<paintcraft_core::types::resources::BudgetAccount>()
        + size_of::<&Document>();
    if bootstrap > 768 {
        return Err(limit());
    }
    let named = size_of::<seed::Seed<'static>>()
        + 320
        + 224
        + size_of::<crate::blend::quote::Quote>()
        + 112
        + 320
        + 112
        + 4 * size_of::<&Document>()
        + size_of::<[u64; 8]>()
        + 96
        + size_of::<RootedCallQuote>()
        + 112
        + 2 * size_of::<paintcraft_core::types::resources::BudgetAccount>();
    if named != if native { 1632 } else { 1576 } || named as u64 > seed::CONTROL {
        return Err(limit());
    }
    Ok(())
}

/// Same existing quote ABI, but only ACTUAL general blend calls are counted.
/// Fresh first-source copy uses the containing Source8 Normal64 skeleton;
/// like existing Normal work, this copy is not an extra General kernel grant.
///
/// # Errors
/// Refuses mismatched caller/key/minima, unsupported controls, exhausted
/// original byte/work capacity, cancellation or malformed geometry.
pub fn seeded_rooted_call_quote_checked(
    doc: &Document,
    region: Rect,
    caller: Caller<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<RootedCallQuote> {
    let resolved = caller::Resolved::new(caller)?;
    let debit = |n| resolved.metadata_spend(n, spend);
    debit(64 + 2 * seed::CONTROL + 128)?;
    validate_controls()?;
    let mut charge = resolved.base.scope().account();
    charge.resize(seed::CONTROL)?;
    let mut q = rooted::rooted_call_quote_checked(doc, region, caller, check, spend)?;
    let proof = seed::Seed::capture(doc, region, check, &debit)?;
    proof.require_matches(doc, region)?;
    q.call.general_samples = q
        .call
        .general_samples
        .checked_sub(proof.samples()?)
        .ok_or_else(limit)?;
    q.call.general_work = q
        .call
        .general_samples
        .checked_mul(crate::blend::GENERAL_WORK)
        .ok_or_else(limit)?;
    q.traversal_work = q
        .traversal_work
        .checked_add(seed::future_work(doc)?)
        .ok_or_else(limit)?;
    q.traversal_control_bytes = q
        .traversal_control_bytes
        .checked_add(seed::CONTROL)
        .ok_or_else(limit)?;
    Ok(q)
}
/// Actual fresh-zero output only. No cache prefix is admitted by this route.
/// Containing Source8 must fund its Normal64 copy skeleton separately; this
/// standalone API preserves the inherited Normal bookkeeping boundary.
///
/// # Errors
/// Refuses stale keys/proofs, invalid ROI, original exhaustion or cancellation.
/// No partial output is returned.
pub fn render_seeded_rooted_call_checked(
    doc: &Document,
    region: Rect,
    programs: &Programs,
    check: &dyn Fn() -> Result<()>,
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
    resolved.spend(64 + 2 * seed::CONTROL + 128)?;
    validate_controls()?;
    let witness = controls::control_bytes()
        .checked_add(seed::CONTROL)
        .ok_or_else(limit)?;
    let bytes = resolved
        .base
        .bytes_limit()
        .checked_sub(witness)
        .ok_or_else(limit)?;
    let mut charge = resolved.base.scope().account();
    charge.resize(witness)?;
    let result = {
        let facts = RootFacts::capture(doc, check, &|n| resolved.spend(n))?;
        let proof = seed::Seed::capture(doc, region, check, &|n| resolved.spend(n))?;
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
            rooted::gateway::render_seeded(doc, region, &effective, &facts, &proof, check)
        })
    };
    drop(charge);
    result
}
