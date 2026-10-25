//! Shared per-association versions; no binding lock is held during file I/O.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_platform::save::{self, Expectation, Outcome, Replacement};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct Binding(Arc<Mutex<State>>);
#[derive(Debug)]
struct State {
    expected: Option<Expectation>,
    allowed: bool,
    busy: bool,
    basis: Option<Uuid>,
}
#[derive(Clone, Debug)]
pub(crate) struct Previous {
    pub path: Option<PathBuf>,
    pub binding: Option<Binding>,
}

/// Compare regular-file identities without opening a FIFO or device.
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    if matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b) {
        return true;
    }
    matches!((a.metadata(), b.metadata()), (Ok(a), Ok(b)) if a.is_file() && b.is_file())
        && same_file::is_same_file(a, b).unwrap_or(false)
}
pub(crate) fn acquire(path: &Path) -> Result<(Vec<u8>, Binding)> {
    let resolved = std::fs::canonicalize(path).map_err(|e| {
        Error::new(
            ErrorCode::StorageFailed,
            format!("Cannot resolve opened document: {}", e.kind()),
        )
    })?;
    let bytes = crate::format_admission::read_source(&resolved)?;
    let binding = Binding::new(Some(Expectation::acquired(&resolved, &bytes)));
    Ok((bytes, binding))
}
impl Binding {
    fn new(expected: Option<Expectation>) -> Self {
        Self(Arc::new(Mutex::new(State {
            expected,
            allowed: true,
            busy: false,
            basis: None,
        })))
    }
    /// Only an explicitly selected new association may start without a baseline.
    pub fn authorized() -> Self {
        Self::new(None)
    }
    pub fn from_outcome(outcome: &Outcome) -> Option<Self> {
        if outcome.replacement != Replacement::Replaced {
            return None;
        }
        let expected = expectation(outcome)?;
        Some(Self(Arc::new(Mutex::new(State {
            expected: Some(expected),
            allowed: true,
            busy: false,
            basis: Some(outcome.operation_id()),
        }))))
    }
    pub fn save(&self, write: impl FnOnce(Option<&Expectation>) -> Outcome) -> Outcome {
        let expected = {
            let Ok(mut state) = self.0.lock() else {
                return denied("Save binding is unavailable");
            };
            if state.busy || !state.allowed {
                return denied(
                    "Resolve the earlier save outcome or choose Save As before saving again",
                );
            }
            state.busy = true;
            state.expected.clone()
        };
        let mut flight = Flight {
            binding: self.clone(),
            armed: true,
        };
        let outcome = write(expected.as_ref());
        if let Ok(mut state) = self.0.lock() {
            state.busy = false;
            match outcome.replacement {
                Replacement::NotAttempted | Replacement::NotReplaced => {}
                Replacement::Replaced => advance(&mut state, &outcome),
                Replacement::Unknown => {
                    state.allowed = false;
                    state.basis = Some(outcome.operation_id());
                }
            }
        }
        flight.armed = false;
        outcome
    }
    /// Late inspection cannot regress a binding already advanced by a later write.
    pub fn reconcile(&self, outcome: &Outcome) {
        let Ok(mut state) = self.0.lock() else { return };
        if state.busy || state.basis != Some(outcome.operation_id()) {
            return;
        }
        match outcome.replacement {
            Replacement::Replaced => advance(&mut state, outcome),
            Replacement::NotReplaced => {
                state.allowed = state.expected.as_ref().is_some_and(|expected| {
                    outcome.observed_sha256.as_ref() == Some(&expected.sha256)
                });
            }
            Replacement::NotAttempted | Replacement::Unknown => state.allowed = false,
        }
    }
}
fn expectation(outcome: &Outcome) -> Option<Expectation> {
    outcome.written_expectation()
}

fn advance(state: &mut State, outcome: &Outcome) {
    state.expected = expectation(outcome);
    state.allowed = state.expected.is_some();
    state.basis = Some(outcome.operation_id());
}
fn denied(message: &str) -> Outcome {
    save::not_attempted(&Error::new(ErrorCode::ConflictStaleBase, message))
}
struct Flight {
    binding: Binding,
    armed: bool,
}
impl Drop for Flight {
    fn drop(&mut self) {
        if self.armed
            && let Ok(mut state) = self.binding.0.lock()
        {
            state.busy = false;
            state.allowed = false;
            state.basis = None;
        }
    }
}

#[cfg(test)]
mod tests;
