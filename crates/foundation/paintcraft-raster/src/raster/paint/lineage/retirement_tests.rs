//! Safe owning-carrier retirement and the actual conditional reader's old fees.
use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Witness {
    payload: Vec<u8>,
    retired: Arc<AtomicUsize>,
    bit: usize,
}
impl Drop for Witness {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.payload));
        self.retired.fetch_or(self.bit, Ordering::Release);
    }
}
#[test]
fn original_release_wake_observes_both_payload_owners_retired_and_reenters_same_scope() {
    use paintcraft_core::types::resources::AllocationClass;
    let budget = SharedBudget::new(80);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut charge = scope.account();
    charge.resize(80).unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let owner = RetainedScratch {
        payload: (
            vec![Witness {
                payload: vec![1; 17],
                retired: retired.clone(),
                bit: 1,
            }],
            Some(Mutex::new(vec![Witness {
                payload: vec![2; 19],
                retired: retired.clone(),
                bit: 2,
            }])),
        ),
        charge,
    };
    let armed = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(AtomicUsize::new(0));
    let reentered = Arc::new(AtomicBool::new(false));
    let (a, r, s, e, f) = (
        armed.clone(),
        retired.clone(),
        seen.clone(),
        reentered.clone(),
        scope.clone(),
    );
    let watch = budget
        .watch(1, move || {
            if a.swap(false, Ordering::AcqRel) {
                s.store(r.load(Ordering::Acquire), Ordering::Release);
                let mut next = f.account();
                e.store(next.resize(80).is_ok(), Ordering::Release);
            }
        })
        .unwrap();
    watch.wait_for_release(true);
    assert_eq!(watch.observe().reserved, 80); // Acknowledge initial coalesced wake.
    armed.store(true, Ordering::Release);
    drop(owner);
    assert_eq!(seen.load(Ordering::Acquire), 3);
    assert!(reentered.load(Ordering::Acquire));
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
}
fn conditional(admission: &Admission) -> PaintSnapshot {
    use paintcraft_core::types::rotation::Rotation;
    let mut raster = RasterLayer::new(2, 1, Rgba::TRANSPARENT);
    raster
        .blend_pixel(
            0,
            0,
            Rgba([17, 34, 51, 0]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let source = PaintSnapshot::capture(&raster, admission, &|| Ok(())).unwrap();
    let g = Rotation::new(raster.bounds(), (644245094, 858993459)).unwrap();
    let plan = MappedPlan::prepare_rotation(
        source,
        super::super::affine::RotationMapping {
            transform: g,
            source_origin: (0, 0),
            destination: g.bounds(g.source()).unwrap(),
            selection: None,
            clear: Rgba::TRANSPARENT,
        },
        admission,
        16_777_216,
        &|| Ok(()),
    )
    .unwrap();
    plan.snapshot(admission, &|| Ok(())).unwrap()
}
fn exact_scratch() -> u64 {
    // Two lineage nodes, each Vec's first exact growth is four elements.
    4 * (size_of::<Frame<'_>>() + size_of::<(i64, i64)>()) as u64
}
#[test]
fn actual_conditional_reader_keeps_exact_old_payload_fee_and_original_factory_boundary() {
    let budget = SharedBudget::new(4096);
    let admission = Admission::new(0).with_paint_metadata_budget(budget.clone());
    let source = conditional(&admission);
    let held = budget.usage().reserved;
    assert_eq!(size_of::<sampling::Coordinates>(), 48);
    assert_eq!(size_of::<Frame<'_>>(), 56); // Native64 and Wasm32 align i64 to eight.
    assert_eq!(exact_scratch(), 288);
    let mut blocker = admission.paint_metadata_account();
    blocker.resize(4096 - held - exact_scratch()).unwrap();
    let before = budget.usage().reserved;
    let reader = source.reader(&admission, &|| Ok(())).unwrap();
    assert!(reader.scratch.payload.positions.is_some());
    assert_eq!(reader.scratch.payload.chain.len(), 2);
    assert_eq!(reader.scratch_bytes(), exact_scratch());
    assert_eq!(budget.usage().reserved, 4096);
    // Independent inverse centers at real2x3 output: hidden source first pixel
    // is preserved at x0,y0, and transparent padding occupies x1,y0.
    assert_eq!(
        reader.read_checked(0, 0, &|| Ok(())).unwrap(),
        Rgba([17, 34, 51, 0])
    );
    assert_eq!(
        reader.read_checked(1, 0, &|| Ok(())).unwrap(),
        Rgba::TRANSPARENT
    );
    drop(reader);
    assert_eq!(budget.usage().reserved, before);
    blocker.resize(blocker.bytes() + 1).unwrap();
    assert_eq!(
        source.reader(&admission, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, before + 1);
    drop(blocker);
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn every_observed_actual_reader_cancel_drops_both_private_vectors_and_original_debt() {
    use std::cell::Cell;
    let budget = SharedBudget::new(4096);
    let admission = Admission::new(0).with_paint_metadata_budget(budget.clone());
    let source = conditional(&admission);
    let held = budget.usage().reserved;
    let calls = Cell::new(0);
    drop(
        source
            .reader(&admission, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    assert_eq!(budget.usage().reserved, held);
    for cut in 1..=calls.get() {
        let n = Cell::new(0);
        let error = source
            .reader(&admission, &|| {
                n.set(n.get() + 1);
                if n.get() == cut {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "reader cancellation identity",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "reader cancellation identity");
        assert_eq!(budget.usage().reserved, held);
    }
    assert_eq!(
        source
            .reader(&admission, &|| Ok(()))
            .unwrap()
            .read_checked(0, 0, &|| Ok(()))
            .unwrap(),
        Rgba([17, 34, 51, 0])
    );
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
}
/// Exact prior defining field model, not a universal platform-mutex constant.
struct PriorReader<'a> {
    source: &'a PaintSnapshot,
    program: Option<&'a PaintProjection>,
    bounds: Rect,
    chain: Vec<Frame<'a>>,
    charge: BudgetAccount,
    positions: Option<Mutex<Vec<(i64, i64)>>>,
}
#[test]
fn ownership_factoring_preserves_complete_target_layout_and_uniform_zero_scratch() {
    assert_eq!(size_of::<PaintReader<'_>>(), size_of::<PriorReader<'_>>());
    assert_eq!(
        std::mem::align_of::<PaintReader<'_>>(),
        std::mem::align_of::<PriorReader<'_>>()
    );
    let budget = SharedBudget::new(4096);
    let admission = Admission::new(0).with_paint_metadata_budget(budget.clone());
    let source = PaintSnapshot::new(
        RasterLayer::new(2, 1, Rgba([7, 9, 11, 0])),
        &admission,
        &|| Ok(()),
    )
    .unwrap();
    let held = budget.usage().reserved;
    let prior = PriorReader {
        source: &source,
        program: None,
        bounds: source.bounds(),
        chain: Vec::new(),
        charge: admission.paint_metadata_account(),
        positions: None,
    };
    // Read every legacy field, keeping the defining comparison free of dead-code allowances.
    assert_eq!(prior.source.bounds(), prior.bounds);
    assert!(prior.program.is_none());
    assert!(prior.chain.is_empty());
    assert_eq!(prior.charge.bytes(), 0);
    assert!(prior.positions.is_none());
    let reader = source.reader(&admission, &|| Ok(())).unwrap();
    assert_eq!(reader.scratch_bytes(), 0);
    assert!(reader.scratch.payload.positions.is_none());
    assert_eq!(
        reader.read_checked(0, 0, &|| Ok(())).unwrap(),
        Rgba([7, 9, 11, 0])
    );
    drop(reader);
    drop(prior);
    assert_eq!(budget.usage().reserved, held);
    drop(source);
    assert_eq!(budget.usage().reserved, 0);
}

// These are compiler obligations on every actual cfg(test) target, including
// Wasm --tests checks. Plain production compilation does not evaluate tests.
const _: () = {
    assert!(size_of::<PaintReader<'static>>() == size_of::<PriorReader<'static>>());
    assert!(
        std::mem::align_of::<PaintReader<'static>>()
            == std::mem::align_of::<PriorReader<'static>>()
    );
    assert!(size_of::<sampling::Coordinates>() == 48);
    assert!(size_of::<Frame<'static>>() == 56);
    assert!(size_of::<(i64, i64)>() == 16);
};
