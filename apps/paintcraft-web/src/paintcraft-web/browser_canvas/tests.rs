use super::*;
use crate::browser_lifecycle::{Lifecycle, StartError};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
struct Identity(Rc<()>);
impl Identity {
    fn new() -> Self {
        Self(Rc::new(()))
    }
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Default)]
struct State {
    entries: RefCell<Vec<(Identity, Entry<Identity>)>>,
    live: Cell<bool>,
    fail_storage: Cell<bool>,
    events: RefCell<Vec<&'static str>>,
}
#[derive(Clone, Default)]
struct Map(Rc<State>);
impl Registry for Map {
    type Canvas = Identity;
    type Owner = Identity;
    fn get(&self, canvas: &Identity) -> Result<Option<Entry<Identity>>, Error> {
        if self.0.fail_storage.get() {
            return Err(Error::Storage);
        }
        Ok(self.0.entries.borrow().iter().find_map(|(key, entry)| {
            (key == canvas).then(|| Entry {
                owner: entry.owner.clone(),
                leases: entry.leases,
            })
        }))
    }
    fn put(&self, canvas: &Identity, entry: Entry<Identity>) -> Result<(), Error> {
        if self.0.fail_storage.get() {
            return Err(Error::Storage);
        }
        let mut entries = self.0.entries.borrow_mut();
        if let Some((_, value)) = entries.iter_mut().find(|(key, _)| key == canvas) {
            *value = entry;
        } else {
            entries.push((canvas.clone(), entry));
        }
        Ok(())
    }
    fn remove(&self, canvas: &Identity) {
        assert!(
            !self.0.live.get(),
            "canvas released before renderer cleanup"
        );
        self.0.entries.borrow_mut().retain(|(key, _)| key != canvas);
        self.0.events.borrow_mut().push("canvas");
    }
}
#[derive(Clone, Default)]
struct Running {
    map: Map,
    claim: Rc<RefCell<Option<Lease<Map>>>>,
}
impl Resources for Running {
    fn stop(&self) {
        if self.map.0.live.replace(false) {
            self.map.0.events.borrow_mut().push("renderer");
        }
        let claim = self.claim.borrow_mut().take();
        drop(claim);
    }
}

#[test]
fn exact_canvas_identity_refuses_other_owner_but_distinct_canvases_are_independent() {
    let map = Map::default();
    let canvas = Identity::new();
    let first = Lease::acquire(map.clone(), canvas.clone(), Identity::new()).unwrap();
    assert!(matches!(
        Lease::acquire(map.clone(), canvas.clone(), Identity::new()),
        Err(Error::Busy)
    ));
    let second = Lease::acquire(map.clone(), Identity::new(), Identity::new()).unwrap();
    assert_eq!(map.0.entries.borrow().len(), 2);
    drop(first);
    assert_eq!(map.0.entries.borrow().len(), 1);
    drop(second);
    assert!(map.0.entries.borrow().is_empty());
}

#[test]
fn foreign_canvas_refusal_preserves_callers_valid_other_canvas_and_running_resources() {
    let resource = Running::default();
    let owner = Lifecycle::new(resource.clone());
    let busy_canvas = Identity::new();
    let foreign =
        Lease::acquire(resource.map.clone(), busy_canvas.clone(), Identity::new()).unwrap();
    let own_canvas = Identity::new();
    let own_token = Identity::new();
    let own = Lease::acquire(resource.map.clone(), own_canvas.clone(), own_token.clone()).unwrap();
    let start = ClaimedStart::new(owner.begin().unwrap(), own);
    resource.map.0.live.set(true);
    start
        .commit(|r, claim| *r.claim.borrow_mut() = Some(claim))
        .unwrap();
    assert!(matches!(
        Lease::acquire(resource.map.clone(), busy_canvas, own_token),
        Err(Error::Busy)
    ));
    // The production entry point acquires the canvas before calling begin.
    assert!(owner.running());
    assert!(resource.map.0.live.get());
    assert_eq!(resource.map.get(&own_canvas).unwrap().unwrap().leases, 1);
    assert!(resource.map.0.events.borrow().is_empty());
    owner.destroy();
    drop(foreign);
}

