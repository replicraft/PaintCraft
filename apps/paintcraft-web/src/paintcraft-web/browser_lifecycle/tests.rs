use super::*;
use std::cell::RefCell;

type StopCallback = Rc<RefCell<Option<Box<dyn Fn()>>>>;

#[derive(Clone, Default)]
struct ResourceState {
    log: Rc<RefCell<Vec<&'static str>>>,
    input: Rc<Cell<bool>>,
    runner: Rc<Cell<bool>>,
    codec: Rc<Cell<bool>>,
    during_stop: StopCallback,
}
impl Resources for ResourceState {
    fn stop(&self) {
        if self.input.replace(false) {
            assert!(self.runner.get(), "input must retire before its runner");
            self.log.borrow_mut().push("input");
        }
        if self.codec.replace(false) {
            self.log.borrow_mut().push("codec");
        }
        let callback = self.during_stop.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
        if self.runner.replace(false) {
            self.log.borrow_mut().push("runner");
        }
    }
}
impl ResourceState {
    fn initialized(&self) {
        self.runner.set(true);
        self.input.set(true);
    }
    fn empty(&self) -> bool {
        !self.runner.get() && !self.input.get() && !self.codec.get()
    }
}

#[test]
fn serial_restart_retires_input_codec_and_runner_before_new_initialization() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    let first = owner.begin().unwrap();
    first.resources().initialized();
    first.commit().unwrap();
    owner.resources().codec.set(true);
    assert!(owner.running());
    let second = owner.begin().unwrap();
    assert!(resources.empty());
    assert_eq!(&*resources.log.borrow(), &["input", "codec", "runner"]);
    assert!(!owner.running());
    second.resources().initialized();
    second.commit().unwrap();
    assert!(owner.running());
    owner.destroy();
    assert!(resources.empty());
    assert!(!owner.running());
    owner.destroy();
    assert_eq!(resources.log.borrow().len(), 5, "cleanup is idempotent");
}

#[test]
fn overlapping_start_refusal_does_not_retire_the_existing_pending_owner() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    let start = owner.begin().unwrap();
    start.resources().initialized();
    assert!(matches!(owner.begin(), Err(StartError::Busy)));
    assert!(start.current());
    assert!(resources.runner.get() && resources.input.get());
    assert!(resources.log.borrow().is_empty());
    start.commit().unwrap();
    assert!(owner.running());
}

#[test]
fn destroy_during_start_holds_exclusivity_until_late_resources_are_cleaned() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    let start = owner.begin().unwrap();
    resources.runner.set(true); // Graphics initialization has begun.
    owner.destroy();
    assert!(!start.current());
    assert!(resources.empty());
    assert!(matches!(owner.begin(), Err(StartError::Busy)));
    // An external GPU future can finish after destroy. It must not resurrect
    // input/rendering, or overlap a new initialization on this same canvas.
    resources.initialized();
    assert!(start.commit().is_err());
    assert!(resources.empty());
    assert!(!owner.running());
    let fresh = owner.begin().unwrap();
    fresh.resources().initialized();
    fresh.commit().unwrap();
    assert!(owner.running());
}

#[test]
fn failed_start_cleanup_and_dropped_future_release_only_after_retirement() {
    for has_input in [false, true] {
        let resources = ResourceState::default();
        let owner = Lifecycle::new(resources.clone());
        let start = owner.begin().unwrap();
        resources.runner.set(true);
        resources.input.set(has_input);
        drop(start); // The same lease Drop runs on every ? failure path.
        assert!(resources.empty());
        assert!(!owner.running());
        let retry = owner.begin().unwrap();
        assert!(retry.current());
        drop(retry);
    }
}

#[test]
fn public_owner_drop_retires_running_resources_without_explicit_destroy() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    let start = owner.begin().unwrap();
    resources.initialized();
    start.commit().unwrap();
    resources.codec.set(true);
    drop(owner);
    assert!(resources.empty());
    assert_eq!(&*resources.log.borrow(), &["input", "codec", "runner"]);
}

#[test]
fn public_owner_drop_invalidates_pending_owned_future_and_its_late_success() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    let start = owner.begin().unwrap();
    resources.runner.set(true);
    drop(owner); // The owned Promise lease is still alive, unlike its JS owner.
    assert!(resources.empty());
    assert!(!start.current());
    resources.initialized();
    assert!(start.commit().is_err());
    assert!(resources.empty());
}

#[test]
fn generation_exhaustion_never_wraps_into_a_valid_start() {
    let resources = ResourceState::default();
    let owner = Lifecycle::new(resources.clone());
    owner.inner.generation.set(u64::MAX - 1);
    assert!(matches!(owner.begin(), Err(StartError::Exhausted)));
    owner.destroy();
    owner.destroy();
    assert!(matches!(owner.begin(), Err(StartError::Exhausted)));
    assert!(!owner.running());
    assert!(resources.empty());
}

#[test]
fn reentrant_destroy_and_begin_during_stop_keep_the_pending_lease_exclusive() {
    for during_restart in [false, true] {
        let resources = ResourceState::default();
        let owner = Rc::new(Lifecycle::new(resources.clone()));
        let first = owner.begin().unwrap();
        resources.initialized();
        let weak = Rc::downgrade(&owner);
        let calls = Rc::new(Cell::new(0));
        let recorded = calls.clone();
        // This is the external-call boundary inside Resources.stop, after
        // taking its callback/binding and ending the RefCell borrow.
        let callback = Box::new(move || {
            recorded.set(recorded.get() + 1);
            let owner = weak.upgrade().unwrap();
            owner.destroy();
            assert!(matches!(owner.begin(), Err(StartError::Busy)));
            assert!(!owner.running());
        });
        let cancelled = if during_restart {
            first.commit().unwrap();
            *resources.during_stop.borrow_mut() = Some(callback);
            owner.begin().unwrap() // stop reenters after the new latch is set.
        } else {
            *resources.during_stop.borrow_mut() = Some(callback);
            owner.destroy();
            first
        };
        assert_eq!(calls.get(), 1);
        assert!(!cancelled.current());
        assert!(matches!(owner.begin(), Err(StartError::Busy)));
        resources.initialized(); // Late external initialization completion.
        assert!(cancelled.commit().is_err());
        assert!(resources.empty());
        assert!(!owner.running());
        let fresh = owner.begin().unwrap();
        resources.initialized();
        fresh.commit().unwrap();
        assert!(owner.running());
    }
}
