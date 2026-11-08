use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::cell::Cell;
fn failure<T>(result: Result<T>) -> Error {
    match result {
        Ok(_) => panic!("expected refusal"),
        Err(error) => error,
    }
}
#[test]
fn fixed_exact_empty_and_overwrite_are_all_or_error_and_sticky() {
    for n in [0, 5] {
        let budget = SharedBudget::new(4096);
        let scope = budget.scope(AllocationClass::PaintMetadata);
        let (mut owner, control) = allocate(&scope, n, &|debit| {
            assert_eq!(debit, n);
            assert_eq!(
                scope.footprint().live,
                n + size_of::<CountedSnapshotBytes>() as u64 + control_bytes()
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(owner.bytes.capacity(), n as usize);
        {
            let mut sink = FixedSink {
                owner: &mut owner,
                cursor: 0,
                stopped: None,
                _control: control,
            };
            assert_eq!(sink.write(&[]).unwrap(), 0);
            if n == 5 {
                sink.write_all(&[1, 2, 3]).unwrap();
                assert!(sink.write(&[4, 5, 6]).is_err());
                assert_eq!(sink.cursor, 3);
                assert_eq!(sink.owner.bytes, [1, 2, 3, 0, 0]);
                assert_eq!(sink.stopped, Some(Stop::Length));
                assert!(sink.write(&[]).is_err());
                assert!(sink.flush().is_err());
            }
        }
        assert_eq!(
            scope.footprint().live,
            n + size_of::<CountedSnapshotBytes>() as u64
        );
        drop(owner);
        assert_eq!(budget.usage().reserved, 0);
    }
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let (mut owner, control) = allocate(&scope, 5, &|_| Ok(())).unwrap();
    {
        let mut sink = FixedSink {
            owner: &mut owner,
            cursor: 0,
            stopped: None,
            _control: control,
        };
        sink.write_all(&[1, 2, 3]).unwrap();
        sink.write_all(&[4, 5]).unwrap();
        assert_eq!(sink.cursor, 5);
        assert_eq!(sink.owner.bytes, [1, 2, 3, 4, 5]);
        sink.flush().unwrap();
        sink.complete().unwrap();
    }
    drop(owner);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn fixed_overflow_and_underwrite_are_private_failures() {
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let (mut owner, control) = allocate(&scope, 5, &|_| Ok(())).unwrap();
    {
        let mut sink = FixedSink {
            owner: &mut owner,
            cursor: 0,
            stopped: None,
            _control: control,
        };
        sink.write_all(&[1, 2]).unwrap();
        assert_ne!(sink.cursor, sink.owner.bytes.len());
        assert_eq!(
            sink.complete().unwrap_err().code,
            ErrorCode::InvariantViolation
        );
        // Arithmetic overflow is unreachable with a valid complete writer cursor.
        sink.cursor = usize::MAX;
        assert!(sink.write(&[0]).is_err());
        assert_eq!(sink.stopped, Some(Stop::Overflow));
        assert_eq!(sink.owner.bytes, [1, 2, 0, 0, 0]);
        assert!(sink.flush().is_err());
    }
    drop(owner);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn fixed_controls_exact_minus_one_before_any_allocation_or_initialization() {
    let n = 5;
    let exact = n + size_of::<CountedSnapshotBytes>() as u64 + control_bytes();
    for capacity in [exact - 1, exact] {
        let budget = SharedBudget::new(exact + 65536);
        let reservation = budget.reserve_capacity(capacity).unwrap();
        let scope = reservation.clone().scope(AllocationClass::PaintMetadata);
        let zeroed = Cell::new(0);
        let result = allocate(&scope, n, &|debit| {
            zeroed.set(zeroed.get() + debit);
            Ok(())
        });
        if capacity == exact {
            let (owner, control) = result.unwrap();
            assert_eq!(zeroed.get(), n);
            assert_eq!(reservation.available(), 0);
            assert_eq!(owner.bytes.capacity(), n as usize);
            drop(control);
            drop(owner);
        } else {
            assert_eq!(failure(result).code, ErrorCode::ResourceLimit);
            assert_eq!(zeroed.get(), 0);
        }
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), capacity);
        drop(scope);
        drop(reservation);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn fixed_zeroing_denial_drops_only_pending_controls_and_never_spends_again() {
    let budget = SharedBudget::new(4096);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let expected = Error::new(ErrorCode::OperationCancelled, "zeroing sentinel").at(5);
    let calls = Cell::new(0);
    let result = allocate(&scope, 5, &|n| {
        assert_eq!(n, 5);
        calls.set(calls.get() + 1);
        Err(expected.clone())
    });
    assert_eq!(failure(result), expected);
    assert_eq!(calls.get(), 1);
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
