//! Captured Studio style intent shared by owner-side publication and preview.
use paintcraft_commands::StyleOp;
use uuid::Uuid;

/// One style operation against the exact session revision seen by the user.
///
/// Host intentions must use stable UUID layer and existing-style references,
/// never an active-layer alias resolved after the operation enters a queue.
/// The alias introduced by Add or Duplicate remains the command's local output
/// alias. Hosts validate the operation and captured base before resolving text,
/// at actual FIFO execution, and before accepting a private preview result.
/// This value is not publication authority and owns no document or history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleEdit {
    pub owner: Uuid,
    pub state_id: Uuid,
    pub publication_epoch: u64,
    pub operation: StyleOp,
}

impl StyleEdit {
    /// Cheap identity check; reference validity and resource policy remain the
    /// receiving host/engine's responsibility. A pre-enqueue check alone cannot
    /// authorize execution after earlier queued work changes this revision.
    pub fn matches_base(&self, owner: Uuid, state_id: Uuid, publication_epoch: u64) -> bool {
        self.owner == owner
            && self.state_id == state_id
            && self.publication_epoch == publication_epoch
    }
}
