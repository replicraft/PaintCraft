//! Shared scoped generation, retained across dialog cancellation, never document state.
use super::*;
use std::sync::Mutex;
pub(crate) type SharedGeneration = Arc<Mutex<installation::Generation>>;
fn lock(shared: &SharedGeneration) -> Result<std::sync::MutexGuard<'_, installation::Generation>> {
    shared.lock().map_err(|_| {
        Error::new(
            ErrorCode::InvariantViolation,
            "JXR generation lock poisoned",
        )
    })
}
pub(super) fn require(shared: &SharedGeneration, ticket: installation::Ticket) -> Result<()> {
    lock(shared)?.require_current(ticket)
}
/// Only called inside the original admitted worker or existing staged checkpoint.
/// Filesystem work never runs with the generation lock held.
pub(super) fn reobserve(
    shared: &SharedGeneration,
    ticket: installation::Ticket,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let request = {
        let mut g = lock(shared)?;
        g.require_current(ticket)?;
        g.begin_refresh()?
    };
    let observation = installation::observe(check)?;
    let current = lock(shared)?.complete(request, observation)?;
    if current != ticket {
        return Err(stale());
    }
    Ok(())
}
pub(super) fn start_refresh(
    shared: &SharedGeneration,
    service: &controller::background::Service,
    state: &mut State,
) -> Result<()> {
    if state.refresh.is_some() {
        return Ok(());
    }
    // No filesystem inspection unless the real editing executor admitted a job.
    if let Some(pending) = service.try_submit_jxr_observation()? {
        let request = lock(shared)?.begin_refresh()?;
        state.refresh = Some((request, pending));
    }
    Ok(())
}
pub(super) fn poll_refresh(shared: &SharedGeneration, state: &mut State) -> Result<()> {
    let Some(result) = state.refresh.as_ref().and_then(|(_, p)| p.poll()) else {
        return Ok(());
    };
    let (request, _pending) = state.refresh.take().ok_or_else(stale)?;
    let current = lock(shared)?.complete(request, result?)?;
    if let Some(expected) = state.ticket {
        if current != expected {
            return Err(stale());
        }
    } else {
        state.ticket = Some(current);
    }
    Ok(())
}
