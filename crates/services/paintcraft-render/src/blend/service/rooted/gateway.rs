//! Only called after complete-key validation on the actual owner(s).
use super::*;
use crate::blend::{CPU, Restore, allowed, context};
fn body(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    seed: Option<&crate::blend::seed::Seed<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    // Same ordered production sequence as render_region_cancellable's body.
    check()?;
    let bytes = crate::region_bytes(doc, region)?;
    crate::spatial::preflight_rooted(facts, region, &check, &|n| resolved.spend(n))?;
    match seed {
        Some(seed) => {
            seed.require_matches(doc, region)?;
            resolved.spend(facts.pass_work(1)?)?;
            crate::blend::seed::fresh_output(seed, bytes, check)
        }
        None => {
            let mut out = crate::output_buffer(bytes)?;
            resolved.spend(facts.pass_work(1)?)?;
            crate::compose_roots_into(doc, region, 0..doc.roots.len(), &mut out, &check)?;
            Ok(out)
        }
    }
}
fn render_shared(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    seed: Option<&crate::blend::seed::Seed<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let retained = resolved.required_retained_bytes()?;
    if allowed(doc) {
        let c = CPU.with(std::cell::Cell::get).ok_or_else(missing)?;
        crate::validate_region(doc, region)?;
        resolved.spend(facts.pass_work(2)?)?;
        preflight(doc, region, c, retained, seed, check)?;
        return body(doc, region, resolved, facts, seed, check);
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
        preflight(doc, region, c, retained, seed, check)?;
        // No prepared/TLS bypass marker is installed. Public callback reentry
        // still executes its original full gateway/preflight.
        body(doc, region, resolved, facts, seed, check)
    })
}

fn preflight(
    doc: &Document,
    region: Rect,
    c: crate::blend::Context,
    retained: u64,
    seed: Option<&crate::blend::seed::Seed<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    match seed {
        None => crate::blend::quote::preflight_prepared(doc, region, c, retained, check),
        Some(seed) => {
            let q = crate::blend::quote::seeded_quote(doc, region, c, seed)?;
            crate::blend::quote::preflight_for_quote(doc, region, retained, q, check)
        }
    }
}
pub(super) fn render(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    render_shared(doc, region, resolved, facts, None, check)
}
pub(in crate::blend::service) fn render_seeded(
    doc: &Document,
    region: Rect,
    resolved: &caller::Resolved,
    facts: &RootFacts<'_>,
    seed: &crate::blend::seed::Seed<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    render_shared(doc, region, resolved, facts, Some(seed), check)
}
