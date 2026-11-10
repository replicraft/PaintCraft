//! Defining reader controls for an independently admitted source forecast.
use super::{metadata, owner_ledger};
use paintcraft_core::types::Result;

/// Fixed reader controls, excluding source-dependent payloads and scratch.
///
/// These quantities describe existing constructors. They grant no capacity,
/// create no factories and do not establish the complete C or K declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8ReaderControlBytes {
    /// Original C account retained by Entry throughout the loaded source phase.
    pub entry: u64,
    /// Additional C controls live during document assembly and adoption.
    pub assembly: u64,
    /// K storage for the two private Arc blocks, including their logical headers.
    pub retained_carriers: u64,
}

/// Return compiled layout facts without inspecting a source or capturing a job.
///
/// The returned 24-byte value and query/result controls belong to the caller.
/// On supported targets a successful query performs no allocation, admission,
/// locking or callback invocation. Refusals use the existing owned diagnostics.
/// A forecast must add source-dependent constructor controls and scratch at
/// their actual overlapping phases. Logical Arc sizes exclude allocator
/// overhead, physical RSS and separate operating-system allocations.
///
/// # Errors
/// Returns ResourceLimit for an unsupported target layout or size overflow.
pub fn source8_reader_control_bytes() -> Result<Source8ReaderControlBytes> {
    owner_ledger::validate_layouts()?;
    Ok(Source8ReaderControlBytes {
        entry: owner_ledger::ENTRY_BYTES,
        assembly: metadata::controls()?,
        retained_carriers: owner_ledger::retained_bytes()?,
    })
}

#[cfg(test)]
mod tests;
