//! Inactive workspace owners keyed by the experience that created them.
use paintcraft_core::types::{Error, ErrorCode, Result};

#[derive(Debug)]
pub(crate) struct Parked<K, V> {
    entries: Vec<(K, V)>,
}
impl<K, V> Default for Parked<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
impl<K: Copy + Eq, V> Parked<K, V> {
    /// Admission occurs before taking a destination or moving the active owner.
    pub(crate) fn prepare(&mut self, active: K, target: K) -> Result<()> {
        if active == target || self.entries.iter().any(|(key, _)| *key == active) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "Invalid workspace ownership",
            ));
        }
        if !self.entries.iter().any(|(key, _)| *key == target) {
            self.entries.try_reserve(1).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "Cannot retain another workspace")
            })?;
        }
        Ok(())
    }
    pub(crate) fn take(&mut self, key: K) -> Option<V> {
        let index = self.entries.iter().position(|(k, _)| *k == key)?;
        Some(self.entries.swap_remove(index).1)
    }
    /// Refusal returns the untouched incoming owner; it never replaces an entry.
    pub(crate) fn insert_reserved(
        &mut self,
        active: K,
        key: K,
        owner: V,
    ) -> std::result::Result<(), V> {
        if active == key
            || self.entries.iter().any(|(k, _)| *k == key)
            || self.entries.len() == self.entries.capacity()
        {
            return Err(owner);
        }
        self.entries.push((key, owner));
        Ok(())
    }
    pub(crate) fn values(&self) -> impl Clone + Iterator<Item = &V> {
        self.entries.iter().map(|(_, owner)| owner)
    }
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.entries.iter_mut().map(|(_, owner)| owner)
    }
    #[cfg(test)]
    pub(crate) fn get(&self, key: K) -> Option<&V> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, owner)| owner)
    }
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
impl super::App {
    pub(crate) fn park_workspace_for_test(
        &mut self,
        key: super::Experience,
        owner: Box<super::tabs::WorkspaceDocuments>,
    ) {
        let active = self.experience();
        self.parked_workspaces.prepare(active, key).unwrap();
        assert!(
            self.parked_workspaces
                .insert_reserved(active, key, owner)
                .is_ok()
        );
    }
}

#[cfg(test)]
#[path = "workspaces/tests.rs"]
mod tests;
