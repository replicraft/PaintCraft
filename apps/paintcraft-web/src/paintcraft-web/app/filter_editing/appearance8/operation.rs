//! Actual original preparation counter; one explicit new counter per new VIEW.
use super::{denied, factory::Factory8};
use paintcraft_core::types::{Result, resources::BudgetAccount};
use paintcraft_render::{
    noise_work::{NoiseAdmission, capture_current_noise_workspace},
    spatial::{WorkspaceAdmission, capture_current_workspace, capture_required_workspace},
};
pub(super) const WORK: u64 = 268435456;
#[derive(Clone)]
pub(super) struct Parent8 {
    base: Option<WorkspaceAdmission>,
    noise: Option<NoiseAdmission>,
}
impl Parent8 {
    /// Synchronous original capture before ANY await. No context installation.
    pub fn capture() -> Result<Self> {
        let base = match capture_current_workspace() {
            Some(original) => Some(capture_required_workspace(original.bytes_limit(), WORK)?),
            None => None,
        };
        let noise = capture_current_noise_workspace();
        if noise.is_some() && base.is_none() {
            return Err(denied());
        }
        Ok(Self { base, noise })
    }
}
pub(super) struct Operation8 {
    // Actual counters/shared owners precede their charge and factory retirement.
    base: WorkspaceAdmission,
    _noise: Option<NoiseAdmission>,
    _counter: BudgetAccount,
    factory: Factory8,
}
impl Operation8 {
    pub fn preparation(parent: Parent8, factory: Factory8) -> Result<Self> {
        let mut counter = factory.scope().account();
        let base = match parent.base {
            Some(base) => base,
            None => {
                counter.resize(WorkspaceAdmission::control_allocation_quote()?)?;
                WorkspaceAdmission::new(factory.scope().clone(), factory.cap(), WORK)
            }
        };
        Ok(Self {
            base,
            _noise: parent.noise,
            _counter: counter,
            factory,
        })
    }
    /// Called once at accepted strictly-new Main VIEW dispatch, never perpatch,
    /// cancellation, copy retry or retirement poll. Original factory stays SAME.
    pub fn view(factory: Factory8) -> Result<Self> {
        if factory.is_quarantined() {
            return Err(denied());
        }
        Self::preparation(
            Parent8 {
                base: None,
                noise: None,
            },
            factory,
        )
    }
    pub fn spend(&self, amount: u64) -> Result<()> {
        self.base.reserve_work(amount)
    }
    pub fn step(&self, raw_check: &dyn Fn() -> Result<()>) -> Result<()> {
        self.spend(1024)?;
        raw_check()
    }
    pub fn factory(&self) -> &Factory8 {
        &self.factory
    }
    #[cfg(test)]
    pub fn used(&self) -> u64 {
        self.base.work_used()
    }
}

impl std::fmt::Debug for Parent8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainParent8")
            .field("captured", &self.base.is_some())
            .field("noise", &self.noise.is_some())
            .finish()
    }
}
impl std::fmt::Debug for Operation8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainOperation8")
            .field("work", &self.base.work_used())
            .finish_non_exhaustive()
    }
}
