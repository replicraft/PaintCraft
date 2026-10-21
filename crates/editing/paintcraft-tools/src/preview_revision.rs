//! Process-local change tokens for derived preview caches, never document IDs.
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PreviewRevision {
    era: Uuid,
    sequence: u64,
}
impl Default for PreviewRevision {
    fn default() -> Self {
        Self {
            era: Uuid::new_v4(),
            sequence: 0,
        }
    }
}
impl PreviewRevision {
    pub fn advance(&mut self) {
        if let Some(sequence) = self.sequence.checked_add(1) {
            self.sequence = sequence;
        } else {
            *self = Self::default();
        }
    }
}
