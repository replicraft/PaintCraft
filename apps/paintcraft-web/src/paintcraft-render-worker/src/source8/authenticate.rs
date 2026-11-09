//! The pre-Plan authentication debit belongs to the original captured caller.
use super::entry::EntryAuth8;
use paintcraft_core::types::Result;
use paintcraft_formats::pcnd::CheckedContainer;

pub(super) fn checked<'a>(
    bytes: &'a [u8],
    auth: &EntryAuth8<'_>,
) -> Result<(CheckedContainer<'a>, [u8; 32])> {
    auth.spend(64)?;
    let work = (bytes.len() as u64)
        .checked_mul(2)
        .ok_or_else(super::assembly_ledger::limit)?;
    auth.spend(work)?;
    let check = || auth.step(0);
    CheckedContainer::verify_with_source_hash(bytes, &check)
}
