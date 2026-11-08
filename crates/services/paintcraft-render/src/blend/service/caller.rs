//! Retain actual Some/pair and minima, never mint an execution authority.
use super::*;
use crate::noise_work::{NoiseLimits, WorkComponents};
pub(super) struct Resolved {
    pub base: WorkspaceAdmission,
    pub noise: Option<NoiseAdmission>,
    pub ambient: Option<WorkspaceAdmission>,
}
impl Resolved {
    pub fn new(caller: Caller<'_>) -> Result<Self> {
        let ambient = crate::spatial::current();
        let ambient_noise = crate::noise_work::current();
        let (explicit, noise) = match caller {
            Caller::Base(a) => {
                if ambient_noise.is_some() {
                    return Err(missing());
                }
                (a, None)
            }
            Caller::Noise(a) => (a.workspace(), Some(a)),
        };
        if let Some(current) = &ambient
            && !current.same_job(explicit)
        {
            return Err(missing());
        }
        let mut base = explicit.clone();
        if let Some(current) = &ambient {
            // A frozen ambient owner cannot be bypassed with an unbound sibling.
            if !base.has_prepared_programs() && current.has_prepared_programs() {
                base = current.clone();
            }
            base = base.tightened(
                explicit.bytes_limit().min(current.bytes_limit()),
                explicit.work_limit().min(current.work_limit()),
            )?;
        }
        let noise = match (noise, ambient_noise, ambient.as_ref()) {
            (None, None, _) => None,
            (Some(a), None, None) => Some(a.for_workspace(&base)?),
            (Some(a), Some(current), Some(current_base)) => {
                let current = current.for_workspace(current_base)?;
                if !a.same_job(&current) {
                    return Err(missing());
                }
                let limits = current.limits();
                Some(
                    a.child(NoiseLimits {
                        bytes: limits.bytes.min(base.bytes_limit()),
                        base_work: limits.base_work.min(base.work_limit()),
                        noise_work: limits.noise_work,
                    })?
                    .for_workspace(&base)?,
                )
            }
            _ => return Err(missing()),
        };
        if let Some(n) = &noise {
            base = n.workspace().clone();
        }
        base.reserve_work(0)?;
        Ok(Self {
            base,
            noise,
            ambient,
        })
    }
    pub fn install<T>(&self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        match &self.noise {
            Some(a) => crate::noise_work::with_noise_workspace(a.clone(), || {
                // Existing pair install may retain the ambient base binding.
                // Install the validated explicit binding without replacing its pair.
                crate::spatial::with_workspace(self.base.clone(), work)
            }),
            None => crate::spatial::with_workspace(self.base.clone(), work),
        }
    }
    pub fn spend(&self, units: u64) -> Result<()> {
        match &self.noise {
            Some(a) => a.reserve_base_work(units),
            None => self.base.reserve_work(units),
        }
    }
    /// External callbacks are trusted immediate SAME-owner debits, not grants.
    /// Check actual resolved minima before debit and again before the helper.
    /// Never debit twice, infer refunds, or turn a wider callback into authority.
    pub fn metadata_spend(&self, units: u64, spend: &dyn Fn(u64) -> Result<()>) -> Result<()> {
        self.check_compile(units)?;
        spend(units)?;
        self.check_compile(0)
    }
    /// Both actual bound complete owners remain live through install/restore.
    pub fn required_retained_bytes(&self) -> Result<u64> {
        crate::prepared::blend_programs::bound_retained_bytes(&self.base, self.ambient.as_ref())
    }
    pub fn check_compile(&self, work: u64) -> Result<()> {
        match &self.noise {
            Some(a) => a.check_available(WorkComponents {
                base: work,
                noise: 0,
            }),
            None => {
                if self
                    .base
                    .work_used()
                    .checked_add(work)
                    .is_none_or(|v| v > self.base.work_limit())
                {
                    Err(limit())
                } else {
                    Ok(())
                }
            }
        }
    }
}
