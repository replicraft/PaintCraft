//! One OPEN owner; no frame/BEGIN constructor and no counter reset on retire.
use super::*;
use paintcraft_render::{
    noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_noise_or_current, with_noise_workspace,
    },
    spatial::{WorkspaceAdmission, capture_workspace_or_current, with_workspace},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Base,
    Noise,
}
#[derive(Clone, Debug)]
pub(super) enum OpenJob {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
impl OpenJob {
    /// Caller pre-admits service/root-control quote BEFORE this call, including
    /// the legacy base counter's Arc/Mutex allocation on a true-absence root.
    /// NoiseAdmission additionally charges its existing sidecar controls itself.
    /// root_scope is a reserved slice of the SAME outer operation budget.
    pub fn capture(mode: Mode, limits: NoiseLimits, root_scope: BudgetScope) -> Result<Self> {
        NoiseJobPolicy::BrowserV1.validate(limits)?;
        if mode == Mode::Base && limits.noise_work != 0 {
            return Err(bad("Source7 Base cannot admit Noise work"));
        }
        match mode {
            Mode::Base => Ok(Self::Base(capture_workspace_or_current(
                || WorkspaceAdmission::new(root_scope, limits.bytes, limits.base_work),
                limits.bytes,
                limits.base_work,
            )?)),
            Mode::Noise => Ok(Self::Noise(capture_noise_or_current(
                || NoiseAdmission::new_root(root_scope, NoiseJobPolicy::BrowserV1, limits),
                limits,
            )?)),
        }
    }
    pub fn workspace(&self) -> &WorkspaceAdmission {
        match self {
            Self::Base(a) => a,
            Self::Noise(a) => a.workspace(),
        }
    }
    pub fn run<T>(&self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        match self {
            Self::Base(a) => with_workspace(a.clone(), work),
            Self::Noise(a) => with_noise_workspace(a.clone(), work),
        }
    }
    pub fn spend_control(&self, work: u64) -> Result<()> {
        match self {
            Self::Base(a) => a.reserve_work(work),
            Self::Noise(a) => a.reserve_base_work(work),
        }
    }
    pub fn check_available(&self, work: WorkComponents) -> Result<()> {
        work.checked_total()?;
        match self {
            Self::Base(a) => {
                if work.noise != 0
                    || a.work_used()
                        .checked_add(work.base)
                        .is_none_or(|n| n > a.work_limit())
                {
                    return Err(resource("Source7 Base cumulative work admission"));
                }
                Ok(())
            }
            Self::Noise(a) => a.check_available(work),
        }
    }
    pub fn validate_document(&self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
        if matches!(self, Self::Base(_))
            && doc.layers.values().any(|l| {
                l.filters.iter().any(|f| {
                    matches!(
                        f.descriptor(),
                        paintcraft_core::types::filters::Descriptor::AddNoiseRgbV1(_)
                    )
                })
            })
        {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Source7 stored Noise requires a Noise OPEN",
            ));
        }
        match self {
            Self::Noise(a) => a.validate_document(doc, check),
            Self::Base(a) => {
                self.check_available(WorkComponents::default())?;
                if a.has_prepared_programs() {
                    for l in doc.layers.values() {
                        check()?;
                        paintcraft_render::prepared::validate_layer_programs(l, a, check)?;
                    }
                } else {
                    paintcraft_render::prepared::quote_document_programs(doc, check)?;
                }
                Ok(())
            }
        }
    }
    /// Retains the SAME work owner; prepared keys belong only to accepted frame.
    /// Never replace the service's unbound OPEN owner by this candidate binding.
    pub fn prepare_document(&self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        self.validate_document(doc, check)?;
        match self {
            Self::Base(a) => self.run(|| {
                paintcraft_render::prepared::prepare_document_programs(doc, a.clone(), check)
                    .map(Self::Base)
            }),
            Self::Noise(a) => a.prepare_document(doc, check).map(Self::Noise),
        }
    }
}
