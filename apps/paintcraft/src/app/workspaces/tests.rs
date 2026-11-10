use super::*;
use std::{cell::Cell, rc::Rc};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Key {
    A,
    B,
    C,
}
struct Owner {
    id: u8,
    drops: Rc<Cell<u8>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn owner(id: u8, drops: &Rc<Cell<u8>>) -> Box<Owner> {
    Box::new(Owner {
        id,
        drops: drops.clone(),
    })
}

#[test]
fn three_keys_roundtrip_preserves_exact_unique_owners_and_drops_once() {
    let drops = Rc::new(Cell::new(0));
    let mut store = Parked::default();
    let mut active = Key::A;
    let mut current = owner(1, &drops);
    let first = &*current as *const Owner;
    for target in [Key::B, Key::C, Key::A, Key::C, Key::B, Key::A] {
        store.prepare(active, target).unwrap();
        let mut next = store.take(target).unwrap_or_else(|| {
            owner(
                match target {
                    Key::A => 1,
                    Key::B => 2,
                    Key::C => 3,
                },
                &drops,
            )
        });
        std::mem::swap(&mut current, &mut next);
        assert!(store.insert_reserved(target, active, next).is_ok());
        active = target;
        assert!(store.get(active).is_none());
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(current.id, 1);
    assert_eq!(&*current as *const Owner, first);
    drop(store);
    assert_eq!(drops.get(), 2);
    drop(current);
    assert_eq!(drops.get(), 3);
}

#[test]
fn refusal_returns_owner_without_overwrite_or_active_key_adoption() {
    let drops = Rc::new(Cell::new(0));
    let mut store = Parked::default();
    let incoming = owner(1, &drops);
    let pointer = &*incoming as *const Owner;
    let incoming = store
        .insert_reserved(Key::A, Key::B, incoming)
        .err()
        .unwrap();
    assert_eq!(&*incoming as *const Owner, pointer);
    store.prepare(Key::A, Key::B).unwrap();
    assert!(store.insert_reserved(Key::A, Key::B, incoming).is_ok());
    let duplicate = owner(2, &drops);
    let duplicate = store
        .insert_reserved(Key::A, Key::B, duplicate)
        .err()
        .unwrap();
    assert_eq!(store.get(Key::B).unwrap().id, 1);
    let duplicate = store
        .insert_reserved(Key::A, Key::A, duplicate)
        .err()
        .unwrap();
    assert_eq!(duplicate.id, 2);
    assert_eq!(drops.get(), 0);
    assert!(store.prepare(Key::B, Key::C).is_err());
    assert!(store.prepare(Key::A, Key::A).is_err());
    drop(duplicate);
    drop(store);
    assert_eq!(drops.get(), 2);
}

#[test]
fn failed_reservation_preserves_existing_identity_and_key() {
    let drops = Rc::new(Cell::new(0));
    let mut store = Parked::default();
    store.prepare(Key::A, Key::B).unwrap();
    assert!(
        store
            .insert_reserved(Key::A, Key::B, owner(7, &drops))
            .is_ok()
    );
    let pointer = &**store.get(Key::B).unwrap() as *const Owner;
    // Actual Vec capacity-overflow refusal, without allocating or using a hook.
    assert!(store.entries.try_reserve(usize::MAX).is_err());
    assert_eq!(&**store.get(Key::B).unwrap() as *const Owner, pointer);
    assert_eq!(drops.get(), 0);
}
