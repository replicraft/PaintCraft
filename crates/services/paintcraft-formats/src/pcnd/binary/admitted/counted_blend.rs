//! Source8 complete counted/fixed passes through the actual rooted blend writer.
//! Normal/Source7 counted helpers and writer bodies remain unchanged.
use super::write_portable_blend_snapshot_admitted_checked as write_rooted;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_document::Document;
mod counting;
mod fixed;
mod forecast;
mod plan_bounds;
pub use fixed::CountedBlendSnapshotBytes;
pub use forecast::BlendSourceDemand;
pub use plan_bounds::BlendPlanRecordUpperBounds;
const SOURCE_MAX: u64 = 64 << 20;
/// Target-sized scalar controls, never a source quote or admission token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendCountedControlBytes {
    pub fact: u64,
    pub counting_sink: u64,
    pub bytes_owner: u64,
    pub fixed_sink: u64,
}
/// Successful complete count tied to the original immutable source and callbacks.
/// Callbacks may use interior state and may refuse subsequent operations.
/// No callback purity or equivalence claim follows from these borrows.
///
/// The original document remains immutably borrowed through each real pass:
/// ```compile_fail
/// use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
/// use paintcraft_formats::pcnd::count_portable_blend_snapshot_rooted_checked;
/// let mut doc = paintcraft_document::import_image(1, 1, &[0, 0, 0, 255], "test").unwrap();
/// let budget = SharedBudget::new(4 << 20);
/// let scope = budget.scope(AllocationClass::PaintMetadata);
/// let check = || Ok(());
/// let spend = |_| Ok(());
/// let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
/// doc.title = Some("different".into());
/// let _ = fact.encode();
/// ```
/// A real pass accepts no replacement scope or callbacks:
/// ```compile_fail
/// use paintcraft_formats::pcnd::CountedBlendSnapshot;
/// use paintcraft_core::types::resources::BudgetScope;
/// fn replace(fact: &CountedBlendSnapshot<'_>, other: &BudgetScope) {
///     let _ = fact.encode(other);
/// }
/// ```
pub struct CountedBlendSnapshot<'a> {
    doc: &'a Document,
    scope: &'a BudgetScope,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    bytes: u64,
    _control: BudgetAccount,
}
impl std::fmt::Debug for CountedBlendSnapshot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountedBlendSnapshot")
            .field("document_id", &self.document_id())
            .field("state_id", &self.state_id())
            .field("wire_len", &self.wire_len())
            .finish_non_exhaustive()
    }
}
impl CountedBlendSnapshot<'_> {
    pub fn wire_len(&self) -> u64 {
        self.bytes
    }
    pub fn document_id(&self) -> uuid::Uuid {
        self.doc.document_id
    }
    pub fn state_id(&self) -> uuid::Uuid {
        self.doc.state_id
    }
    /// Run the complete writer again, paying initialization and writer work.
    /// No arguments can replace the original source, scope or callbacks.
    pub fn encode(&self) -> Result<CountedBlendSnapshotBytes> {
        fixed::encode(self)
    }
}
pub fn blend_counted_snapshot_control_bytes() -> BlendCountedControlBytes {
    BlendCountedControlBytes {
        fact: size_of::<CountedBlendSnapshot<'static>>() as u64,
        counting_sink: counting::CountingSink::control_bytes(),
        bytes_owner: size_of::<CountedBlendSnapshotBytes>() as u64,
        fixed_sink: fixed::control_bytes(),
    }
}
fn limit(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn mismatch() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "counted snapshot length mismatch",
    )
}
/// Count the complete portable container, including its final SHA trailer.
/// Both passes use only this scope and these callbacks. No output Vec is retained.
pub fn count_portable_blend_snapshot_rooted_checked<'a>(
    doc: &'a Document,
    scope: &'a BudgetScope,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
) -> Result<CountedBlendSnapshot<'a>> {
    spend(64)?;
    let sizes = blend_counted_snapshot_control_bytes();
    spend(2 * sizes.fact + 2 * sizes.counting_sink + 4 * 128 + 64)?;
    validate_layouts()?;
    step(check, spend)?;
    let mut control = scope.account();
    control.resize(blend_counted_snapshot_control_bytes().fact)?;
    let mut sink = counting::CountingSink::new(scope)?;
    let result = write_checked(doc, &mut sink, scope, check, spend);
    if sink.stop().is_some() {
        return Err(limit("counted snapshot exceeds source limit"));
    }
    result?;
    let bytes = sink.bytes();
    drop(sink);
    Ok(CountedBlendSnapshot {
        doc,
        scope,
        check,
        spend,
        bytes,
        _control: control,
    })
}
// These are finite source-action and complete inline-result ceilings, not CPU
// instruction/RSS/allocator claims. Original arbitrary callback Error is moved.
fn validate_layouts() -> Result<()> {
    let s = blend_counted_snapshot_control_bytes();
    let expected = match size_of::<usize>() {
        8 => [96, 56, 96, 64],
        4 => [56, 40, 56, 40],
        _ => return Err(limit("Source8 counted target layout")),
    };
    if [s.fact, s.counting_sink, s.bytes_owner, s.fixed_sink] != expected
        || size_of::<Result<CountedBlendSnapshot<'static>>>() > 128
        || size_of::<Result<CountedBlendSnapshotBytes>>() > 128
        || size_of::<Result<counting::CountingSink>>() > 128
        || !fixed::return_layout_supported()
    {
        return Err(limit("Source8 counted target layout"));
    }
    Ok(())
}
fn step(check: &dyn Fn() -> Result<()>, spend: &dyn Fn(u64) -> Result<()>) -> Result<()> {
    spend(512)?;
    check()
}
fn write_checked(
    doc: &Document,
    sink: impl std::io::Write,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let checked = || step(check, spend);
    // Every Metered writer call prepays its original debit before this sink.
    // Additional32 covers scalar/IO bookkeeping and128 the NEW adapter result;
    // other existing writer events conservatively pay the same addition.
    let priced = |n: u64| {
        let Some(units) = n.checked_add(160) else {
            return Err(limit("Source8 counted work overflow"));
        };
        spend(units)
    };
    write_rooted(doc, sink, scope, &checked, &priced)
}
#[cfg(test)]
mod tests;
