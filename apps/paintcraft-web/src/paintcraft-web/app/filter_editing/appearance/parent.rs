//! Capture the real caller before async submission; no fallback counter is minted.
use paintcraft_core::types::{Error, ErrorCode, Result, resources::BudgetAccount};
use paintcraft_document::Document;
use paintcraft_formats::export::layer_appearance::{Admission7, Mode7, Origin7};
use paintcraft_render::{
    noise_work::{NoiseAdmission, NoiseJobPolicy, WorkComponents, capture_noise_or_current},
    spatial::{WorkspaceAdmission, capture_current_workspace},
};
use std::cell::Cell;
#[derive(Clone, Debug)]
// Capture before admission without introducing an unadmitted heap allocation.
#[allow(clippy::large_enum_variant)]
pub(super) enum Parent7 {
    Standalone,
    Captured {
        base: WorkspaceAdmission,
        noise: Option<NoiseAdmission>,
        noise_error: Option<Error>,
    },
}
/// Conservative remote scratch claim in the actual captured scope. This is
/// logical ownership, not a process RSS/allocator estimate or a new memory pool.
#[derive(Debug)]
pub(super) struct Remote7 {
    _account: BudgetAccount,
    _parent: Parent7,
}
fn denied() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source7 captured authority exhausted",
    )
}
impl Parent7 {
    pub fn capture() -> Result<Self> {
        let base = capture_current_workspace();
        let absent = Cell::new(false);
        let noise = capture_noise_or_current(
            || {
                absent.set(true);
                Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Source7 absent paired caller",
                ))
            },
            NoiseJobPolicy::BrowserV1.max_limits(),
        );
        match base {
            None if absent.get() => Ok(Self::Standalone),
            None => Err(noise.err().unwrap_or_else(denied)),
            Some(base) => {
                let (noise, noise_error) = match noise {
                    Ok(noise) => (Some(noise), None),
                    Err(error) => (None, Some(error)),
                };
                Ok(Self::Captured {
                    base,
                    noise,
                    noise_error,
                })
            }
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Standalone, Self::Standalone) => true,
            (
                Self::Captured {
                    base: a, noise: an, ..
                },
                Self::Captured {
                    base: b, noise: bn, ..
                },
            ) => {
                a.same_job(b)
                    && a.bytes_limit() == b.bytes_limit()
                    && a.work_limit() == b.work_limit()
                    && match (an, bn) {
                        (None, None) => true,
                        (Some(a), Some(b)) => {
                            a.same_job(b)
                                && a.limits().bytes == b.limits().bytes
                                && a.limits().base_work == b.limits().base_work
                                && a.limits().noise_work == b.limits().noise_work
                        }
                        _ => false,
                    }
            }
            _ => false,
        }
    }
    /// Query retained owners, never whatever TLS happens to exist after an await.
    pub fn limits(&self) -> Result<(u64, u64, u64)> {
        match self {
            Self::Standalone => Ok((
                Admission7::WORKSPACE_MAX,
                Admission7::BASE_MAX,
                Admission7::NOISE_MAX,
            )),
            Self::Captured { base, noise, .. } => {
                let base_limit = base
                    .work_limit()
                    .min(Admission7::BASE_MAX)
                    .min(noise.as_ref().map_or(u64::MAX, |n| n.limits().base_work));
                let base_remaining = base_limit
                    .checked_sub(base.work_used())
                    .ok_or_else(denied)?;
                let noise_remaining = if let Some(n) = noise {
                    n.limits()
                        .noise_work
                        .min(Admission7::NOISE_MAX)
                        .checked_sub(n.used()?.noise)
                        .ok_or_else(denied)?
                } else {
                    0
                };
                Ok((
                    base.bytes_limit()
                        .min(Admission7::WORKSPACE_MAX)
                        .min(noise.as_ref().map_or(u64::MAX, |n| n.limits().bytes)),
                    base_remaining,
                    noise_remaining,
                ))
            }
        }
    }
    pub fn origin(&self) -> Origin7 {
        match self {
            Self::Standalone => Origin7::StandaloneViewJobs,
            Self::Captured { .. } => Origin7::CapturedJob,
        }
    }
    pub fn validate_mode(&self, mode: Mode7) -> Result<()> {
        if mode == Mode7::Noise
            && let Self::Captured {
                noise: None,
                noise_error,
                ..
            } = self
        {
            return Err(noise_error.clone().unwrap_or_else(denied));
        }
        Ok(())
    }
    pub fn spend(&self, work: u64) -> Result<()> {
        match self {
            Self::Standalone => Ok(()),
            Self::Captured { noise: Some(n), .. } => n.reserve_base_work(work),
            Self::Captured { base, .. } => base.reserve_work(work),
        }
    }
    /// Preserve frozen program membership before transferring to a realm whose
    /// TLS has no knowledge of this bound owner. No compile/cache extension.
    pub fn validate_document(
        &self,
        doc: &Document,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<()> {
        let Self::Captured { base, noise, .. } = self else {
            return Ok(());
        };
        if !base.has_prepared_programs() {
            return Ok(());
        }
        let levels = base.prepared_levels().map_or(0, |j| j.len());
        let points = base.prepared_programs().map_or(0, |j| j.len());
        let exposures = points.saturating_sub(levels);
        let curves = base
            .prepared_curves()
            .map_or(0, |j| j.len())
            .saturating_sub(points);
        for layer in doc.layers.values() {
            check()?;
            spend(1)?;
            // One pricing prewalk and the three existing enabled-filter passes.
            // Comparisons in linear LUT lookups are separately bounded by each
            // actual index length and the policy's canonical byte width.
            spend(
                (layer.filters.len() as u64)
                    .checked_mul(4)
                    .ok_or_else(denied)?,
            )?;
            for filter in layer.filters.iter().filter(|f| f.enabled()) {
                use paintcraft_core::types::filters::Descriptor;
                let (count, width) = match filter.descriptor() {
                    Descriptor::LevelsRgbV1(p) => (levels, p.semantic_bytes().len()),
                    Descriptor::ExposureSrgbV1(p) => (exposures, p.semantic_bytes().len()),
                    Descriptor::CurvesRgbV1(p) => (curves, p.semantic_bytes().len()),
                    _ => (0, 0),
                };
                spend(
                    (count as u64)
                        .checked_mul(width as u64)
                        .ok_or_else(denied)?,
                )?;
            }
        }
        let charged = || {
            check()?;
            spend(1)
        };
        if let Some(noise) = noise {
            noise.validate_document(doc, &charged)
        } else {
            for layer in doc.layers.values() {
                charged()?;
                paintcraft_render::prepared::validate_layer_programs(layer, base, &charged)?;
            }
            Ok(())
        }
    }
    /// A transfer is a debit, not a copy of remaining authority. Sibling spending
    /// can make this fail; failure never selects None, refreshes or refunds.
    pub fn claim(&self, admission: Admission7) -> Result<Option<Remote7>> {
        let remote = match self {
            Self::Standalone => None,
            Self::Captured { base, .. } => {
                let mut account = base.scope().account();
                account.resize(admission.workspace_bytes)?;
                Some(Remote7 {
                    _account: account,
                    _parent: self.clone(),
                })
            }
        };
        match self {
            Self::Standalone => Ok(()),
            Self::Captured { noise: Some(n), .. } => n.reserve_transferred_work(WorkComponents {
                base: admission.base_work,
                noise: admission.noise_work,
            }),
            Self::Captured { base, .. } if admission.noise_work == 0 => {
                base.reserve_work(admission.base_work)
            }
            _ => Err(denied()),
        }?;
        Ok(remote)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::filter_editing::appearance::preparation::Preparation7;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    use paintcraft_render::{
        noise_work::{NoiseLimits, with_noise_workspace},
        spatial::with_workspace,
    };
    #[test]
    fn captured_base_survives_scope_exit_and_debits_same_owner_through_seal() {
        let budget = SharedBudget::new(4096);
        let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 513, 200);
        parent.reserve_work(7).unwrap();
        let captured = with_workspace(parent.clone(), Parent7::capture).unwrap();
        assert!(capture_current_workspace().is_none()); // async continuation has no TLS
        let prefix = Preparation7::from_parent(captured).unwrap();
        let initial = prefix.select_mode(Mode7::Base).unwrap();
        assert_eq!(
            (
                initial.workspace_bytes,
                initial.base_work,
                initial.noise_work
            ),
            (513, 193, 0)
        );
        assert_eq!(initial.work_authority, Origin7::CapturedJob);
        prefix.spend(11).unwrap();
        assert_eq!(parent.work_used(), 18);
        let sealed = prefix.seal(3).unwrap();
        assert_eq!(sealed.admission.base_work, 179);
        assert_eq!(parent.work_used(), 200);
        assert!(parent.reserve_work(1).is_err());
        assert_eq!(budget.usage().reserved, 513);
        drop(sealed);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn paired_noise_transfer_is_consumed_once_and_parallel_copy_cannot_mint_it() {
        let budget = SharedBudget::new(16384);
        let parent = NoiseAdmission::new_root(
            budget.scope(AllocationClass::SharedBudget),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 4096,
                base_work: 300,
                noise_work: 500,
            },
        )
        .unwrap();
        parent.reserve_base_work(3).unwrap();
        let captured = with_noise_workspace(parent.clone(), Parent7::capture).unwrap();
        let first = Preparation7::from_parent(captured.clone()).unwrap();
        let second = Preparation7::from_parent(captured).unwrap();
        let initial = first.select_mode(Mode7::Noise).unwrap();
        second.select_mode(Mode7::Noise).unwrap();
        assert_eq!(
            (
                initial.workspace_bytes,
                initial.base_work,
                initial.noise_work
            ),
            (4096, 297, 500)
        );
        first.spend(5).unwrap();
        let sealed = first.seal(7).unwrap();
        assert_eq!(
            (parent.used().unwrap().base, parent.used().unwrap().noise),
            (300, 500)
        );
        assert_eq!(second.seal(0).unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(
            (parent.used().unwrap().base, parent.used().unwrap().noise),
            (300, 500)
        );
        drop(sealed);
    }
    #[test]
    fn some_without_noise_pair_and_exhausted_some_never_choose_standalone() {
        let budget = SharedBudget::new(4096);
        let parent = WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 128, 4);
        let captured = with_workspace(parent.clone(), Parent7::capture).unwrap();
        let prefix = Preparation7::from_parent(captured.clone()).unwrap();
        assert_eq!(
            prefix.select_mode(Mode7::Noise).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        parent.reserve_work(4).unwrap();
        let prefix = Preparation7::from_parent(captured).unwrap();
        assert_eq!(prefix.spend(1).unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(parent.work_used(), 4);
        let none = Parent7::capture().unwrap();
        assert_eq!(none.origin(), Origin7::StandaloneViewJobs);
    }
    #[test]
    fn actual_reserved_parent_scope_keeps_remote_scratch_credit_until_owner_drop() {
        let budget = SharedBudget::new(65536);
        let scope = budget
            .reserve_capacity(1024)
            .unwrap()
            .scope(AllocationClass::SharedBudget);
        let parent = WorkspaceAdmission::new(scope, 1024, 100);
        let captured = with_workspace(parent.clone(), Parent7::capture).unwrap();
        let admission = Admission7 {
            work_authority: Origin7::CapturedJob,
            mode: Mode7::Base,
            workspace_bytes: 1024,
            base_work: 1,
            noise_work: 0,
        };
        let first = captured.claim(admission).unwrap();
        assert_eq!(
            captured.claim(admission).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(parent.work_used(), 1); // failed memory claim never spends quota
        drop(first);
        let second = captured.claim(admission).unwrap();
        assert_eq!(parent.work_used(), 2);
        drop(second);
        assert_eq!(budget.usage().reserved, 1024); // live parent reservation still owns capacity
    }
}
