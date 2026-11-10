//! Keyed inactive owners; a prepared transfer retains rollback ownership.
#[derive(Debug)]
pub(in crate::app) struct Owners<K, V> {
    entries: Vec<(K, V)>,
}
impl<K, V> Default for Owners<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    SameKey,
    ActiveRetained,
    Capacity,
}
#[derive(Debug)]
pub(super) struct Transfer<'a, K: Copy + Eq, V> {
    owners: &'a mut Owners<K, V>,
    old: K,
    target: K,
    destination: Option<V>,
}
impl<K: Copy + Eq, V> Owners<K, V> {
    pub(super) fn prepare(&mut self, old: K, target: K) -> Result<Transfer<'_, K, V>, Refusal> {
        if old == target {
            return Err(Refusal::SameKey);
        }
        if self.entries.iter().any(|(key, _)| *key == old) {
            return Err(Refusal::ActiveRetained);
        }
        let index = self.entries.iter().position(|(key, _)| *key == target);
        // Removing an existing target already creates the outgoing vacancy.
        // An absent target needs one slot before any ownership/input mutation.
        if index.is_none() {
            self.entries.try_reserve(1).map_err(|_| Refusal::Capacity)?;
        }
        let destination = index.map(|index| self.entries.swap_remove(index).1);
        Ok(Transfer {
            owners: self,
            old,
            target,
            destination,
        })
    }
    #[cfg(test)]
    pub(super) fn get(&self, key: K) -> Option<&V> {
        self.entries
            .iter()
            .find(|(stored, _)| *stored == key)
            .map(|(_, owner)| owner)
    }
}
impl<K: Copy + Eq, V> Transfer<'_, K, V> {
    pub(super) fn has_destination(&self) -> bool {
        self.destination.is_some()
    }
    pub(super) fn take_destination(&mut self) -> Option<V> {
        self.destination.take()
    }
    pub(super) fn finish(self, outgoing: V) {
        assert!(
            self.destination.is_none(),
            "destination must be adopted before parking outgoing owner"
        );
        // prepare holds the unique mutable borrow and reserved this vacancy.
        self.owners.entries.push((self.old, outgoing));
    }
}
impl<K: Copy + Eq, V> Drop for Transfer<'_, K, V> {
    fn drop(&mut self) {
        if let Some(destination) = self.destination.take() {
            // Any fallible preparation failure restores the exact target owner.
            self.owners.entries.push((self.target, destination));
        }
    }
}

#[cfg(test)]
#[path = "owners/tests.rs"]
mod tests;
