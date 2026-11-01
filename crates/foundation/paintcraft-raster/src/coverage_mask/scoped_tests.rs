//! Supplied-scope ownership, old API compatibility and all callback rollback.
use super::*;
use crate::selection::{BinaryMask, MaskLimits, Selection};
use paintcraft_core::types::{policy::hex, text::MaskSpan};
fn ok() -> Result<()> {
    Ok(())
}
fn domain() -> Rect {
    Rect::new(0, 0, 64, 1).unwrap()
}
fn dense(budget: &SharedBudget) -> Selection {
    Selection::Mask(
        BinaryMask::from_spans(
            domain(),
            false,
            &[MaskSpan {
                y: 0,
                x0: 0,
                x1: 0,
                coverage: 255,
            }],
            budget,
            MaskLimits::default(),
            &ok,
        )
        .unwrap(),
    )
}
fn sample(budget: &SharedBudget) -> A8Mask {
    let runs = [
        Run {
            x0: -2,
            x1: 0,
            coverage: 1,
        },
        Run {
            x0: 0,
            x1: 2,
            coverage: 127,
        },
        Run {
            x0: 3,
            x1: 5,
            coverage: 255,
        },
    ];
    A8Mask::from_bands(
        Rect::new(-3, -2, 8, 6).unwrap(),
        &[Band {
            y0: -1,
            y1: 1,
            runs: &runs,
        }],
        budget,
        Limits::default(),
        &ok,
    )
    .unwrap()
}
/// Explicit visible-domain/band/run bytes, not a new persistence wire format.
fn visible_bytes(mask: &A8Mask) -> Vec<u8> {
    let mut bytes = Vec::new();
    let domain = mask.domain();
    for n in [domain.x0, domain.y0, domain.x1, domain.y1] {
        bytes.extend_from_slice(&n.to_be_bytes());
    }
    mask.visit_bands_checked(u64::MAX, &ok, |event| {
        match event {
            Visit::Band { y0, y1, runs } => {
                bytes.push(1);
                bytes.extend_from_slice(&y0.to_be_bytes());
                bytes.extend_from_slice(&y1.to_be_bytes());
                bytes.extend_from_slice(&(runs as u64).to_be_bytes());
            }
            Visit::Run(run) => {
                bytes.push(2);
                bytes.extend_from_slice(&run.x0.to_be_bytes());
                bytes.extend_from_slice(&run.x1.to_be_bytes());
                bytes.push(run.coverage);
            }
        }
        Ok(())
    })
    .unwrap();
    bytes
}
#[test]
fn capture_scope_matches_budget_bytes_digest_callback_and_complete_weighted_work() {
    let source = SharedBudget::new(1 << 20);
    let dense = dense(&source);
    let Selection::Mask(binary) = &dense else {
        unreachable!()
    };
    let selections = [
        Selection::Inactive,
        Selection::Empty,
        Selection::Rect(Rect::new(-1, 0, 3, 1).unwrap()),
        Selection::Mask(BinaryMask::uniform(domain(), true).unwrap()),
        dense.clone(),
        Selection::Mask(binary.inverted()),
    ];
    let budget = SharedBudget::new(2 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    for selection in selections {
        let a = Cell::new(0u64);
        let b = Cell::new(0u64);
        let legacy = SharedBudget::new(1 << 20);
        let old = A8Mask::from_selection_measured(
            &selection,
            domain(),
            &legacy,
            Limits::default(),
            &|| {
                a.set(a.get() + 1);
                Ok(())
            },
        );
        let new = A8Mask::from_selection_measured_in_scope(
            &selection,
            domain(),
            &scope,
            Limits::default(),
            &|| {
                b.set(b.get() + 1);
                Ok(())
            },
        );
        assert_eq!(a.get(), b.get());
        match (old, new) {
            (Ok((old, old_work)), Ok((new, new_work))) => {
                assert_eq!(old_work, new_work);
                assert_eq!(old.digest(), new.digest());
                assert_eq!(visible_bytes(&old), visible_bytes(&new));
                assert_eq!(
                    scope.footprint().live,
                    new.storage_usage().map_or(0, |(_, n)| n)
                );
                drop(new);
            }
            (Err(old), Err(new)) => {
                assert_eq!(old.code, new.code);
                assert_eq!(old.message, new.message);
            }
            _ => panic!("scoped capture changed result"),
        }
        assert_eq!(scope.footprint().live, 0);
    }
    let callbacks = Cell::new(0u64);
    let (mask, work) = A8Mask::from_selection_measured_in_scope(
        &dense,
        domain(),
        &scope,
        Limits::default(),
        &|| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(work - callbacks.get(), 65);
    assert_eq!(
        hex(&mask.digest()),
        "009dfbc004cc0c532cbc2b1f973d04bbafd727e6018ba6aeaeebd1353cb5c259"
    );
    let field =
        A8Mask::from_selection_in_scope(&dense, domain(), &scope, Limits::default(), &ok).unwrap();
    assert_eq!(field.digest(), mask.digest());
    drop(field);
    drop(mask);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn scoped_invert_matches_old_bytes_callbacks_fractional_literals_and_double_invert() {
    let source = SharedBudget::new(1 << 20);
    let mask = sample(&source);
    let budget = SharedBudget::new(2 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let a = Cell::new(0);
    let b = Cell::new(0);
    let old = mask
        .inverted_checked(&SharedBudget::new(1 << 20), Limits::default(), &|| {
            a.set(a.get() + 1);
            Ok(())
        })
        .unwrap();
    let new = mask
        .inverted_in_scope_checked(&scope, Limits::default(), &|| {
            b.set(b.get() + 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(a.get(), b.get());
    assert_eq!(old.digest(), new.digest());
    assert_eq!(visible_bytes(&old), visible_bytes(&new));
    for (x, y, value) in [
        (-3, -2, 255),
        (-2, -1, 254),
        (0, -1, 128),
        (3, -1, 0),
        (-4, -1, 0),
    ] {
        assert_eq!(new.coverage(x, y), value);
    }
    let double = new
        .inverted_in_scope_checked(&scope, Limits::default(), &ok)
        .unwrap();
    assert_eq!(double.digest(), mask.digest());
    assert_eq!(visible_bytes(&double), visible_bytes(&mask));
    drop(double);
    drop(new);
    assert_eq!(scope.footprint().live, 0);
    assert!(source.usage().reserved > 0); // immutable input remains independently owned
}
#[test]
fn reserved_scope_does_not_borrow_global_headroom_even_for_final_builder_peak() {
    let source = SharedBudget::new(1 << 20);
    let selection = dense(&source);
    let mask = sample(&source);
    for invert in [false, true] {
        let measured = SharedBudget::new(1 << 20);
        let measuring = measured.scope(AllocationClass::CoverageMasks);
        let result = if invert {
            mask.inverted_in_scope_checked(&measuring, Limits::default(), &ok)
                .unwrap()
        } else {
            A8Mask::from_selection_in_scope(
                &selection,
                domain(),
                &measuring,
                Limits::default(),
                &ok,
            )
            .unwrap()
        };
        let peak = measuring.footprint().peak;
        assert!(peak > 1);
        drop(result);
        assert_eq!(measuring.footprint().live, 0);
        let global = SharedBudget::new(8 << 20);
        let scope = global
            .reserve_capacity(peak - 1)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let failed = if invert {
            mask.inverted_in_scope_checked(&scope, Limits::default(), &ok)
        } else {
            A8Mask::from_selection_in_scope(&selection, domain(), &scope, Limits::default(), &ok)
        };
        let error = failed.unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(error.allocation_denial().is_some());
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(global.usage().reserved, peak - 1);
        assert!(scope.footprint().peak >= peak);
        let mut check = scope.account();
        check.resize(peak - 1).unwrap();
        assert_eq!(scope.footprint().live, peak - 1);
        drop(check);
        drop(scope);
        assert_eq!(global.usage().reserved, 0);
    }
}
#[test]
fn every_capture_and_invert_callback_cancellation_releases_scoped_private_storage() {
    let input = SharedBudget::new(1 << 20);
    let selection = dense(&input);
    let mask = sample(&input);
    let source_bytes = input.usage().reserved;
    for invert in [false, true] {
        let global = SharedBudget::new(1 << 20);
        let scope = global
            .reserve_capacity(65536)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let calls = Cell::new(0usize);
        let count = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let field = if invert {
            mask.inverted_in_scope_checked(&scope, Limits::default(), &count)
                .unwrap()
        } else {
            A8Mask::from_selection_in_scope(&selection, domain(), &scope, Limits::default(), &count)
                .unwrap()
        };
        drop(field);
        assert_eq!(scope.footprint().live, 0);
        for stop in 1..=calls.get() {
            let seen = Cell::new(0usize);
            let cancelled = || {
                seen.set(seen.get() + 1);
                if seen.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "scoped A8 cancelled",
                    ))
                } else {
                    Ok(())
                }
            };
            let result = if invert {
                mask.inverted_in_scope_checked(&scope, Limits::default(), &cancelled)
            } else {
                A8Mask::from_selection_in_scope(
                    &selection,
                    domain(),
                    &scope,
                    Limits::default(),
                    &cancelled,
                )
            };
            assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
            assert_eq!(seen.get(), stop);
            assert_eq!(scope.footprint().live, 0);
            assert_eq!(global.usage().reserved, 65536);
            assert_eq!(input.usage().reserved, source_bytes);
        }
        drop(scope);
        assert_eq!(global.usage().reserved, 0);
    }
}
#[test]
fn work_and_schema_errors_match_legacy_without_releasing_or_mutating_input() {
    let source = SharedBudget::new(1 << 20);
    let selection = dense(&source);
    let mask = sample(&source);
    let before = visible_bytes(&mask);
    let source_bytes = source.usage().reserved;
    let global = SharedBudget::new(1 << 20);
    let scope = global
        .reserve_capacity(65536)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    for work in [0, 1, 64, 65] {
        let limits = Limits {
            work,
            ..Limits::default()
        };
        let old = A8Mask::from_selection_measured(
            &selection,
            domain(),
            &SharedBudget::new(1 << 20),
            limits,
            &ok,
        )
        .unwrap_err();
        let new =
            A8Mask::from_selection_measured_in_scope(&selection, domain(), &scope, limits, &ok)
                .unwrap_err();
        assert_eq!(old.code, new.code);
        assert_eq!(old.message, new.message);
        let old = mask.inverted_checked(&SharedBudget::new(1 << 20), limits, &ok);
        let new = mask.inverted_in_scope_checked(&scope, limits, &ok);
        match (old, new) {
            (Ok(old), Ok(new)) => assert_eq!(visible_bytes(&old), visible_bytes(&new)),
            (Err(old), Err(new)) => {
                assert_eq!(old.code, new.code);
                assert_eq!(old.message, new.message);
            }
            _ => panic!("scoped invert limit outcome changed"),
        }
        assert_eq!(scope.footprint().live, 0);
    }
    assert_eq!(
        A8Mask::from_selection_in_scope(
            &Selection::Inactive,
            domain(),
            &scope,
            Limits::default(),
            &ok
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    assert_eq!(visible_bytes(&mask), before);
    assert_eq!(source.usage().reserved, source_bytes);
}
#[test]
fn empty_results_need_no_capacity_and_shared_result_lives_until_last_payload_owner() {
    let source = SharedBudget::new(1 << 20);
    let full = A8Mask::uniform(domain(), 255, &source, Limits::default(), &ok).unwrap();
    let global = SharedBudget::new(1 << 20);
    let zero = global
        .reserve_capacity(0)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let empty = full
        .inverted_in_scope_checked(&zero, Limits::default(), &ok)
        .unwrap();
    assert_eq!(empty.storage_usage(), None);
    assert_eq!(zero.footprint().live, 0);
    let (captured, work) = A8Mask::from_selection_measured_in_scope(
        &Selection::Empty,
        domain(),
        &zero,
        Limits::default(),
        &ok,
    )
    .unwrap();
    assert!(work > 0);
    assert_eq!(captured.storage_usage(), None);
    let scope = global
        .reserve_capacity(65536)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let result = A8Mask::from_selection_in_scope(
        &Selection::Rect(domain()),
        domain(),
        &scope,
        Limits::default(),
        &ok,
    )
    .unwrap();
    let (_, bytes) = result.storage_usage().unwrap();
    assert_eq!(scope.footprint().live, bytes);
    let alias = result.clone();
    drop(result);
    assert_eq!(scope.footprint().live, bytes);
    let mut blocker = scope.account();
    blocker.resize(65536 - bytes).unwrap();
    assert_eq!(
        scope.account().resize(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(alias);
    assert_eq!(scope.footprint().live, 65536 - bytes);
    drop(blocker);
    assert_eq!(scope.footprint().live, 0);
    drop(scope);
    drop(zero);
    assert_eq!(global.usage().reserved, 0);
    let scope = global
        .reserve_capacity(65536)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let result = A8Mask::from_selection_in_scope(
        &Selection::Rect(domain()),
        domain(),
        &scope,
        Limits::default(),
        &ok,
    )
    .unwrap();
    let (_, bytes) = result.storage_usage().unwrap();
    drop(scope); // unused reservation returns; retained payload still owns its charge
    assert_eq!(global.usage().reserved, bytes);
    drop(result);
    assert_eq!(global.usage().reserved, 0);
}
