//! Complete count and fixed-output passes on the same borrowed rooted writer lane.
use super::write_portable_normal_snapshot_rooted_admitted_checked as write_rooted;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_document::Document;
mod counting;
mod fixed;
pub use fixed::CountedSnapshotBytes;
const SOURCE_MAX: u64 = 64 << 20;
/// Target-sized scalar controls, never a source quote or admission token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalCountedControlBytes {
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
/// use paintcraft_formats::pcnd::count_portable_normal_snapshot_rooted_checked;
/// let mut doc = paintcraft_document::import_image(1, 1, &[0, 0, 0, 255], "test").unwrap();
/// let budget = SharedBudget::new(4 << 20);
/// let scope = budget.scope(AllocationClass::PaintMetadata);
/// let check = || Ok(());
/// let spend = |_| Ok(());
/// let fact = count_portable_normal_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
/// doc.title = Some("different".into());
/// let _ = fact.encode();
/// ```
/// A real pass accepts no replacement scope or callbacks:
/// ```compile_fail
/// use paintcraft_formats::pcnd::CountedNormalSnapshot;
/// use paintcraft_core::types::resources::BudgetScope;
/// fn replace(fact: &CountedNormalSnapshot<'_>, other: &BudgetScope) {
///     let _ = fact.encode(other);
/// }
/// ```
pub struct CountedNormalSnapshot<'a> {
    doc: &'a Document,
    scope: &'a BudgetScope,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
    bytes: u64,
    _control: BudgetAccount,
}
impl std::fmt::Debug for CountedNormalSnapshot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountedNormalSnapshot")
            .field("document_id", &self.document_id())
            .field("state_id", &self.state_id())
            .field("wire_len", &self.wire_len())
            .finish_non_exhaustive()
    }
}
impl CountedNormalSnapshot<'_> {
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
    pub fn encode(&self) -> Result<CountedSnapshotBytes> {
        fixed::encode(self)
    }
}
pub fn normal_counted_snapshot_control_bytes() -> NormalCountedControlBytes {
    NormalCountedControlBytes {
        fact: size_of::<CountedNormalSnapshot<'static>>() as u64,
        counting_sink: counting::CountingSink::control_bytes(),
        bytes_owner: size_of::<CountedSnapshotBytes>() as u64,
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
pub fn count_portable_normal_snapshot_rooted_checked<'a>(
    doc: &'a Document,
    scope: &'a BudgetScope,
    check: &'a dyn Fn() -> Result<()>,
    spend: &'a dyn Fn(u64) -> Result<()>,
) -> Result<CountedNormalSnapshot<'a>> {
    // No additional wrapper debit; an injected check may itself charge its work.
    check()?;
    let mut control = scope.account();
    control.resize(normal_counted_snapshot_control_bytes().fact)?;
    let mut sink = counting::CountingSink::new(scope)?;
    let result = write_rooted(doc, &mut sink, scope, check, spend);
    if sink.stop().is_some() {
        return Err(limit("counted snapshot exceeds source limit"));
    }
    result?;
    let bytes = sink.bytes();
    drop(sink);
    Ok(CountedNormalSnapshot {
        doc,
        scope,
        check,
        spend,
        bytes,
        _control: control,
    })
}
#[cfg(test)]
mod tests;
