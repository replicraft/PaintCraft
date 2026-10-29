use super::*;
use paintcraft_input::tablet::DeviceKind;
fn key(begin: u64, target: Target) -> Key {
    Key {
        installation: 7,
        identity: Identity {
            device: 1,
            pointer: 9,
            kind: DeviceKind::Pen,
        },
        begin,
        target,
    }
}
#[test]
fn reserved_namespace_and_ordinal_are_refused_but_source_zero_is_valid() {
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    for installation in [0, u64::MAX] {
        assert!(matches!(
            bank.begin(Key {
                installation,
                ..key(0, Target::Ink)
            }),
            Err(Invalid::Owner)
        ));
    }
    assert!(matches!(
        bank.begin(key(u64::MAX, Target::Ink)),
        Err(Invalid::Owner)
    ));
    assert!(bank.begin(key(0, Target::Ink)).is_ok());
}
#[test]
fn completed_ink_survives_active_chord_and_new_tip_retirement() {
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    let old = bank.begin(key(1, Target::Ink)).unwrap();
    assert!(bank.complete(&old));
    let ui = bank.begin(key(2, Target::Ui)).unwrap();
    let next = bank.begin(key(3, Target::Ink)).unwrap();
    let cancellations = bank.retire_active();
    assert!(old.current());
    assert!(!ui.current());
    assert!(!next.current());
    assert!(!cancellations.iter().flatten().any(|c| c.matches(&old)));
    assert!(cancellations.iter().flatten().any(|c| c.matches(&ui)));
    assert!(cancellations.iter().flatten().any(|c| c.matches(&next)));
    assert!(bank.retire_active().iter().all(Option::is_none));
}
#[test]
fn exact_cell_match_refuses_identical_visible_keys_from_another_bank() {
    let budget = SharedBudget::new(1 << 20);
    let mut first = Bank::new(&budget).unwrap();
    let mut second = Bank::new(&budget).unwrap();
    let a = first.begin(key(1, Target::Ink)).unwrap();
    let b = second.begin(key(1, Target::Ink)).unwrap();
    let cancellations = first.retire_active();
    let cancellation = cancellations.iter().flatten().next().unwrap();
    assert!(cancellation.matches(&a));
    assert!(!cancellation.matches(&b));
    assert!(b.current());
}
#[test]
fn mixed_batch_requires_every_member_but_separate_completed_batch_survives() {
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    let complete = bank.begin(key(1, Target::Ink)).unwrap();
    bank.complete(&complete);
    let held = bank.begin(key(2, Target::Ink)).unwrap();
    let mut mixed = Leases::new();
    mixed.insert(&complete).unwrap();
    mixed.insert(&held).unwrap();
    let mut earlier = Leases::new();
    earlier.insert(&complete).unwrap();
    assert!(mixed.current());
    bank.retire_active();
    assert!(!mixed.current());
    assert!(earlier.current());
}
#[test]
fn retained_completed_leases_prevent_reuse_without_invalidating_old_authority() {
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    let mut leases = Leases::new();
    for ordinal in 0..LIMIT as u64 {
        let lease = bank.begin(key(ordinal, Target::Ink)).unwrap();
        bank.complete(&lease);
        leases.insert(&lease).unwrap();
        // Repeating a member does not consume another slot.
        leases.insert(&lease).unwrap();
    }
    assert!(matches!(
        bank.begin(key(100, Target::Ink)),
        Err(Invalid::Capacity)
    ));
    assert!(leases.current());
    drop(leases);
    for ordinal in 100..200 {
        let lease = bank.begin(key(ordinal, Target::Ink)).unwrap();
        assert!(bank.complete(&lease));
    }
}
#[test]
fn admission_precedes_cells_and_accounts_survive_through_last_holder() {
    assert!(Bank::new(&SharedBudget::new((LIMIT * CELL_BYTES - 1) as u64)).is_err());
    assert!(CELL_BYTES >= std::mem::size_of::<Cell>() + 2 * std::mem::size_of::<usize>());
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    let lease = bank.begin(key(1, Target::Ink)).unwrap();
    let cancellations = bank.retire_active();
    drop(bank);
    assert!(!lease.current());
    assert_eq!(budget.usage().reserved, CELL_BYTES as u64);
    drop(lease);
    assert_eq!(budget.usage().reserved, CELL_BYTES as u64);
    drop(cancellations);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn continuation_rebases_only_included_token_context_not_other_live_contacts() {
    let budget = SharedBudget::new(1 << 20);
    let mut bank = Bank::new(&budget).unwrap();
    let before = crate::pen_input::tests::view().owner;
    let after = Owner {
        state: before.state + 1,
        publication: before.publication + 1,
        ..before
    };
    let earlier = bank.begin(key(0, Target::Ink)).unwrap();
    let later = bank.begin(key(2, Target::Ink)).unwrap();
    bank.bind_owner(&earlier, before);
    bank.bind_owner(&later, before);
    let mut early = Leases::new();
    early.insert(&earlier).unwrap();
    let mut continued = Leases::new();
    continued.insert(&later).unwrap();
    assert!(bank.context_matches(&early, before));
    assert!(bank.follows(&continued, 1));
    assert!(!bank.follows(&early, 1));
    bank.acknowledge(&continued, after);
    assert!(bank.context_matches(&continued, after));
    assert!(!bank.context_matches(&early, after));
    assert!(bank.context_matches(&early, before));
}
