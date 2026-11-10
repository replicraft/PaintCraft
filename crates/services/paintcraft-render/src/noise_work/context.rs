#[cfg(test)]
use super::NoiseJobPolicy;
use super::{NoiseAdmission, NoiseLimits, WorkComponents, limit, missing};
use crate::spatial::WorkspaceAdmission;
#[cfg(test)]
use paintcraft_core::types::ErrorCode;
use paintcraft_core::types::Result;
use std::cell::RefCell;
thread_local! { static ACTIVE: RefCell<Option<(NoiseAdmission,usize)>> = const { RefCell::new(None) }; }
struct Restore(Option<(NoiseAdmission, usize)>);
impl Drop for Restore {
    fn drop(&mut self) {
        let old = ACTIVE.with(|a| a.replace(self.0.take()));
        drop(old);
    }
}
pub(crate) fn current() -> Option<NoiseAdmission> {
    ACTIVE.with(|a| a.borrow().as_ref().map(|(v, _)| v.clone()))
}
/// Clone the actual installed pair, with no fallback, tightening or new counter.
/// A returned handle is an observation, not evidence that Base TLS matches it.
pub fn capture_current_noise_workspace() -> Option<NoiseAdmission> {
    current()
}
pub(crate) fn current_for(base: &WorkspaceAdmission) -> Result<NoiseAdmission> {
    current().ok_or_else(missing)?.for_workspace(base)
}
pub(crate) fn required() -> Result<NoiseAdmission> {
    current_for(&crate::spatial::current().ok_or_else(missing)?)
}
pub fn with_noise_workspace<T>(
    admission: NoiseAdmission,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // Work must retire before a rejected original input, just like parameters.
    let mut admission = Some(admission);
    let callback = work;
    let (_restore, base) = enter(&mut admission)?;
    crate::spatial::with_workspace(base, callback)
}
// Keep one pair/depth/installation body for all callback and result types.
#[inline(never)]
fn enter(admission: &mut Option<NoiseAdmission>) -> Result<(Restore, WorkspaceAdmission)> {
    let previous = ACTIVE.with(|a| a.borrow().clone());
    let depth = previous.as_ref().map_or(1, |(_, d)| d + 1);
    if depth > 16 {
        return Err(limit());
    }
    let admission = match crate::spatial::current() {
        Some(base) => {
            // The original Some-Base path borrowed its input to derive a child;
            // retain that original owner in the wrapper through work/restore.
            let original = admission.as_ref().expect("unconsumed context input");
            let parent = previous
                .as_ref()
                .ok_or_else(missing)?
                .0
                .for_workspace(&base)?;
            if !parent.same_job(original) {
                return Err(missing());
            }
            original.child(parent.limits())?.for_workspace(&base)?
        }
        None => {
            if previous.is_some() {
                return Err(missing());
            }
            // Only the original absent-Base branch moved its input into TLS.
            admission.take().expect("unconsumed context input")
        }
    };
    admission.check_available(WorkComponents::default())?;
    let base = admission.workspace().clone();
    ACTIVE.with(|a| *a.borrow_mut() = Some((admission, depth)));
    Ok((Restore(previous), base))
}
const _: () =
    assert!(std::mem::size_of::<Option<NoiseAdmission>>() <= std::mem::size_of::<NoiseAdmission>());

pub fn capture_noise_or_current(
    fallback: impl FnOnce() -> Result<NoiseAdmission>,
    requested: NoiseLimits,
) -> Result<NoiseAdmission> {
    match (crate::spatial::current(), current()) {
        (Some(base), Some(admission)) => {
            drop(fallback);
            admission.for_workspace(&base)?.child(requested)
        }
        (None, None) => fallback()?.child(requested),
        _ => {
            drop(fallback);
            Err(missing())
        }
    }
}
/// Submitter captured BOTH contexts. An absent Noise context clears unrelated
/// worker TLS; installing the captured legacy workspace is the caller's task.
pub(crate) fn captured<T>(
    admission: &Option<NoiseAdmission>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // A pool worker may retain another job's paired TLS. Replace, don't inherit.
    let old = ACTIVE.with(|a| a.replace(None));
    let _restore = Restore(old);
    crate::spatial::captured(&None, || match admission {
        Some(a) => with_noise_workspace(a.clone(), work),
        None => work(),
    })
}

/// Capture only the actual matching Base/Noise pair and tighten its existing
/// component ceilings. Absence, half-contexts and mismatches never mint a root
/// or install TLS. Both original counters/control owners remain shared.
pub fn capture_required_noise_workspace(requested: NoiseLimits) -> Result<NoiseAdmission> {
    match (crate::spatial::current(), current()) {
        (Some(base), Some(admission)) => admission.for_workspace(&base)?.child(requested),
        _ => Err(missing()),
    }
}
#[cfg(test)]
mod required_tests;

#[cfg(test)]
#[path = "context/entry_tests.rs"]
mod entry_tests;
