//! One preserving storage-maintenance step on an owned private timeline.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidencyMaintenance {
    pub packed_entries: usize,
    pub transferred_payloads: usize,
}
impl ResidencyMaintenance {
    pub fn made_progress(self) -> bool {
        self.packed_entries != 0 || self.transferred_payloads != 0
    }
}
impl History {
    /// Consume a private fork; errors discard it without replacing its owner.
    /// Keep the current and newest snapshots hot; never trim recorded history.
    /// Scans/transfers are bounded per step, not by a native latency deadline.
    pub fn prepare_residency_step(
        mut self,
        check: impl Fn() -> Result<()>,
    ) -> Result<(Self, ResidencyMaintenance)> {
        check()?;
        let packed_entries = if self.packing == Packing::Adaptive {
            self.compact_limited(1, 64 << 20, 64, 64, &check)?
                .packed_entries
        } else {
            0
        };
        let transferred_payloads = if self.spill_store.is_some() {
            self.spill_unpinned(64 << 20, 64, &check)?.payloads
        } else {
            self.refine_deferred(64 << 20, 64, &check)?.payloads
        };
        check()?;
        Ok((
            self,
            ResidencyMaintenance {
                packed_entries,
                transferred_payloads,
            },
        ))
    }
}
