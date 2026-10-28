//! Owned startup leases prevent an async runner from outliving its public owner.
use std::{cell::Cell, rc::Rc};

pub(crate) trait Resources {
    /// Idempotent; retire input callbacks before their runner and editor.
    fn stop(&self);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartError {
    Busy,
    Exhausted,
}
pub(crate) struct Lifecycle<R: Resources> {
    inner: Rc<Inner<R>>,
}
struct Inner<R: Resources> {
    resources: R,
    generation: Cell<u64>,
    pending: Cell<Option<u64>>,
    running: Cell<bool>,
}
/// Keeps initialization exclusively owned even after destroy/free invalidates it.
/// Its final cleanup must finish before another initialization can begin.
pub(crate) struct Start<R: Resources> {
    inner: Rc<Inner<R>>,
    generation: u64,
    armed: bool,
}
impl<R: Resources> Lifecycle<R> {
    pub fn new(resources: R) -> Self {
        Self {
            inner: Rc::new(Inner {
                resources,
                generation: Cell::new(0),
                pending: Cell::new(None),
                running: Cell::new(false),
            }),
        }
    }
    pub fn resources(&self) -> &R {
        &self.inner.resources
    }
    pub fn running(&self) -> bool {
        self.inner.running.get()
    }
    pub fn begin(&self) -> Result<Start<R>, StartError> {
        if self.inner.pending.get().is_some() {
            return Err(StartError::Busy);
        }
        let generation = self
            .inner
            .generation
            .get()
            .checked_add(1)
            .filter(|next| *next != u64::MAX)
            .ok_or(StartError::Exhausted)?;
        self.inner.generation.set(generation);
        self.inner.pending.set(Some(generation));
        self.inner.running.set(false);
        let start = Start {
            inner: self.inner.clone(),
            generation,
            armed: true,
        };
        self.inner.resources.stop();
        Ok(start)
    }
    pub fn destroy(&self) {
        self.inner
            .generation
            .set(self.inner.generation.get().saturating_add(1));
        self.inner.running.set(false);
        self.inner.resources.stop();
        // A pending future can still allocate graphics resources. Retain its
        // latch until that exact lease performs final cleanup, not just until
        // destroy returns; two initializations must never touch the same canvas.
    }
}
impl<R: Resources> Drop for Lifecycle<R> {
    fn drop(&mut self) {
        self.destroy();
    }
}
impl<R: Resources> Start<R> {
    pub fn resources(&self) -> &R {
        &self.inner.resources
    }
    pub fn current(&self) -> bool {
        self.inner.pending.get() == Some(self.generation)
            && self.inner.generation.get() == self.generation
    }
    pub fn commit(mut self) -> Result<(), ()> {
        if !self.current() {
            return Err(());
        }
        self.inner.running.set(true);
        self.inner.pending.set(None);
        self.armed = false;
        Ok(())
    }
}
impl<R: Resources> Drop for Start<R> {
    fn drop(&mut self) {
        if self.armed {
            self.inner.resources.stop();
            self.inner.running.set(false);
            if self.inner.pending.get() == Some(self.generation) {
                self.inner.pending.set(None);
            }
        }
    }
}

#[cfg(test)]
mod tests;
