//! Read-only demand for actual complete prepared payload owners.
use super::WorkspaceAdmission;
use paintcraft_core::types::Result;

impl WorkspaceAdmission {
    /// Sum distinct complete prepared payload owners by their actual identity.
    /// Equal numerical keys or a shared work counter do not establish sharing.
    /// This excludes Arc bookkeeping, as `prepared_retained_bytes` does, and
    /// neither admits capacity nor validates caller/counter compatibility.
    pub fn prepared_owner_union_bytes(&self, other: Option<&Self>) -> Result<u64> {
        crate::prepared::blend_programs::bound_retained_bytes(self, other)
    }
}

#[cfg(test)]
mod tests;
