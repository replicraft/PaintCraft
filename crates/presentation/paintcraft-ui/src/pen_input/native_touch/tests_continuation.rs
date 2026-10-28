use super::*;
fn proof() -> Predecessor {
    let v = view();
    Predecessor {
        before: v.owner,
        after: Owner {
            state: 20,
            publication: 2,
            ..v.owner
        },
        sequence: 1,
        transform: v.transform,
        canvas: v.canvas,
        pixels_per_point: v.pixels_per_point,
        focused: true,
        modal: false,
        ime: false,
    }
}
fn pending(s: &Source) -> (Batch, NativeTouchBatch) {
    assert_eq!(pen(s, 2, Phase::Down), Route::Canvas);
    assert_eq!(pen(s, 3, Phase::Up), Route::Canvas);
    let p = s.take(1, view().owner, false).unwrap();
    let t = admitted(submit(s, finger(4, 1, Phase::Down)));
    drop(submit(s, finger(5, 1, Phase::Up)));
    (p, t)
}
#[test]
fn mixed_opaque_continuation_is_atomic_and_requires_every_actual_lifetime() {
    let s = source(64);
    let (mut p, mut t) = pending(&s);
    let proof = proof();
    s.acknowledge_continuations(
        &mut [Continuation::Pen(&mut p), Continuation::NativeTouch(&mut t)],
        proof,
    )
    .unwrap();
    assert!(s.batch_is_current(&p, proof.after));
    assert!(s.native_touch_batch_is_current(&t, proof.after));
    assert!(
        s.native_touch_publication_guard(&t, proof.after)
            .unwrap()
            .is_current()
    );
    assert!(
        s.acknowledge_continuations(
            &mut [Continuation::Pen(&mut p), Continuation::NativeTouch(&mut t)],
            proof
        )
        .is_err()
    );
    for foreign in [false, true] {
        let a = source(64);
        let b = source(64);
        let (mut pen, mut touch) = pending(&a);
        if foreign {
            touch = admitted(submit(&b, finger(4, 1, Phase::Down)));
        } else {
            a.retire_native_touches().unwrap();
        }
        assert!(
            a.acknowledge_continuations(
                &mut [
                    Continuation::Pen(&mut pen),
                    Continuation::NativeTouch(&mut touch)
                ],
                proof
            )
            .is_err()
        );
        assert!(a.batch_is_current(&pen, proof.before));
        assert!(!a.batch_is_current(&pen, proof.after));
        if foreign {
            assert!(b.native_touch_batch_is_current(&touch, proof.before));
        }
    }
}
#[test]
fn empty_and_head_ticket_only_rebase_cannot_mutate_source() {
    let s = source(64);
    assert!(s.acknowledge_continuations(&mut [], proof()).is_err());
    let mut touch = admitted(
        s.submit_native_finger(
            finger(4, 1, Phase::Down),
            NativeFingerAdmission::Candidate,
            |_| Ok(Some((200., 200.))),
        )
        .unwrap(),
    );
    let mut head = view();
    head.owner = Owner {
        state: 99,
        publication: 2,
        ..head.owner
    };
    s.publish(head).unwrap();
    assert!(s.native_touch_batch_is_current(&touch, head.owner));
    assert!(
        s.acknowledge_continuations(&mut [Continuation::NativeTouch(&mut touch)], proof())
            .is_err()
    );
    assert!(s.ui_publication_guard(head.owner).unwrap().is_current());
}
#[test]
fn only_exact_acknowledged_down_lifetimes_gain_the_new_publication_context() {
    for completed in [false, true] {
        let s = source(64);
        let old = admitted(submit(&s, finger(0, 9, Phase::Down)));
        if completed {
            drop(submit(&s, finger(1, 9, Phase::Up)));
        }
        let mut later = admitted(submit(&s, finger(4, 1, Phase::Down)));
        let proof = proof();
        s.acknowledge_continuations(&mut [Continuation::NativeTouch(&mut later)], proof)
            .unwrap();
        assert!(!s.native_touch_batch_is_current(&old, proof.after));
        assert!(
            s.native_touch_publication_guard(&old, proof.after)
                .is_none()
        );
        let outcome = s
            .submit_native_finger(
                finger(5, 9, Phase::Move),
                NativeFingerAdmission::Candidate,
                |_| panic!("unacknowledged contact cannot borrow new view"),
            )
            .unwrap();
        if completed {
            suppressed(outcome);
        } else {
            let NativeTouchOutcome::Cancelled(c) = outcome else {
                panic!("stale private gesture cancellation")
            };
            assert!(c.matches_batch(&old));
        }
        let next = admitted(submit(&s, finger(6, 1, Phase::Move)));
        assert!(s.native_touch_batch_is_current(&next, proof.after));
        assert!(
            s.native_touch_publication_guard(&next, proof.after)
                .unwrap()
                .is_current()
        );
    }
}
