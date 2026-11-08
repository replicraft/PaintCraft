//! Only called after complete-key validation on the actual owner(s).
use super::*;
use crate::blend::{CPU, Restore, allowed, context};
fn body(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    // Same ordered production sequence as render_region_cancellable's body.
    check()?;
    let bytes = crate::region_bytes(doc, region)?;
    crate::spatial::preflight_rooted(facts, region, &check, &|n| resolved.spend(n))?;
    let mut out = crate::output_buffer(bytes)?;
    resolved.spend(facts.pass_work(1)?)?;
    crate::compose_roots_into(doc, region, 0..doc.roots.len(), &mut out, &check)?;
    Ok(out)
}
pub(super) fn render(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let retained = resolved.required_retained_bytes()?;
    if allowed(doc) {
        let c = CPU.with(std::cell::Cell::get).ok_or_else(missing)?;
        crate::validate_region(doc, region)?;
        resolved.spend(facts.pass_work(2)?)?;
        crate::blend::quote::preflight_prepared(doc, region, c, retained, check)?;
        return body(doc, region, resolved, facts, check);
    }
    let controls = crate::blend::control_quote();
    if controls > resolved.base.bytes_limit() {
        return Err(limit());
    }
    let mut charge = resolved.base.scope().account();
    charge.resize(controls)?;
    let setup = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(limit)?;
    resolved.spend(setup)?;
    let c = context(doc)?;
    let child = resolved
        .base
        .clone()
        .with_byte_limit(resolved.base.bytes_limit() - controls);
    crate::spatial::with_workspace(child, || {
        let prior = CPU.with(|active| active.replace(Some(c)));
        let _restore = Restore(prior);
        crate::validate_region(doc, region)?;
        resolved.spend(facts.pass_work(2)?)?;
        crate::blend::quote::preflight_prepared(doc, region, c, retained, check)?;
        // No prepared/TLS bypass marker is installed. Public callback reentry
        // still executes its original full gateway/preflight.
        body(doc, region, resolved, facts, check)
    })
}