#[test]
fn same_owner_restart_keeps_claim_while_old_running_lease_is_retired() {
    let map = Map::default();
    let canvas = Identity::new();
    let owner = Identity::new();
    let first = Lease::acquire(map.clone(), canvas.clone(), owner.clone()).unwrap();
    let next = Lease::acquire(map.clone(), canvas.clone(), owner).unwrap();
    assert_eq!(map.get(&canvas).unwrap().unwrap().leases, 2);
    drop(first);
    assert_eq!(map.get(&canvas).unwrap().unwrap().leases, 1);
    assert!(matches!(
        Lease::acquire(map.clone(), canvas.clone(), Identity::new()),
        Err(Error::Busy)
    ));
    drop(next);
    assert!(map.get(&canvas).unwrap().is_none());
}

#[test]
fn per_handle_busy_alias_drops_without_releasing_original_pending_canvas() {
    let resource = Running::default();
    let owner = Lifecycle::new(resource.clone());
    let canvas = Identity::new();
    let token = Identity::new();
    let claim = Lease::acquire(resource.map.clone(), canvas.clone(), token.clone()).unwrap();
    let pending = ClaimedStart::new(owner.begin().unwrap(), claim);
    let extra = Lease::acquire(resource.map.clone(), canvas.clone(), token).unwrap();
    assert!(matches!(owner.begin(), Err(StartError::Busy)));
    drop(extra);
    assert!(pending.current());
    assert_eq!(resource.map.get(&canvas).unwrap().unwrap().leases, 1);
    drop(pending);
    assert!(resource.map.get(&canvas).unwrap().is_none());
}

#[test]
fn free_keeps_pending_canvas_claim_until_late_renderer_is_stopped() {
    let resource = Running::default();
    let owner = Lifecycle::new(resource.clone());
    let canvas = Identity::new();
    let claim = Lease::acquire(resource.map.clone(), canvas.clone(), Identity::new()).unwrap();
    let pending = ClaimedStart::new(owner.begin().unwrap(), claim);
    resource.map.0.live.set(true);
    drop(owner);
    assert!(matches!(
        Lease::acquire(resource.map.clone(), canvas.clone(), Identity::new()),
        Err(Error::Busy)
    ));
    resource.map.0.live.set(true); // Late GPU completion after public free.
    assert!(!pending.current());
    drop(pending);
    assert_eq!(
        &*resource.map.0.events.borrow(),
        &["renderer", "renderer", "canvas"]
    );
    let next = Lease::acquire(resource.map.clone(), canvas, Identity::new()).unwrap();
    drop(next);
}

#[test]
fn adopted_running_claim_releases_only_after_renderer_and_failed_start_releases_too() {
    for commit in [false, true] {
        let resource = Running::default();
        let owner = Lifecycle::new(resource.clone());
        let canvas = Identity::new();
        let claim = Lease::acquire(resource.map.clone(), canvas.clone(), Identity::new()).unwrap();
        let pending = ClaimedStart::new(owner.begin().unwrap(), claim);
        pending.resources().map.0.live.set(true);
        if commit {
            pending
                .commit(|resource, claim| *resource.claim.borrow_mut() = Some(claim))
                .unwrap();
            assert!(resource.map.get(&canvas).unwrap().is_some());
            owner.destroy();
        } else {
            drop(pending);
        }
        assert!(resource.map.get(&canvas).unwrap().is_none());
        assert_eq!(&*resource.map.0.events.borrow(), &["renderer", "canvas"]);
    }
}

#[test]
fn stale_release_never_removes_foreign_owner_and_exhaustion_or_storage_fail_closed() {
    let map = Map::default();
    let canvas = Identity::new();
    let token = Identity::new();
    let stale = Lease::acquire(map.clone(), canvas.clone(), Identity::new()).unwrap();
    map.put(
        &canvas,
        Entry {
            owner: token.clone(),
            leases: u32::MAX,
        },
    )
    .unwrap();
    drop(stale);
    assert!(matches!(
        Lease::acquire(map.clone(), canvas.clone(), token),
        Err(Error::Capacity)
    ));
    assert_eq!(map.get(&canvas).unwrap().unwrap().leases, u32::MAX);
    map.0.fail_storage.set(true);
    assert!(matches!(
        Lease::acquire(map, Identity::new(), Identity::new()),
        Err(Error::Storage)
    ));
}
