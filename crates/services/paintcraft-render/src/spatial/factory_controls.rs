//! Fresh work-counter control for explicit standalone operation owners.
use super::{Result, WorkspaceAdmission, limit};
use std::{
    mem::{align_of, size_of},
    sync::Mutex,
};

impl WorkspaceAdmission {
    /// Logical inline `Arc<Mutex<u64>>` created by one fresh workspace factory.
    ///
    /// Pure defining-type fact: no scope, work owner, reservation or authority is
    /// created. Clones/tightened children reuse the same counter allocation.
    /// The inline factory handle, original scope's controls, prepared jobs and
    /// separately quoted Noise sidecars are excluded. This also excludes
    /// allocator overhead/slack, RSS and lazy OS mutex backing allocations.
    /// Legacy constructors retain their existing bookkeeping contract; an
    /// embedding host uses this fact to precharge its new explicit operation.
    pub fn control_allocation_quote() -> Result<u64> {
        let align = align_of::<Mutex<u64>>().max(align_of::<usize>());
        let error = || limit("Workspace work-counter control layout");
        let offset = (2 * size_of::<usize>())
            .checked_next_multiple_of(align_of::<Mutex<u64>>())
            .ok_or_else(error)?;
        offset
            .checked_add(size_of::<Mutex<u64>>())
            .and_then(|n| n.checked_next_multiple_of(align))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(error)
    }
}

#[cfg(test)]
#[path = "factory_control_tests.rs"]
mod tests;
