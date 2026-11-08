//! Separate Source8 rooted route. Existing service/gateway bodies stay exact.
use super::*;
use crate::blend::rooted::{self as controls, RootFacts};
mod gateway;
#[derive(Clone, Copy, Debug)]
pub struct RootedCallQuote {
    pub call: CallQuote,
    /// Future actual facts construction and ordered helper passes, not spent now.
    pub traversal_work: u64,
    /// Actual execution witness, additional to existing call.context_bytes.
    pub traversal_control_bytes: u64,
}
/// Quote pays its own capture/proof/quote-loop/facts helper phases now.
/// Future actual validation/traversal/context/kernel costs are forecasts only.
pub fn rooted_call_quote_checked(
    doc: &Document,
    region: Rect,
    caller: Caller<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<RootedCallQuote> {
    let resolved = caller::Resolved::new(caller)?;
    let debit = |n| resolved.metadata_spend(n, spend);
    let keys = capture(doc, &resolved, check, &debit)?;
    let retained = if resolved.base.has_prepared_programs() {
        resolved.required_retained_bytes()?
    } else {
        debit(keys.quote_metadata()?)?;
        keys.quote()?.retained_bytes
    };
    let proof = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(limit)?;
    debit(proof)?; // context(doc)'s existing named proof allowance
    debit(controls::quote_loop_work(doc)?)?; // distinct quote(doc,c) root pass
    let quote = super::super::quote_document(doc, region)?;
    let facts = RootFacts::capture(doc, check, &debit)?;
    let fresh = !super::super::allowed(doc);
    check()?;
    Ok(RootedCallQuote {
        call: CallQuote {
            setup_work: if fresh { proof } else { 0 },
            context_bytes: if fresh { quote.control_bytes } else { 0 },
            required_retained_bytes: retained,
            opaque_samples: quote.opaque_samples,
            general_samples: quote.general_samples,
            opaque_work: quote
                .opaque_samples
                .checked_mul(super::super::OPAQUE_EXTENSION_WORK)
                .ok_or_else(limit)?,
            general_work: quote
                .general_samples
                .checked_mul(super::super::GENERAL_WORK)
                .ok_or_else(limit)?,
            call_metadata_work: keys.validation_work(&resolved.base)?,
        },
        traversal_work: facts.traversal_work()?,
        traversal_control_bytes: controls::control_bytes(),
    })
}
/// No global bypass: exact keys/minima/owners freshly resolve on every invocation.
pub fn render_rooted_call_checked(
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
    let witness_bytes = controls::control_bytes();
    let bytes = resolved
        .base
        .bytes_limit()
        .checked_sub(witness_bytes)
        .ok_or_else(limit)?;
    let mut witness_charge = resolved.base.scope().account();
    witness_charge.resize(witness_bytes)?;
    let result = {
        let facts = RootFacts::capture(doc, check, &|n| resolved.spend(n))?;
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
        effective.install(|| gateway::render(doc, region, &effective, &facts, check))
    }; // facts and effective owners end before their witness declaration
    drop(witness_charge);
    result
}
#[cfg(test)]
mod tests;
