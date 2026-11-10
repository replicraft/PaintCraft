//! Only accepted compiled dispatch creates a root; absence never selects fallback.
use super::{
    manifest::{Open8, W},
    policy::*,
};
use paintcraft_core::types::{
    Result,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_render::{
    noise_work::{self, NoiseAdmission, NoiseJobPolicy, NoiseLimits, WorkComponents},
    spatial::{self, WorkspaceAdmission},
};
#[repr(C, u8)]
pub(super) enum Job8 {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
#[repr(C)]
pub(super) struct OperationJob8 {
    pub job: Job8,
    // Actual counter storage retires before its original H account/factory.
    _counter: BudgetAccount,
    _factory: BudgetScope,
}
impl OperationJob8 {
    pub fn new(
        dispatch: &super::StandaloneDispatch8,
        open: &Open8,
        workspace: &BudgetScope,
        h: &BudgetScope,
    ) -> Result<Self> {
        if !matches!(dispatch.kind, 1 | 2) {
            return Err(missing());
        }
        if spatial::capture_current_workspace().is_some()
            || noise_work::capture_current_noise_workspace().is_some()
        {
            return Err(missing());
        }
        let original = workspace.capacity_snapshot();
        if !original.reserved || original.ceiling != open.roles[W] {
            return Err(bound());
        }
        let mut counter = h.account();
        counter.resize(WorkspaceAdmission::control_allocation_quote()?)?;
        let job = if open.noise() {
            Job8::Noise(NoiseAdmission::new_root(
                workspace.clone(),
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: JOB,
                    base_work: open.base_work,
                    noise_work: open.noise_work,
                },
            )?)
        } else {
            Job8::Base(WorkspaceAdmission::new(
                workspace.clone(),
                JOB,
                open.base_work,
            ))
        };
        Ok(Self {
            job,
            _counter: counter,
            _factory: h.clone(),
        })
    }
    pub fn spend(&self, n: u64) -> Result<()> {
        match &self.job {
            Job8::Base(b) => b.reserve_work(n),
            Job8::Noise(p) => p.reserve_base_work(n),
        }
    }
    pub fn used(&self) -> Result<WorkComponents> {
        match &self.job {
            Job8::Base(b) => Ok(WorkComponents {
                base: b.work_used(),
                noise: 0,
            }),
            Job8::Noise(p) => p.used(),
        }
    }
    pub fn run<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        match &self.job {
            Job8::Base(b) => spatial::with_workspace(b.clone(), f),
            Job8::Noise(p) => noise_work::with_noise_workspace(p.clone(), f),
        }
    }
    pub fn request(&self) -> crate::source8::Source8InspectionRequest {
        crate::source8::Source8InspectionRequest {
            mode: match &self.job {
                Job8::Base(_) => crate::source8::Source8InspectionMode::Base,
                Job8::Noise(_) => crate::source8::Source8InspectionMode::Noise,
            },
            caller: match &self.job {
                Job8::Base(b) => NoiseLimits {
                    bytes: JOB,
                    base_work: b.work_limit(),
                    noise_work: 0,
                },
                Job8::Noise(p) => p.limits(),
            },
            source: Default::default(),
        }
    }
}
