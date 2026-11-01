//! Exact row-work ledger and old API/canonical field/callback compatibility.
use super::*;
use crate::selection::{BinaryMask, MaskLimits};
use paintcraft_core::types::{policy::hex, text::MaskSpan};
fn ok() -> Result<()> {
    Ok(())
}
fn domain() -> Rect {
    Rect::new(0, 0, 64, 1).unwrap()
}
fn dense(budget: &SharedBudget) -> BinaryMask {
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
    .unwrap()
}
#[test]
fn dense_sixty_four_row_charges_sixty_five_weighted_units_in_addition_to_callbacks() {
    let source = SharedBudget::new(1 << 20);
    let selection = dense(&source);
    assert_eq!(selection.uniform_coverage(), None);
    let row_callbacks = Cell::new(0);
    let mut spans = Vec::new();
    let row_work = selection
        .visit_row(
            0,
            0,
            64,
            1000,
            &|| {
                row_callbacks.set(row_callbacks.get() + 1);
                Ok(())
            },
            |a, b| {
                spans.push((a, b));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(row_work, 65);
    assert_eq!(row_callbacks.get(), 4);
    assert_eq!(spans, [(0, 1)]);
    let budget = SharedBudget::new(1 << 20);
    let callbacks = Cell::new(0u64);
    let (field, work) = A8Mask::from_selection_measured(
        &Selection::Mask(selection),
        domain(),
        &budget,
        Limits::default(),
        &|| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(work - callbacks.get(), 65);
    assert!(work <= Limits::default().work);
    assert_eq!(field.domain(), domain());
    assert_eq!(field.stats().bands, 1);
    assert_eq!(field.stats().runs, 1);
    assert_eq!(field.coverage(0, 0), 255);
    assert_eq!(field.coverage(1, 0), 0);
    assert_eq!(field.coverage(64, 0), 0);
    // Independent SHA of explicit canonical domain/band/run BE records.
    assert_eq!(
        hex(&field.digest()),
        "009dfbc004cc0c532cbc2b1f973d04bbafd727e6018ba6aeaeebd1353cb5c259"
    );
}
#[test]
fn terminal_canonical_builder_and_hash_callbacks_are_in_measured_total() {
    let budget = SharedBudget::new(1 << 20);
    let callbacks = Cell::new(0u64);
    let (mask, work) = A8Mask::from_selection_measured(
        &Selection::Rect(Rect::new(0, 0, 1, 1).unwrap()),
        domain(),
        &budget,
        Limits::default(),
        &|| {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    // Rect capture has no BinaryMask weighted row spend. The final builder/hash
    // uses probe.check directly; its callbacks must NOT be lost from this total.
    assert_eq!(work, callbacks.get());
    assert!(work > 0);
    assert_eq!(mask.coverage(0, 0), 255);
    assert_eq!(mask.coverage(1, 0), 0);
}
#[test]
fn old_and_measured_api_have_identical_field_error_and_callback_cadence() {
    let source = SharedBudget::new(1 << 20);
    let mask = dense(&source);
    for selection in [
        Selection::Inactive,
        Selection::Empty,
        Selection::Rect(Rect::new(0, 0, 1, 1).unwrap()),
        Selection::Mask(mask.clone()),
        Selection::Mask(mask.inverted()),
    ] {
        let budget = SharedBudget::new(1 << 20);
        let a = Cell::new(0);
        let b = Cell::new(0);
        let old = A8Mask::from_selection(&selection, domain(), &budget, Limits::default(), &|| {
            a.set(a.get() + 1);
            Ok(())
        });
        let new = A8Mask::from_selection_measured(
            &selection,
            domain(),
            &budget,
            Limits::default(),
            &|| {
                b.set(b.get() + 1);
                Ok(())
            },
        );
        assert_eq!(a.get(), b.get());
        match (old, new) {
            (Ok(old), Ok((new, _))) => {
                assert_eq!(old, new);
                assert_eq!(old.digest(), new.digest());
            }
            (Err(old), Err(new)) => {
                assert_eq!(old.code, new.code);
                assert_eq!(old.message, new.message);
            }
            _ => panic!("old/measured outcome differs"),
        }
    }
}
#[test]
fn all_callback_cancellations_and_small_limit_denials_release_private_storage() {
    let source = SharedBudget::new(1 << 20);
    let selection = Selection::Mask(dense(&source));
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    let (mask, work) =
        A8Mask::from_selection_measured(&selection, domain(), &budget, Limits::default(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    drop(mask);
    assert_eq!(budget.usage().reserved, 0);
    for stop in 1..=calls.get() {
        let a = Cell::new(0);
        let b = Cell::new(0);
        let cancel = |seen: &Cell<usize>| {
            seen.set(seen.get() + 1);
            if seen.get() == stop {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "measured capture cancelled",
                ))
            } else {
                Ok(())
            }
        };
        let old = A8Mask::from_selection(&selection, domain(), &budget, Limits::default(), &|| {
            cancel(&a)
        })
        .unwrap_err();
        let new = A8Mask::from_selection_measured(
            &selection,
            domain(),
            &budget,
            Limits::default(),
            &|| cancel(&b),
        )
        .unwrap_err();
        assert_eq!(old.code, ErrorCode::OperationCancelled);
        assert_eq!(old.code, new.code);
        assert_eq!(old.message, new.message);
        assert_eq!(a.get(), b.get());
        assert_eq!(budget.usage().reserved, 0);
    }
    for amount in [0, 1, 64, 65, work - 1] {
        let limits = Limits {
            work: amount,
            ..Limits::default()
        };
        assert_eq!(
            A8Mask::from_selection_measured(&selection, domain(), &budget, limits, &ok)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let (mask, used) = A8Mask::from_selection_measured(
        &selection,
        domain(),
        &budget,
        Limits {
            work,
            ..Limits::default()
        },
        &ok,
    )
    .unwrap();
    assert_eq!(used, work);
    drop(mask);
    assert_eq!(budget.usage().reserved, 0);
}
