//! Pure rooted forecast: actual Base AND Noise ambient remain forbidden.
use super::*;
use crate::blend::rooted::{self as controls, RootFacts};
#[derive(Clone, Copy, Debug)]
pub struct UnboundRootedCallForecast {
    pub call: UnboundCallForecast,
    pub traversal_work: u64,
    pub traversal_control_bytes: u64,
}
pub fn quote_unbound_rooted_call_checked(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
    spend_metadata: &dyn Fn(u64) -> Result<()>,
) -> Result<UnboundRootedCallForecast> {
    absent()?;
    let check = || {
        absent()?;
        check()?;
        absent()
    };
    let spend = |n| {
        absent()?;
        spend_metadata(n)?;
        absent()
    };
    let (keys, requires_noise) = Keys::capture_unbound(doc, &check, &spend)?;
    spend(keys.quote_metadata()?)?;
    let _ = keys.quote()?;
    let setup_work = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(super::super::limit)?;
    spend(setup_work)?;
    spend(controls::quote_loop_work(doc)?)?;
    let quote = super::super::quote_document(doc, region)?;
    let facts = RootFacts::capture(doc, &check, &spend)?;
    check()?;
    Ok(UnboundRootedCallForecast {
        call: UnboundCallForecast {
            context_bytes: quote.control_bytes,
            setup_work,
            opaque_samples: quote.opaque_samples,
            general_samples: quote.general_samples,
            opaque_work: quote
                .opaque_samples
                .checked_mul(super::super::OPAQUE_EXTENSION_WORK)
                .ok_or_else(super::super::limit)?,
            general_work: quote
                .general_samples
                .checked_mul(super::super::GENERAL_WORK)
                .ok_or_else(super::super::limit)?,
            prepared_validation_work: keys.unbound_validation_work()?,
            requires_noise,
        },
        traversal_work: facts.traversal_work()?,
        traversal_control_bytes: controls::control_bytes(),
    })
}
