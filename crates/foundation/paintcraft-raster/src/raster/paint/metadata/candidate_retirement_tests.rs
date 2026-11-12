//! Actual candidate owner/error retirement and unchanged mapped constructor fees.
use super::*;
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
struct Witness {
    payload: Vec<u8>,
    retired: Arc<AtomicBool>,
}
impl Drop for Witness {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.payload));
        self.retired.store(true, Ordering::Release);
    }
}
fn table(retired: Arc<AtomicBool>) -> HashMap<TileCoord, Witness> {
    let mut values = HashMap::new();
    values.try_reserve(1).unwrap();
    values.insert(
        TileCoord { tx: 0, ty: 0 },
        Witness {
            payload: vec![7; 17],
            retired,
        },
    );
    values
}
fn current_error(owner: RetainedTable<Witness>) -> Result<()> {
    let _owner = owner;
    Err(Error::new(
        ErrorCode::OperationCancelled,
        "candidate error identity",
    ))
}
fn historical_error(values: HashMap<TileCoord, Witness>, charge: BudgetAccount) -> Result<()> {
    // Recreate the exact historical local declaration order: reverse drop sends
    // a capacity wake while the earlier table is still an owned local.
    let _values = values;
    let _charge = charge;
    Err(Error::new(
        ErrorCode::OperationCancelled,
        "candidate error identity",
    ))
}
fn observe(new_owner: bool) -> (bool, bool, bool) {
    use paintcraft_core::types::resources::AllocationClass;
    let budget = SharedBudget::new(192);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let retired = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(AtomicBool::new(false));
    let reentered = Arc::new(AtomicBool::new(false));
    let armed = Arc::new(AtomicBool::new(false));
    let mut charge = scope.account();
    charge.resize(192).unwrap();
    let (a, r, o, e, f) = (
        armed.clone(),
        retired.clone(),
        observed.clone(),
        reentered.clone(),
        scope.clone(),
    );
    let watch = budget
        .watch(1, move || {
            if a.swap(false, Ordering::AcqRel) {
                o.store(r.load(Ordering::Acquire), Ordering::Release);
                let mut next = f.account();
                e.store(next.resize(192).is_ok(), Ordering::Release);
            }
        })
        .unwrap();
    watch.wait_for_release(true);
    assert_eq!(watch.observe().reserved, 192);
    let values = table(retired.clone());
    armed.store(true, Ordering::Release);
    let result = if new_owner {
        current_error(RetainedTable { values, charge })
    } else {
        historical_error(values, charge)
    };
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message, "candidate error identity");
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
    (
        observed.load(Ordering::Acquire),
        retired.load(Ordering::Acquire),
        reentered.load(Ordering::Acquire),
    )
}
#[test]
fn actual_owner_retires_table_before_error_wake_historical_locals_prove_the_regression() {
    assert_eq!(observe(false), (false, true, true));
    assert_eq!(observe(true), (true, true, true));
}
#[test]
fn unit_candidate_growth_retains_exact_existing_payload_and_denial_cleanup() {
    let budget = SharedBudget::new(4096);
    let admission = Admission::new(0).with_paint_metadata_budget(budget.clone());
    let mut owner = RetainedTable::<()>::new(&admission);
    let mut meta = Metadata::new(0, 48).unwrap();
    meta.table(&mut owner.values, &mut owner.charge).unwrap();
    owner.values.insert(TileCoord { tx: 0, ty: 0 }, ());
    assert_eq!(owner.values.capacity(), 3);
    assert_eq!(owner.charge.bytes(), 48);
    assert_eq!(meta.used, 48);
    drop(owner);
    assert_eq!(budget.usage().reserved, 0);
    let mut owner = RetainedTable::<()>::new(&admission);
    let mut meta = Metadata::new(0, 47).unwrap();
    assert_eq!(
        meta.table(&mut owner.values, &mut owner.charge)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(owner.values.capacity(), 0);
    assert_eq!(owner.charge.bytes(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
const SOURCE: [Rgba; 6] = [
    Rgba([11, 22, 33, 0]),
    Rgba([44, 55, 66, 64]),
    Rgba([77, 88, 99, 128]),
    Rgba([111, 122, 133, 255]),
    Rgba([144, 155, 166, 17]),
    Rgba([177, 188, 199, 201]),
];
const CLEAR: Rgba = Rgba([13, 14, 15, 16]);
fn raw(r: &RasterLayer) -> Vec<Rgba> {
    let mut out = Vec::new();
    for y in 0..r.height() {
        for x in 0..r.width() {
            out.push(r.get(i64::from(x), i64::from(y)))
        }
    }
    out
}
fn snapshot(admission: &Admission) -> (RasterLayer, PaintSnapshot) {
    let mut r = RasterLayer::new(3, 2, Rgba::TRANSPARENT);
    let mut input = admission.clone();
    for y in 0..2 {
        for x in 0..3 {
            r.blend_pixel(
                x,
                y,
                SOURCE[(y * 3 + x) as usize],
                BlendMode::Replace,
                &mut input,
            )
            .unwrap();
        }
    }
    let s = PaintSnapshot::capture(&r, admission, &|| Ok(())).unwrap();
    (r, s)
}
fn prepare(
    source: PaintSnapshot,
    patch: bool,
    admission: &Admission,
    limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<MappedPlan> {
    use paintcraft_core::types::orthogonal::OrthogonalTransform::RotateRight;
    if patch {
        MappedPlan::prepare_patch(
            source,
            Rect::new(0, 0, 2, 1).unwrap(),
            RotateRight,
            CLEAR,
            admission,
            limit,
            check,
        )
        .map(|p| p.0)
    } else {
        MappedPlan::prepare_transform(source, RotateRight, admission, limit, check)
    }
}
#[test]
fn actual_both_candidate_constructors_keep_pixels_exact_read_boundary_and_all_cancel_debt() {
    for patch in [false, true] {
        let budget = SharedBudget::new(1 << 20);
        let admission = Admission::new(65536).with_paint_metadata_budget(budget.clone());
        let (input, source) = snapshot(&admission);
        let held = budget.usage().reserved;
        assert_eq!(
            prepare(source.clone(), patch, &admission, 5, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, held);
        let calls = Cell::new(0);
        let plan = prepare(source.clone(), patch, &admission, 6, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(plan.source_reads(), 6);
        let result = plan.replay(&mut admission.clone(), &|| Ok(())).unwrap();
        let expected = if patch {
            vec![SOURCE[0], CLEAR, SOURCE[2], SOURCE[1], SOURCE[4], SOURCE[5]]
        } else {
            vec![
                SOURCE[3], SOURCE[0], SOURCE[4], SOURCE[1], SOURCE[5], SOURCE[2],
            ]
        };
        assert_eq!(
            (result.width(), result.height()),
            if patch { (3, 2) } else { (2, 3) }
        );
        assert_eq!(raw(&result), expected);
        assert_eq!(raw(&input), SOURCE);
        drop(result);
        drop(plan);
        assert_eq!(budget.usage().reserved, held);
        for cut in 1..=calls.get() {
            let n = Cell::new(0);
            let error = prepare(source.clone(), patch, &admission, 6, &|| {
                n.set(n.get() + 1);
                if n.get() == cut {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "original candidate cut",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(error.message, "original candidate cut");
            assert_eq!(budget.usage().reserved, held);
            assert_eq!(raw(&input), SOURCE);
        }
        drop(source);
        assert_eq!(budget.usage().reserved, 0);
    }
}
// Compiler obligations on actual cfg(test) Native/Wasm targets. The carrier
// adds no heap owner and retains the complete prior inline table/account layout.
const _: () = {
    assert!(size_of::<(TileCoord, ())>() == 16);
    assert!(size_of::<RetainedTable<()>>() == size_of::<(HashMap<TileCoord, ()>, BudgetAccount)>());
    assert!(
        std::mem::align_of::<RetainedTable<()>>()
            == std::mem::align_of::<(HashMap<TileCoord, ()>, BudgetAccount)>()
    );
};
