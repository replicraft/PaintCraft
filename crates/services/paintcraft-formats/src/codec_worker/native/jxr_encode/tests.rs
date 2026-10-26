use super::*;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
use std::cell::Cell;
fn owner() -> Owner {
    Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    }
}
#[test]
fn source_owner_cancellation_deadline_and_admission_refusals_precede_discovery() {
    for case in 0..7 {
        let mut expected = owner();
        if case == 1 {
            expected.frontend = 0;
        }
        let current = Cell::new(if case == 2 { owner() } else { expected });
        let layout = wire::Layout::new(2, 1, 1024).unwrap();
        let quote = layout.reservation_bytes().unwrap();
        let budget = SharedBudget::new(quote);
        let lease = ResourceLease::new(budget.reserve(quote).unwrap());
        let job = JxrEncodeJob {
            layout,
            owner: expected,
            deadline: if case == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs(5)
            },
        };
        let error = job
            .prepare(
                Arc::new(vec![0; if case == 3 { 5 } else { 6 }]),
                lease,
                &|| {
                    if case == 4 {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(current.get()),
                |actual, profile| {
                    assert_eq!(actual, layout);
                    assert_eq!(profile, wire::PROFILE);
                    if case == 6 {
                        current.set(owner());
                        Ok(())
                    } else {
                        Err(unsupported("private admission refused"))
                    }
                },
                |_, _| panic!("invalid job must not launch"),
            )
            .unwrap_err();
        assert_eq!(
            error.code,
            match case {
                0 => ErrorCode::InputSchema,
                1 | 5 => ErrorCode::CapabilityUnsupported,
                2 | 6 => ErrorCode::ConflictStaleBase,
                3 => ErrorCode::CodecFailed,
                4 => ErrorCode::OperationCancelled,
                _ => unreachable!(),
            }
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
