use super::*;
use std::cell::Cell;
fn policy(v: [i16; 5]) -> LevelsRgbV1 {
    LevelsRgbV1::new(v[0], v[1], v[2], v[3], v[4]).unwrap()
}
fn build(p: LevelsRgbV1) -> ([u8; 256], WorkUsed) {
    let mut output = [199; 256];
    let mut scratch = Scratch::default();
    let used = build_lut(p, &mut scratch, &mut output, quote(p).into(), &|| Ok(())).unwrap();
    assert!(used.units <= quote(p).work);
    (output, used)
}
#[test]
fn identity_inversion_constant_and_nonneutral_exact_halves() {
    let (identity, _) = build(LevelsRgbV1::default());
    assert_eq!(identity, std::array::from_fn(|v| v as u8));
    let (inverted, _) = build(policy([0, 255, 100, 255, 0]));
    assert_eq!(inverted, std::array::from_fn(|v| 255 - v as u8));
    let (constant, _) = build(policy([127, 128, 999, 73, 73]));
    assert_eq!(constant, [73; 256]);
    for (p, want) in [
        ([0, 4, 200, 0, 255], 128),
        ([0, 4, 200, 255, 0], 128),
        ([0, 2, 50, 0, 2], 1),
        ([0, 2, 50, 2, 0], 2),
    ] {
        assert_eq!(build(policy(p)).0[1], want, "{p:?}");
    }
}
#[test]
fn geometry_and_preflight_refuse_before_callbacks_or_publication() {
    let p = policy([0, 255, 999, 255, 0]);
    let q = quote(p);
    let mut scratch = Scratch::default();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    for length in [0, 1, 255, 257] {
        let mut output = vec![199; length];
        assert_eq!(
            build_lut(p, &mut scratch, &mut output, q.into(), &check)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(output, vec![199; length]);
    }
    for limits in [
        Limits {
            scratch_bytes: q.scratch_bytes - 1,
            work: q.work,
        },
        Limits {
            scratch_bytes: q.scratch_bytes,
            work: q.work - 1,
        },
    ] {
        let mut output = [199; 256];
        assert_eq!(
            build_lut(p, &mut scratch, &mut output, limits, &check)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(output, [199; 256]);
    }
    assert_eq!(calls.get(), 0);
    assert!(q.work < 80_000_000);
    assert_eq!(q.scratch_bytes, std::mem::size_of::<Scratch>() as u64);
}
#[test]
fn early_mid_and_final_check_cancellation_never_publish_and_scratch_can_be_reused() {
    let p = policy([0, 4, 200, 255, 0]);
    let mut scratch = Scratch::default();
    let mut output = [199; 256];
    let count = Cell::new(0usize);
    build_lut(p, &mut scratch, &mut output, quote(p).into(), &|| {
        count.set(count.get() + 1);
        Ok(())
    })
    .unwrap();
    let expected = output;
    let total = count.get();
    assert!(total > 100);
    for stop in [1, 2, total / 2, total] {
        let seen = Cell::new(0);
        output.fill(199);
        let check = || {
            let now = seen.get() + 1;
            seen.set(now);
            if now == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "levels test"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            build_lut(p, &mut scratch, &mut output, quote(p).into(), &check)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(output, [199; 256]);
        build_lut(p, &mut scratch, &mut output, quote(p).into(), &|| Ok(())).unwrap();
        assert_eq!(output, expected);
    }
}
#[test]
fn fixed_capacity_grouping_terminates_for_zero_one_and_reports_checked_overflow() {
    let mut scratch = Scratch::default();
    let mut budget = Budget {
        used: 0,
        limit: u64::MAX,
        check: &|| Ok(()),
    };
    scratch.left.pow(0, 999, &mut budget).unwrap();
    assert_eq!(
        scratch.left.compare(&Integer::zero(), &mut budget).unwrap(),
        std::cmp::Ordering::Equal
    );
    scratch.left.pow(1, 999, &mut budget).unwrap();
    scratch.right.pow(2, 0, &mut budget).unwrap();
    assert_eq!(
        scratch.left.compare(&scratch.right, &mut budget).unwrap(),
        std::cmp::Ordering::Equal
    );
    scratch.left.pow(2, 10239, &mut budget).unwrap();
    scratch.right.pow(2, 1, &mut budget).unwrap();
    assert_eq!(
        scratch
            .temporary
            .product(&scratch.left, &scratch.right, &mut budget)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        scratch.left.pow(2, 10240, &mut budget).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let output = build(policy([0, 255, 999, 0, 255])).0;
    assert_eq!((output[0], output[255]), (0, 255));
    assert!(output.windows(2).all(|v| v[0] <= v[1]));
}
