//! An admitted portable snapshot; compressed bytes stay private until finalization.
use super::preparation::Preparation7;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_document::Document;
use std::io::Write;

#[derive(Debug)]
pub(in crate::app::filter_editing) struct Snapshot7 {
    bytes: Vec<u8>,
    account: BudgetAccount,
}
impl Snapshot7 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Scratch/output/control upper reservations and JS credit are already live.
    /// Scope reconciliation refuses capacity excess, never borrows a new realm.
    pub fn encode(
        document: &Document,
        source_limit: usize,
        scope: &BudgetScope,
        prefix: &Preparation7,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if source_limit > 64 << 20 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Source7 snapshot byte limit",
            ));
        }
        let mut output = Sink {
            snapshot: Self {
                bytes: Vec::new(),
                account: scope.account(),
            },
            limit: source_limit,
            scope: scope.clone(),
            prefix,
            check,
            failure: None,
        };
        let result = paintcraft_formats::pcnd::write_portable_snapshot_admitted_checked(
            document,
            &mut output,
            scope,
            check,
            &|units| prefix.spend(units),
        );
        if let Some(error) = output.failure {
            return Err(error);
        }
        result?;
        Ok(output.snapshot)
    }
}
struct Sink<'a> {
    scope: BudgetScope,
    snapshot: Snapshot7,
    limit: usize,
    prefix: &'a Preparation7,
    check: &'a dyn Fn() -> Result<()>,
    failure: Option<Error>,
}
impl Sink<'_> {
    fn fit(&self, capacity: usize) -> Result<()> {
        let total = self
            .scope
            .footprint()
            .live
            .checked_sub(self.snapshot.account.bytes())
            .and_then(|n| n.checked_add(capacity as u64))
            .ok_or_else(exhausted)?;
        if total > self.limit as u64 {
            return Err(exhausted());
        }
        Ok(())
    }
    fn append(&mut self, input: &[u8]) -> Result<()> {
        (self.check)()?;
        let length = self
            .snapshot
            .bytes
            .len()
            .checked_add(input.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(exhausted)?;
        if length > self.snapshot.bytes.capacity() {
            // Reallocation may copy every existing byte; price that upper before
            // asking the allocator. New input copy+writer SHA are already priced.
            self.prefix.spend(self.snapshot.bytes.len() as u64)?;
            let requested = length
                .max(self.snapshot.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.fit(requested)?;
            self.snapshot.account.resize(requested as u64)?;
            self.snapshot
                .bytes
                .try_reserve_exact(requested - self.snapshot.bytes.len())
                .map_err(|_| exhausted())?;
            let actual = self.snapshot.bytes.capacity();
            if actual > self.limit || self.fit(actual).is_err() {
                drop(std::mem::take(&mut self.snapshot.bytes));
                self.snapshot.account.resize(0)?;
                return Err(exhausted());
            }
            if let Err(error) = self.snapshot.account.resize(actual as u64) {
                drop(std::mem::take(&mut self.snapshot.bytes));
                self.snapshot.account.resize(0)?;
                return Err(error);
            }
        }
        self.snapshot.bytes.extend_from_slice(input);
        Ok(())
    }
}
impl Write for Sink<'_> {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        match self.append(input) {
            Ok(()) => Ok(input.len()),
            Err(error) => {
                self.failure = Some(error);
                Err(std::io::Error::other("Source7 snapshot refused"))
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn exhausted() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source7 snapshot output capacity")
}
