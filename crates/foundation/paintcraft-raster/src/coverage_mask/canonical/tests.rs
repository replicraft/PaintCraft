use super::*;
use paintcraft_core::types::{Error, ErrorCode, json};
use std::cell::{Cell, RefCell};
type Input = Vec<(i64, i64, Vec<Run>)>;
fn domain() -> Rect {
    Rect::from_origin_size(0, 0, 260, 4).unwrap()
}
fn one() -> Input {
    vec![(
        0,
        4,
        vec![Run {
            x0: 0,
            x1: 260,
            coverage: 128,
        }],
    )]
}
fn config(input: &Input) -> CanonicalA8Config {
    CanonicalA8Config {
        domain: domain(),
        bands: input.len(),
        runs: input.iter().map(|r| r.2.len()).sum(),
        limits: Limits::default(),
    }
}
fn build(
    input: &Input,
    scope: &BudgetScope,
    m: &BudgetAccount,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<A8Mask> {
    let mut b = CanonicalA8Builder::new(config(input), scope, m, check, spend)?;
    for (y0, y1, runs) in input {
        b = b.band(*y0, *y1, runs.len())?;
        for run in runs {
            b = b.run(*run)?;
        }
    }
    b.finish()
}
fn fixtures() -> Vec<json::Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/canonical-a8-fixtures/oracle.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn expected(row: &json::Value) -> &json::Value {
    row.get("targets")
        .unwrap()
        .get(if cfg!(target_pointer_width = "64") {
            "native64"
        } else {
            "wasm32"
        })
        .unwrap()
}
fn number(row: &json::Value, key: &str) -> u64 {
    row.get(key).unwrap().as_i64().unwrap() as u64
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn guard(scope: &BudgetScope) -> BudgetAccount {
    let mut m = scope.account();
    m.resize(512).unwrap();
    m
}
#[test]
fn independent_event_vectors_hash_bytes_and_old_unviewed_stats_match() {
    let mut inputs = vec![
        vec![],
        one(),
        vec![
            (
                0,
                2,
                vec![Run {
                    x0: 0,
                    x1: 260,
                    coverage: 128,
                }],
            ),
            (
                2,
                4,
                vec![Run {
                    x0: 0,
                    x1: 260,
                    coverage: 255,
                }],
            ),
        ],
    ];
    for n in [128, 129] {
        inputs.push(vec![(
            0,
            4,
            (0..n)
                .map(|i| Run {
                    x0: 2 * i,
                    x1: 2 * i + 1,
                    coverage: 1 + (i % 255) as u8,
                })
                .collect(),
        )]);
    }
    for (input, row) in inputs.into_iter().zip(fixtures()) {
        let budget = SharedBudget::new(4 << 20);
        let scope = budget.scope(AllocationClass::CoverageMasks);
        let m = guard(&budget.scope(AllocationClass::PaintMetadata));
        let trace = RefCell::new(Vec::new());
        let checks = Cell::new(0u64);
        let mask = build(
            &input,
            &scope,
            &m,
            &|| {
                checks.set(checks.get() + 1);
                Ok(())
            },
            &|n| {
                trace.borrow_mut().push(n);
                Ok(())
            },
        )
        .unwrap();
        let target = expected(&row);
        let event: Vec<u64> = target
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|e| number(e, "debit"))
            .collect();
        assert_eq!(*trace.borrow(), event);
        assert_eq!(event.iter().sum::<u64>(), number(target, "total_work"));
        assert_eq!(checks.get(), number(target, "checks"));
        assert_eq!(
            hex(&mask.digest()),
            row.get("digest_sha256").unwrap().as_str().unwrap()
        );
        let bands: Vec<_> = input
            .iter()
            .map(|(y0, y1, runs)| Band {
                y0: *y0,
                y1: *y1,
                runs,
            })
            .collect();
        let old =
            A8Mask::from_bands_in_scope(domain(), &bands, &scope, Limits::default(), &|| Ok(()))
                .unwrap();
        assert_eq!(mask.digest(), old.digest());
        assert_eq!(mask.stats(), old.stats());
        assert_eq!(mask, old);
        assert_eq!(
            mask.stats(),
            Stats {
                bands: input.len(),
                runs: input.iter().map(|r| r.2.len()).sum()
            }
        );
        assert_eq!(mask.storage.is_some(), !input.is_empty());
        drop(old);
        drop(mask);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn defining_layout_and_fixed_slot_do_not_become_observed_grants() {
    assert_eq!(size_of::<Pending>(), 48);
    assert_eq!(size_of::<Run>(), 24);
    assert_eq!(
        size_of::<Rows>(),
        if cfg!(target_pointer_width = "64") {
            32
        } else {
            24
        }
    );
    assert_eq!(
        canonical_a8_storage_arc_quote().unwrap(),
        if cfg!(target_pointer_width = "64") {
            112
        } else {
            64
        }
    );
    for cap in [
        4095,
        4096,
        4096 + retained_quote(1, 1).unwrap() - 1,
        4096 + retained_quote(1, 1).unwrap(),
    ] {
        let budget = SharedBudget::new(4 << 20);
        let reservation = budget.reserve_capacity(cap).unwrap();
        let scope = reservation.clone().scope(AllocationClass::CoverageMasks);
        let m = guard(&budget.scope(AllocationClass::PaintMetadata));
        let paid = Cell::new(0u64);
        let result = build(&one(), &scope, &m, &|| Ok(()), &|n| {
            paid.set(paid.get() + n);
            Ok(())
        });
        if cap == 4096 + retained_quote(1, 1).unwrap() {
            drop(result.unwrap());
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        if cap == 4095 {
            assert_eq!(paid.get(), 64);
        }
        assert_eq!(scope.footprint().live, 0);
        assert_eq!(reservation.available(), cap);
    }
}
#[test]
fn full_storage_guard_short_refuses_before_buffers_and_never_changes_realm() {
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::CoverageMasks);
    let mut m = scope.account();
    m.resize(canonical_a8_storage_arc_quote().unwrap() - 1)
        .unwrap();
    let original = m.bytes();
    let error = build(&one(), &scope, &m, &|| Ok(()), &|_| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(scope.footprint().live, original);
}
#[test]
fn each_actual_callback_cancel_preserves_first_typed_error_and_drops_before_credit() {
    let total = 7;
    for stop in 1..=total {
        let budget = SharedBudget::new(1 << 20);
        let scope = budget.scope(AllocationClass::CoverageMasks);
        let m = guard(&budget.scope(AllocationClass::PaintMetadata));
        let checks = Cell::new(0);
        let sentinel = Error::new(ErrorCode::OperationCancelled, "canonical stream cut").at(61);
        let result = build(
            &one(),
            &scope,
            &m,
            &|| {
                checks.set(checks.get() + 1);
                if checks.get() == stop {
                    Err(sentinel.clone())
                } else {
                    Ok(())
                }
            },
            &|_| Ok(()),
        );
        assert_eq!(result.unwrap_err(), sentinel);
        assert_eq!(checks.get(), stop);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn every_debit_refusal_and_same_counter_retry_retains_paid_prefix() {
    let row = fixtures().remove(1);
    let events: Vec<u64> = expected(&row)
        .get("events")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| number(e, "debit"))
        .collect();
    for stop in 0..events.len() {
        let budget = SharedBudget::new(1 << 20);
        let scope = budget.scope(AllocationClass::CoverageMasks);
        let m = guard(&budget.scope(AllocationClass::PaintMetadata));
        let ordinal = Cell::new(0);
        let paid = Cell::new(0u64);
        let refusing = Cell::new(true);
        let sentinel = Error::new(ErrorCode::ResourceLimit, "exact debit cut").at(62);
        let spend = |n| {
            let i = ordinal.get();
            ordinal.set(i + 1);
            if refusing.get() && i == stop {
                return Err(sentinel.clone());
            }
            paid.set(paid.get() + n);
            Ok(())
        };
        assert_eq!(
            build(&one(), &scope, &m, &|| Ok(()), &spend).unwrap_err(),
            sentinel
        );
        let prefix = events[..stop].iter().sum::<u64>();
        assert_eq!(paid.get(), prefix);
        assert_eq!(scope.footprint().live, 0);
        refusing.set(false);
        drop(build(&one(), &scope, &m, &|| Ok(()), &spend).unwrap());
        assert_eq!(paid.get(), prefix + number(expected(&row), "total_work"));
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn strict_canonical_geometry_counts_zero_and_adjacency_refuse() {
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::CoverageMasks);
    let m = guard(&budget.scope(AllocationClass::PaintMetadata));
    let check = || Ok(());
    let spend = |_| Ok(());
    for which in 0..12 {
        let mut c = config(&one());
        if which == 0 {
            c.domain.x1 = c.domain.x0;
        }
        if which == 1 {
            c.runs = 0;
        }
        if which == 2 {
            c.bands = 0;
        }
        if which >= 7 {
            c.bands = 2;
            c.runs = 2;
        }
        let result = (|| {
            let b = CanonicalA8Builder::new(c, &scope, &m, &check, &spend)?;
            match which {
                0..=2 => b.finish(),
                3 => b.band(0, 4, 0)?.finish(),
                4 => b.band(-1, 4, 1)?.finish(),
                5 => b
                    .band(0, 4, 1)?
                    .run(Run {
                        x0: 0,
                        x1: 1,
                        coverage: 0,
                    })?
                    .finish(),
                6 => b.band(0, 4, 1)?.finish(),
                7 => b
                    .band(0, 2, 1)?
                    .run(Run {
                        x0: 0,
                        x1: 260,
                        coverage: 128,
                    })?
                    .band(2, 4, 1)?
                    .run(Run {
                        x0: 0,
                        x1: 260,
                        coverage: 128,
                    })?
                    .finish(),
                8 => b
                    .band(0, 4, 2)?
                    .run(Run {
                        x0: 0,
                        x1: 1,
                        coverage: 128,
                    })?
                    .run(Run {
                        x0: 1,
                        x1: 2,
                        coverage: 128,
                    })?
                    .finish(),
                9 => b
                    .band(0, 4, 2)?
                    .run(Run {
                        x0: 0,
                        x1: 2,
                        coverage: 128,
                    })?
                    .run(Run {
                        x0: 1,
                        x1: 3,
                        coverage: 255,
                    })?
                    .finish(),
                10 => b.band(0, 4, 3)?.finish(),
                _ => b.finish(),
            }
        })();
        assert_eq!(result.unwrap_err().code, ErrorCode::InputSchema);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn debug_is_bounded_callback_free_and_empty_ignores_full_storage_guard() {
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::CoverageMasks);
    let m = scope.account();
    let paid = Cell::new(0);
    let checks = Cell::new(0);
    let check = || {
        checks.set(checks.get() + 1);
        Ok(())
    };
    let spend = |n| {
        paid.set(paid.get() + n);
        Ok(())
    };
    let b = CanonicalA8Builder::new(config(&vec![]), &scope, &m, &check, &spend).unwrap();
    let before = (paid.get(), checks.get());
    assert!(format!("{b:?}").len() < 160);
    assert_eq!((paid.get(), checks.get()), before);
    let mask = b.finish().unwrap();
    assert!(mask.storage.is_none());
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn negative_coordinates_preserve_digest_and_overflow_counts_refuse_before_reserve() {
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::CoverageMasks);
    let m = guard(&budget.scope(AllocationClass::PaintMetadata));
    let check = || Ok(());
    let paid = Cell::new(0u64);
    let spend = |n| {
        paid.set(paid.get() + n);
        Ok(())
    };
    let c = CanonicalA8Config {
        domain: Rect::from_origin_size(-4, -3, 2, 2).unwrap(),
        bands: 1,
        runs: 1,
        limits: Limits::default(),
    };
    let mask = CanonicalA8Builder::new(c, &scope, &m, &check, &spend)
        .unwrap()
        .band(-3, -1, 1)
        .unwrap()
        .run(Run {
            x0: -4,
            x1: -2,
            coverage: 255,
        })
        .unwrap()
        .finish()
        .unwrap();
    let expected = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/canonical-a8-fixtures/negative-domain.json"
    )))
    .unwrap();
    assert_eq!(
        hex(&mask.digest()),
        expected.get("digest_sha256").unwrap().as_str().unwrap()
    );
    assert_eq!(mask.coverage(-3, -2), 255);
    assert_eq!(mask.coverage(0, 0), 0);
    assert_eq!(
        paid.get(),
        if cfg!(target_pointer_width = "64") {
            4590
        } else {
            4574
        }
    );
    drop(mask);
    paid.set(0);
    let c = CanonicalA8Config {
        domain: domain(),
        bands: usize::MAX,
        runs: usize::MAX,
        limits: Limits {
            bands: usize::MAX,
            runs: usize::MAX,
            work: u64::MAX,
        },
    };
    let error = CanonicalA8Builder::new(c, &scope, &m, &check, &spend).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(paid.get(), 576);
    assert_eq!(scope.footprint().live, 0);
}
