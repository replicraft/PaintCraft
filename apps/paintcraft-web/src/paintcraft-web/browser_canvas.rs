//! Exact canvas identity is shared across public handles, unlike runner state.
use super::browser_lifecycle::{Resources, Start};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Busy,
    Capacity,
    Storage,
}
pub(crate) struct Entry<O> {
    pub owner: O,
    pub leases: u32,
}
pub(crate) trait Registry: Clone {
    type Canvas: Clone;
    type Owner: Clone + PartialEq;
    fn get(&self, canvas: &Self::Canvas) -> Result<Option<Entry<Self::Owner>>, Error>;
    fn put(&self, canvas: &Self::Canvas, entry: Entry<Self::Owner>) -> Result<(), Error>;
    fn remove(&self, canvas: &Self::Canvas);
}
pub(crate) struct Lease<M: Registry> {
    registry: M,
    canvas: M::Canvas,
    owner: M::Owner,
}
impl<M: Registry> Lease<M> {
    pub fn acquire(registry: M, canvas: M::Canvas, owner: M::Owner) -> Result<Self, Error> {
        let leases = if let Some(entry) = registry.get(&canvas)? {
            if entry.owner != owner {
                return Err(Error::Busy);
            }
            entry.leases.checked_add(1).ok_or(Error::Capacity)?
        } else {
            1
        };
        registry.put(
            &canvas,
            Entry {
                owner: owner.clone(),
                leases,
            },
        )?;
        Ok(Self {
            registry,
            canvas,
            owner,
        })
    }
}
impl<M: Registry> Drop for Lease<M> {
    fn drop(&mut self) {
        let Ok(Some(entry)) = self.registry.get(&self.canvas) else {
            return;
        };
        if entry.owner != self.owner {
            return;
        }
        if entry.leases == 1 {
            self.registry.remove(&self.canvas);
        } else if entry.leases > 1 {
            // Storage failure retains ownership, never releases another owner.
            let _ = self.registry.put(
                &self.canvas,
                Entry {
                    owner: entry.owner,
                    leases: entry.leases - 1,
                },
            );
        }
    }
}
/// Always clean a pending renderer before releasing its canvas, including free
/// while awaiting GPU work. Running ownership is moved into Resources at commit.
pub(crate) struct ClaimedStart<R: Resources, M: Registry> {
    start: Option<Start<R>>,
    claim: Option<Lease<M>>,
}
impl<R: Resources, M: Registry> ClaimedStart<R, M> {
    pub fn new(start: Start<R>, claim: Lease<M>) -> Self {
        Self {
            start: Some(start),
            claim: Some(claim),
        }
    }
    pub fn current(&self) -> bool {
        self.start.as_ref().is_some_and(Start::current)
    }
    pub fn resources(&self) -> &R {
        self.start
            .as_ref()
            .expect("owned startup lease")
            .resources()
    }
    pub fn commit(mut self, adopt: impl FnOnce(&R, Lease<M>)) -> Result<(), ()> {
        let start = self.start.take().expect("owned startup lease");
        let claim = self.claim.take().expect("owned canvas lease");
        adopt(start.resources(), claim);
        start.commit()
    }
}
impl<R: Resources, M: Registry> Drop for ClaimedStart<R, M> {
    fn drop(&mut self) {
        let start = self.start.take();
        drop(start);
        let claim = self.claim.take();
        drop(claim);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;
#[cfg(test)]
mod tests;
