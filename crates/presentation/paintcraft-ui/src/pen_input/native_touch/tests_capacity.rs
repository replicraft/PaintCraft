use super::*;
#[test]
fn completed_tokens_cannot_be_reused_while_external_batches_or_guards_survive() {
    for guard_only in [false, true] {
        let s = source(128);
        let mut batches = Vec::new();
        let mut guards = Vec::new();
        for life in 0..32 {
            let batch = admitted(submit(&s, finger(life * 2, 1, Phase::Down)));
            if guard_only {
                guards.push(
                    s.native_touch_publication_guard(&batch, view().owner)
                        .unwrap(),
                );
            } else {
                batches.push(batch);
            }
            drop(submit(&s, finger(life * 2 + 1, 1, Phase::Up)));
        }
        for batch in &batches {
            assert!(s.native_touch_batch_is_current(batch, view().owner));
        }
        for guard in &guards {
            assert!(guard.is_current());
        }
        assert!(matches!(
            s.submit_native_finger(
                finger(64, 1, Phase::Down),
                NativeFingerAdmission::Candidate,
                |_| Ok(Some((10., 10.)))
            ),
            Err(Invalid::Capacity)
        ));
        // Capacity is a genuine global acquisition fault, not a scoped retire.
        for batch in &batches {
            assert!(!s.native_touch_batch_is_current(batch, view().owner));
        }
        for guard in &guards {
            assert!(!guard.is_current());
        }
    }
}
#[test]
fn dropping_all_holders_allows_bounded_cell_reuse_without_rebasing_retained_work() {
    let s = source(128);
    let mut batches = Vec::new();
    for life in 0..32 {
        batches.push(admitted(submit(&s, finger(life * 2, 1, Phase::Down))));
        drop(submit(&s, finger(life * 2 + 1, 1, Phase::Up)));
    }
    let retained = batches.remove(0);
    let ordinal = retained.down_ordinal();
    drop(batches);
    for life in 32..96 {
        drop(submit(&s, finger(life * 2, 1, Phase::Down)));
        drop(submit(&s, finger(life * 2 + 1, 1, Phase::Up)));
    }
    assert_eq!(retained.down_ordinal(), ordinal);
    assert!(s.native_touch_batch_is_current(&retained, view().owner));
}
#[test]
fn only_true_lifetime_end_reclaims_clocks_and_held_retirement_is_refused() {
    let s = source(128);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    assert!(
        s.retire_native_touch_identity(
            down.metadata().identity,
            7,
            1,
            Time {
                micros: 101,
                ..down.metadata().time
            }
        )
        .is_err()
    );
    drop(submit(&s, finger(1, 1, Phase::Up)));
    // Preserve handoff clock after Up: a later ordered record with old native
    // time cannot authorize a new contact. It cannot invalidate another device.
    let mut stale = finger(2, 1, Phase::Down);
    stale.time.micros = 99;
    assert!(matches!(
        s.submit_native_finger(stale, NativeFingerAdmission::Candidate, |_| Ok(None)),
        Err(Invalid::Clock)
    ));
    let s = source(128);
    for life in 0..128 {
        let m = finger(life * 3, life, Phase::Down);
        drop(submit(&s, m));
        drop(submit(&s, finger(life * 3 + 1, life, Phase::Up)));
        s.retire_native_touch_identity(
            m.identity,
            7,
            life * 3 + 2,
            Time {
                micros: 102 + life * 3,
                ..m.time
            },
        )
        .unwrap();
    }
    let s = source(128);
    for life in 0..64 {
        drop(submit(&s, finger(life * 3, life, Phase::Down)));
        drop(submit(&s, finger(life * 3 + 1, life, Phase::Up)));
        suppressed(submit(&s, finger(life * 3 + 2, life, Phase::Leave)));
    }
    assert!(matches!(
        s.submit_native_finger(
            finger(192, 64, Phase::Down),
            NativeFingerAdmission::Candidate,
            |_| Ok(Some((10., 10.)))
        ),
        Err(Invalid::Capacity)
    ));
}
#[test]
fn frame_scope_and_metadata_storage_are_admitted_before_callback_allocations() {
    assert!(Source::new_native_touch(64, 64, &SharedBudget::new(1)).is_err());
    let budget = SharedBudget::new(8 << 20);
    let before = budget.usage();
    {
        let s = Source::new_native_touch(2, 2, &budget).unwrap();
        s.publish(view()).unwrap();
        let first = admitted(submit(&s, finger(0, 1, Phase::Down)));
        let second = admitted(submit(&s, finger(1, 1, Phase::Move)));
        assert!(matches!(
            s.submit_native_finger(
                finger(2, 1, Phase::Move),
                NativeFingerAdmission::Candidate,
                |_| Ok(Some((10., 10.)))
            ),
            Err(Invalid::Capacity)
        ));
        assert!(!s.native_touch_batch_is_current(&first, view().owner));
        assert!(!s.native_touch_batch_is_current(&second, view().owner));
    }
    assert_eq!(budget.usage().reserved, before.reserved);
}
#[test]
fn history_is_geometry_only_and_withdrawal_or_terminal_never_authorizes_release() {
    let s = source(64);
    let down = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let mut history = finger(1, 1, Phase::Move);
    history.provenance = Provenance::Coalesced;
    let geometry = admitted(
        s.submit_native_finger(history, NativeFingerAdmission::Candidate, |_| {
            Ok(Some((20., 20.)))
        })
        .unwrap(),
    );
    assert_eq!(geometry.position(), Some((20., 20.)));
    assert_eq!(geometry.down_ordinal(), down.down_ordinal());
    history.ordinal = 2;
    history.time.micros = 102;
    history.confidence = FingerConfidence::Unspecified;
    assert!(
        s.submit_native_finger(history, NativeFingerAdmission::Candidate, |_| panic!(
            "historical withdrawal"
        ))
        .is_err()
    );
    assert!(!s.native_touch_batch_is_current(&down, view().owner));
    // A later high main event cannot hide the withdrawn authority.
    suppressed(submit(&s, finger(3, 1, Phase::Move)));
    suppressed(submit(&s, finger(4, 1, Phase::Up)));
    drop(submit(&s, finger(5, 1, Phase::Down)));
    let s = source(64);
    let batch = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let mut history = finger(1, 1, Phase::Up);
    history.provenance = Provenance::Coalesced;
    assert!(
        s.submit_native_finger(history, NativeFingerAdmission::Candidate, |_| panic!(
            "historical terminal"
        ))
        .is_err()
    );
    assert!(!s.native_touch_batch_is_current(&batch, view().owner));
    suppressed(submit(&s, finger(2, 1, Phase::Down)));
    suppressed(submit(&s, finger(3, 1, Phase::Up)));
    drop(admitted(submit(&s, finger(4, 1, Phase::Down))));
}
#[test]
fn token_admission_outlives_source_until_batch_guard_or_cancel_holder_is_gone() {
    for kind in 0..3 {
        let budget = SharedBudget::new(8 << 20);
        let s = Source::new_native_touch(64, 64, &budget).unwrap();
        s.publish(view()).unwrap();
        let batch = admitted(submit(&s, finger(0, 1, Phase::Down)));
        let guard = s
            .native_touch_publication_guard(&batch, view().owner)
            .unwrap();
        let cancel = if kind == 2 {
            s.retire_native_touches()
                .unwrap()
                .into_iter()
                .flatten()
                .next()
        } else {
            None
        };
        let batch = if kind == 0 {
            Some(batch)
        } else {
            drop(batch);
            None
        };
        let guard = if kind == 1 {
            Some(guard)
        } else {
            drop(guard);
            None
        };
        drop(s);
        assert!(
            budget.usage().reserved > 0,
            "cell admission follows retained holder"
        );
        if let Some(guard) = &guard {
            assert!(!guard.is_current());
        }
        drop(batch);
        drop(guard);
        drop(cancel);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[path = "tests_continuation.rs"]
mod continuation;
#[test]
fn default_source_keeps_native_bank_absent_and_native_methods_explicitly_refuse() {
    let budget = SharedBudget::new(8 << 20);
    let default = Source::new(64, 64, &budget).unwrap();
    default.publish(view()).unwrap();
    let default_quote = budget.usage().reserved;
    assert!(matches!(
        default.submit_native_finger(
            finger(0, 1, Phase::Down),
            NativeFingerAdmission::Candidate,
            |_| panic!("unconfigured")
        ),
        Err(Invalid::Owner)
    ));
    assert!(default.retire_native_touches().is_err());
    assert!(default.mouse_capture(true));
    assert!(default.mouse_capture(false));
    assert_eq!(pen(&default, 0, Phase::Down), Route::Canvas);
    drop(default);
    assert_eq!(budget.usage().reserved, 0);
    let configured = Source::new_native_touch(64, 64, &budget).unwrap();
    assert!(budget.usage().reserved > default_quote);
    drop(configured);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn retained_cell_quote_covers_its_actual_payload_and_logical_arc_counters() {
    let s = source(64);
    let batch = admitted(submit(&s, finger(0, 1, Phase::Down)));
    let cell = &batch.binding.native_touch.as_ref().unwrap().valid;
    assert!(
        cell._storage.bytes()
            >= (std::mem::size_of::<Cell>() + 2 * std::mem::size_of::<usize>()) as u64
    );
    // The much larger pinned View stays in the admitted resident Slot table,
    // not in escaped cells. Its allocation is freed when Source retires.
    assert!(std::mem::size_of::<Cell>() < std::mem::size_of::<View>());
}
