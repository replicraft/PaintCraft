use super::{
    test_api::{Backend, Reference, Wide},
    *,
};
use std::cell::Cell;
fn policy(stops: i16) -> ExposureSrgbV1 {
    ExposureSrgbV1::new(stops).unwrap()
}
fn controls<B: Backend>() {
    let p = policy(137);
    let q = B::quote(p);
    let mut scratch = B::Scratch::default();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    for n in [0, 1, 255, 257] {
        let mut out = vec![193; n];
        assert_eq!(
            B::build(p, &mut scratch, &mut out, q.into(), &check)
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(out, vec![193; n]);
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
        let mut out = [193; 256];
        assert_eq!(
            B::build(p, &mut scratch, &mut out, limits, &check)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(out, [193; 256]);
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(q.scratch_bytes, std::mem::size_of::<B::Scratch>() as u64);
    let mut expected = [193; 256];
    let used = B::build(p, &mut scratch, &mut expected, q.into(), &check).unwrap();
    assert!(used.units <= q.work);
    let total = calls.get();
    assert!(total > 100);
    for stop in [1, 2, total / 2, total] {
        let seen = Cell::new(0);
        let mut out = [193; 256];
        let cancel = || {
            let now = seen.get() + 1;
            seen.set(now);
            if now == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "Exposure test"))
            } else {
                Ok(())
            }
        };
        assert_eq!(
            B::build(p, &mut scratch, &mut out, q.into(), &cancel)
                .unwrap_err()
                .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(out, [193; 256]);
        // Exercise stale private threshold/power contents before reusing scratch.
        let opposite = policy(-137);
        B::build(
            opposite,
            &mut scratch,
            &mut out,
            B::quote(opposite).into(),
            &|| Ok(()),
        )
        .unwrap();
        let retry = B::build(p, &mut scratch, &mut out, q.into(), &|| Ok(())).unwrap();
        assert_eq!(out, expected);
        assert_eq!(retry, used);
    }
    let mut out = [193; 256];
    let neutral = policy(0);
    let used = B::build(
        neutral,
        &mut scratch,
        &mut out,
        B::quote(neutral).into(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(out, std::array::from_fn(|i| i as u8));
    assert_eq!(used.units, 512);
    assert_eq!(B::quote(neutral).work, 512);
    assert!(evaluate_pixel(&out, Rgba([10, 11, 255, 0])).0 == [10, 11, 255, 0]);
    for stops in -800..=800 {
        assert!(B::quote(policy(stops)).work < B::WORK_BOUND);
    }
}
#[test]
fn reference_geometry_preflight_early_mid_final_cancel_reuse_and_identity() {
    controls::<Reference>();
}
#[test]
fn wide_geometry_preflight_early_mid_final_cancel_reuse_and_identity() {
    controls::<Wide>();
}
#[test]
fn transparent_scratch_layout_and_small_private_capacity_errors_preserve_output() {
    assert_eq!(
        std::mem::size_of::<Scratch>(),
        std::mem::size_of::<engine::Workspace<word::U32, 320>>()
    );
    assert_eq!(
        std::mem::align_of::<Scratch>(),
        std::mem::align_of::<engine::Workspace<word::U32, 320>>()
    );
    assert_eq!(
        std::mem::size_of::<u64::Scratch>(),
        std::mem::size_of::<engine::Workspace<word::U64, 160>>()
    );
    assert_eq!(
        std::mem::align_of::<u64::Scratch>(),
        std::mem::align_of::<engine::Workspace<word::U64, 160>>()
    );
    let mut tiny = engine::Workspace::<word::U32, 1>::default();
    let mut out = [193; 256];
    let p = policy(137);
    let err = engine::build_lut(
        p,
        &mut tiny,
        &mut out,
        std::primitive::u64::MAX,
        quote(p).work,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ResourceLimit);
    assert!(err.message.contains("Exposure"));
    assert_eq!(out, [193; 256]);
}
// A separate accounting implementation checks shared primitive quotes. It does
// not call either compiler's Budget, and raw counts never cross word widths.
#[derive(Default)]
struct Recorder {
    units: u64,
    charges: usize,
}
impl crate::exact::Budget for Recorder {
    fn charge(&mut self, n: u64) -> Result<()> {
        self.units = self.units.checked_add(n).unwrap();
        self.charges += 1;
        Ok(())
    }
    fn resource(&self, _: crate::exact::Fault) -> Error {
        resource("Exposure independent primitive counter")
    }
}
fn primitive_quotes<W: word::Word, const N: usize>() {
    let mut a = integer::Integer::<W, N>::zero();
    let mut b = integer::Integer::<W, N>::zero();
    let mut c = integer::Integer::<W, N>::zero();
    let mut r = Recorder::default();
    a.pow(10761, 240, &mut r).unwrap();
    assert!(r.units <= integer::power_quote::<W>(10761, 240));
    r = Recorder::default();
    b.pow(32946, 100, &mut r).unwrap();
    assert!(r.units <= integer::power_quote::<W>(32946, 100));
    let aw = integer::words::<W>(10761, 240);
    let bw = integer::words::<W>(32946, 100);
    r = Recorder::default();
    c.product(&a, &b, &mut r).unwrap();
    assert!(r.units <= integer::product_quote(aw, bw));
    let capacity = aw + bw + 800u64.div_ceil(u64::from(W::BITS));
    let shift_quote = 1 + 800u64.div_ceil(u64::from(W::BITS - 1)) * (capacity + 2);
    r = Recorder::default();
    c.scale_pow2(800, &mut r).unwrap();
    assert!(r.units <= shift_quote);
    assert!(r.charges > 1);
    // Two independently constructed equal integers agree after the bounded shift.
    a.product(&b, &b, &mut r).unwrap();
    c.product(&b, &b, &mut r).unwrap();
    a.scale_pow2(0, &mut r).unwrap();
    assert!(a.compare(&c, &mut r).unwrap().is_eq());
}
#[test]
fn independently_recorded_reference_and_wide_primitive_costs_fit_their_own_quotes() {
    primitive_quotes::<word::U32, 320>();
    primitive_quotes::<word::U64, 160>();
}
