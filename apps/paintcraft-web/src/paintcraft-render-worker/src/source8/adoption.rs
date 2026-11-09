//! One complete conservative supplement before the unchanged retained adopter.
use super::{
    assembly_ledger::{self as work, AdoptionFacts},
    entry::EntryAuth8,
    metadata::FreshLayerMap8,
};
use paintcraft_core::types::{Result, resources::BudgetScope};
use paintcraft_document::Document;
use paintcraft_formats::pcnd::layer_blend_appearance::{AdoptedCoverage8, SourceCoverage8};
pub(super) fn finish(
    source: SourceCoverage8,
    document: &Document,
    fresh: &FreshLayerMap8,
    facts: &AdoptionFacts,
    scope: &BudgetScope,
    auth: &EntryAuth8<'_>,
) -> Result<AdoptedCoverage8> {
    auth.spend(64)?;
    if fresh.written != fresh.entries
        || document.roots.len() != fresh.entries
        || document.layers.len() != fresh.entries
        || document.layers.capacity() != fresh.capacity
        || fresh.buckets as u64 != work::buckets(fresh.entries as u64)?
        || (fresh.entries != 0 && !fresh.retirement_paid)
    {
        return Err(work::invalid());
    }
    // The witness was created at the only fresh allocation; this comparison alone
    // cannot construct it for an arbitrary document.
    auth.spend(work::adoption(
        fresh.entries as u64,
        fresh.buckets as u64,
        facts,
    )?)?;
    let raw = || auth.check();
    let spend = |n| auth.spend(n);
    source.finish_adoption_checked(document, scope, &raw, &spend)
}
