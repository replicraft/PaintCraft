//! Bounded negative metadata is visible even while an earlier frame owns a job.
use super::*;
impl Projection {
    pub(crate) fn retain_framework_marker(&mut self, serial: u64) -> Result<()> {
        self.push(None, Pointer::Framework(serial))
            .map_err(|_| resource("Framework FIFO marker"))
    }
    pub(crate) fn discard_retired_entries(&mut self, cancellation: &NativePenCancellation) {
        for entry in &mut self.queue {
            if matches!(&entry.pointer, Pointer::Pen(batch) if cancellation.matches_batch(batch)) {
                entry.pointer = Pointer::Retired;
            }
        }
    }
    pub(super) fn retire_active(
        &mut self,
        ordinal: Option<u64>,
    ) -> std::result::Result<(), RecordFault> {
        let cancellations = self
            .source
            .retire_active_native_pen()
            .map_err(|_| RecordFault::Invalid)?;
        self.span_first = None;
        for cancellation in cancellations.into_iter().flatten() {
            if self.retirements.len() == self.limit {
                return Err(RecordFault::Capacity);
            }
            self.retirements.push(cancellation.clone());
            self.push(ordinal, Pointer::Retire(cancellation))?;
        }
        Ok(())
    }
    /// Both vectors were reserved/charged before capture. Never allocate or
    /// truncate a prefix under the installation/ingress lock.
    pub(crate) fn drain_retirements(
        &mut self,
        output: &mut Vec<NativePenCancellation>,
    ) -> Result<()> {
        if output
            .len()
            .checked_add(self.retirements.len())
            .is_none_or(|size| size > output.capacity())
        {
            self.fault();
            return Err(resource("Native retirement handoff capacity"));
        }
        output.append(&mut self.retirements);
        Ok(())
    }
}
