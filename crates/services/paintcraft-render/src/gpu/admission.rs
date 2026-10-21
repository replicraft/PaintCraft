//! Synchronous request scope; only unobserved submissions retain its charge.
use super::*;
use paintcraft_core::types::resources::ResourceLease;
use std::cell::RefCell;
use std::sync::Arc;

pub(super) struct Scope {
    _lease: ResourceLease,
    _parent: Option<Arc<Scope>>,
    depth: usize,
    progress: Option<std::sync::Weak<progress::State>>,
}
thread_local! {
    pub(super) static ACTIVE: RefCell<Option<Arc<Scope>>> = const { RefCell::new(None) };
}
struct Restore(Option<Arc<Scope>>);
impl Drop for Restore {
    fn drop(&mut self) {
        // Budget observers may run on final release. Do not retain a RefCell borrow.
        let old = ACTIVE.with(|active| active.replace(self.0.take()));
        drop(old);
    }
}
/// Attach the existing admitted charge to synchronous GPU calls on this thread.
/// Scopes restore on error/unwind, do not propagate to spawned tasks and nest at
/// most 16 deep. Nested submissions retain every enclosing charge via the chain.
pub fn with_admission<T>(lease: ResourceLease, work: impl FnOnce() -> Result<T>) -> Result<T> {
    scoped(lease, None, work)
}
/// Observe deferred completion on a matching device even when its canvas is idle.
pub fn with_admission_progress<T>(
    lease: ResourceLease,
    driver: &CompletionDriver,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    scoped(lease, Some(driver.weak()), work)
}
fn scoped<T>(
    lease: ResourceLease,
    progress: Option<std::sync::Weak<progress::State>>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let parent = ACTIVE.with(|active| active.borrow().clone());
    let depth = parent.as_ref().map_or(1, |scope| scope.depth + 1);
    if depth > 16 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "GPU admission scope nesting exceeds limit",
        ));
    }
    let scope = Arc::new(Scope {
        _lease: lease,
        _parent: parent.clone(),
        depth,
        progress,
    });
    ACTIVE.with(|active| *active.borrow_mut() = Some(scope));
    let _restore = Restore(parent);
    work()
}

pub(super) struct Submission {
    pub index: Option<wgpu::SubmissionIndex>,
    queue: wgpu::Queue,
    identity: Arc<()>,
    owner: Option<Arc<Scope>>,
}
impl Submission {
    pub fn wait(mut self, device: &wgpu::Device, epoch: &DeviceEpoch) -> Result<()> {
        completion::wait(
            device,
            epoch,
            self.index.take().expect("submitted GPU work"),
        )?;
        self.owner = None;
        Ok(())
    }
}
impl Drop for Submission {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            let mut node = Some(owner.as_ref());
            let mut ticket = None;
            while let Some(scope) = node {
                if let Some(state) = scope.progress.as_ref().and_then(std::sync::Weak::upgrade) {
                    ticket = state.track(&self.identity);
                    if ticket.is_some() {
                        break;
                    }
                }
                node = scope._parent.as_deref();
            }
            self.queue.on_submitted_work_done(move || {
                drop(owner);
                drop(ticket);
            });
        }
    }
}
pub(super) fn submit(
    queue: &wgpu::Queue,
    identity: &Arc<()>,
    encoder: wgpu::CommandEncoder,
) -> Submission {
    let owner = ACTIVE.with(|active| active.borrow().clone());
    // Install ownership before submit too: an unwind after dispatch must retain it.
    let mut submission = Submission {
        index: None,
        queue: queue.clone(),
        identity: identity.clone(),
        owner,
    };
    submission.index = Some(queue.submit([encoder.finish()]));
    submission
}
