use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    A,
    B,
    C,
}
#[derive(Debug)]
struct Owner {
    id: Key,
    drops: Rc<Cell<usize>>,
    identity: Rc<()>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn owner(id: Key, drops: &Rc<Cell<usize>>) -> Owner {
    Owner {
        id,
        drops: drops.clone(),
        identity: Rc::new(()),
    }
}

#[test]
fn three_keys_restore_exact_owners_and_drop_each_once() {
    let drops = Rc::new(Cell::new(0));
    let mut active = owner(Key::A, &drops);
    let mut owners = Owners::default();
    let mut identities = [None; 3];
    identities[Key::A as usize] = Some(Rc::as_ptr(&active.identity));
    for target in [Key::B, Key::C, Key::A, Key::C, Key::B, Key::A] {
        let old = active.id;
        let mut transfer = owners.prepare(old, target).unwrap();
        let mut destination = transfer
            .take_destination()
            .unwrap_or_else(|| owner(target, &drops));
        std::mem::swap(&mut active, &mut destination);
        transfer.finish(destination);
        assert_eq!(active.id, target);
        let identity = Rc::as_ptr(&active.identity);
        if let Some(original) = identities[target as usize] {
            assert_eq!(identity, original);
        } else {
            identities[target as usize] = Some(identity);
        }
        assert!(owners.get(target).is_none());
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(owners.entries.len(), 2);
    assert_eq!(owners.get(Key::B).unwrap().id, Key::B);
    assert_eq!(owners.get(Key::C).unwrap().id, Key::C);
    drop(active);
    assert_eq!(drops.get(), 1);
    drop(owners);
    assert_eq!(drops.get(), 3);
}

#[test]
fn prepared_existing_target_rolls_back_and_absent_constructor_failure_consumes_nothing() {
    let drops = Rc::new(Cell::new(0));
    let mut owners = Owners::default();
    owners.entries.push((Key::B, owner(Key::B, &drops)));
    owners.entries.push((Key::C, owner(Key::C, &drops)));
    let identity = Rc::as_ptr(&owners.get(Key::B).unwrap().identity);
    {
        let transfer = owners.prepare(Key::A, Key::B).unwrap();
        assert!(transfer.has_destination());
        // A fallible caller construction returns before taking the target.
    }
    assert_eq!(Rc::as_ptr(&owners.get(Key::B).unwrap().identity), identity);
    assert_eq!(owners.get(Key::C).unwrap().id, Key::C);
    assert_eq!(drops.get(), 0);
    let mut empty: Owners<Key, Owner> = Owners::default();
    drop(empty.prepare(Key::A, Key::B).unwrap());
    assert!(empty.entries.is_empty());
    drop(owners);
    assert_eq!(drops.get(), 2);
}

#[test]
fn same_or_retained_active_key_refuses_without_overwriting_any_owner() {
    let drops = Rc::new(Cell::new(0));
    let mut owners = Owners::default();
    owners.entries.push((Key::B, owner(Key::B, &drops)));
    assert_eq!(
        owners.prepare(Key::A, Key::A).unwrap_err(),
        Refusal::SameKey
    );
    assert_eq!(
        owners.prepare(Key::B, Key::C).unwrap_err(),
        Refusal::ActiveRetained
    );
    assert_eq!(owners.entries.len(), 1);
    assert_eq!(owners.get(Key::B).unwrap().id, Key::B);
    assert_eq!(drops.get(), 0);
}
