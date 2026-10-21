//! Guarded publication of recorded-history storage, independent of content edits.
use super::*;
use paintcraft_history::ResidencyMaintenance;
#[derive(Debug)]
pub struct PreparedHistoryMaintenance {
    base: Ticket,
    session: Session,
    report: ResidencyMaintenance,
}
impl Preparation {
    pub fn maintain_history(
        mut self,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedHistoryMaintenance> {
        check()?;
        let (history, report) = self.session.history.prepare_residency_step(&check)?;
        self.session.history = history;
        check()?;
        Ok(PreparedHistoryMaintenance {
            base: self.base,
            session: self.session,
            report,
        })
    }
}
impl Session {
    /// Storage publication changes no logical revision, cursor, selection or save
    /// acknowledgement. A successful replacement invalidates older preparations.
    pub fn publish_history_maintenance(
        &mut self,
        prepared: PreparedHistoryMaintenance,
        check: impl Fn() -> Result<()>,
    ) -> Result<ResidencyMaintenance> {
        check()?;
        self.validate_preparation(prepared.base, &prepared.session)?;
        if prepared.report.made_progress() {
            self.history = prepared.session.history;
            self.preparation_policy = Arc::new(());
        }
        Ok(prepared.report)
    }
}
