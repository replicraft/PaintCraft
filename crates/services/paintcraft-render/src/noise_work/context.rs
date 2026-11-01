use super::*;
use crate::spatial::WorkspaceAdmission;
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
    let previous = ACTIVE.with(|a| a.borrow().clone());
    let depth = previous.as_ref().map_or(1, |(_, d)| d + 1);
    if depth > 16 {
        return Err(limit());
    }
    let admission = match crate::spatial::current() {
        Some(base) => {
            let parent = previous
                .as_ref()
                .ok_or_else(missing)?
                .0
                .for_workspace(&base)?;
            if !parent.same_job(&admission) {
                return Err(missing());
            }
            admission.child(parent.limits())?.for_workspace(&base)?
        }
        None => {
            if previous.is_some() {
                return Err(missing());
            }
            admission
        }
    };
    admission.check_available(WorkComponents::default())?;
    let base = admission.workspace().clone();
    ACTIVE.with(|a| *a.borrow_mut() = Some((admission, depth)));
    let _restore = Restore(previous);
    crate::spatial::with_workspace(base, work)
}
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
