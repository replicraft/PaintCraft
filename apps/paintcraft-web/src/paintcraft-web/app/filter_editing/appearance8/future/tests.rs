//! Actual own-F construction, debt, account and destructor ordering.
use super::*;
use crate::app::filter_editing::appearance8::{factory, operation};
use paintcraft_core::types::{
    ErrorCode,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::Admission;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::Waker,
};

struct Args {
    made: Rc<Cell<bool>>,
    dropped: Rc<Cell<bool>>,
    scope: paintcraft_core::types::resources::BudgetScope,
    baseline: u64,
}
struct Probe(Args);
impl Future for Probe {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        assert!(
            self.0.scope.footprint().live > self.0.baseline,
            "own F must die before its original control-account refund"
        );
        self.0.dropped.set(true);
    }
}
fn make(args: Args) -> Probe {
    args.made.set(true);
    Probe(args)
}
fn setup(cap: u64) -> (SharedBudget, Factory8, Rc<Cell<u32>>) {
    let budget = SharedBudget::new(cap);
    let scope = budget
        .reserve_capacity(cap)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let admission = Admission::new(0).with_paint_metadata_scope(scope);
    let released = Rc::new(Cell::new(0));
    let factory = factory::Factory8::observed(&admission, cap, released.clone()).unwrap();
    drop(admission);
    (budget, factory, released)
}
fn args(factory: &Factory8) -> Args {
    Args {
        made: Rc::new(Cell::new(false)),
        dropped: Rc::new(Cell::new(false)),
        scope: factory.scope().clone(),
        baseline: factory.scope().footprint().live,
    }
}
#[test]
fn actual_boxed_future_drops_before_account_and_original_factory_credit() {
    let (budget, factory, released) = setup(1 << 20);
    let input = args(&factory);
    let made = input.made.clone();
    let dropped = input.dropped.clone();
    let q = quote(make).unwrap();
    let baseline = factory.scope().footprint().live;
    let work = Cell::new(0);
    let mut tracked = track(
        &factory,
        &|n| {
            work.set(work.get() + n);
            Ok(())
        },
        input,
        make,
    )
    .unwrap_or_else(|_| panic!("admit actual compiled future"));
    assert!(made.get());
    assert!(!dropped.get());
    assert_eq!(work.get(), 4096 + q.work);
    assert_eq!(factory.scope().footprint().live, baseline + q.bytes);
    let waker = Waker::noop();
    assert!(
        Pin::new(&mut tracked)
            .poll(&mut Context::from_waker(waker))
            .is_pending()
    );
    drop(factory);
    assert_eq!(released.get(), 0);
    drop(tracked);
    assert!(dropped.get());
    // Args retained a scope solely for the destructor observation; dropping F
    // destroys that alias before the wrapper's original factory/credit.
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn one_byte_short_original_account_denies_before_future_constructor_or_box() {
    let (budget, factory, released) = setup(1 << 20);
    let input = args(&factory);
    let made = input.made.clone();
    let dropped = input.dropped.clone();
    let q = quote(make).unwrap();
    let mut pressure = factory.scope().account();
    pressure
        .resize(factory.cap() - factory.scope().footprint().live - q.bytes + 1)
        .unwrap();
    let before = factory.scope().footprint();
    let (error, input) = match track(&factory, &|_| Ok(()), input, make) {
        Ok(_) => panic!("one-byte-short original scope admitted own F"),
        Err(value) => value,
    };
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(!made.get());
    assert!(!dropped.get());
    assert_eq!(factory.scope().footprint().live, before.live);
    drop(input);
    drop(pressure);
    drop(error);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn original_counter_denial_returns_unconsumed_args_and_retry_keeps_debt() {
    let (_budget, factory, _released) = setup(1 << 20);
    let input = args(&factory);
    let made = input.made.clone();
    let q = quote(make).unwrap();
    let original = paintcraft_render::spatial::WorkspaceAdmission::new(
        factory.scope().clone(),
        1,
        q.work + 4095,
    );
    original.reserve_work(17).unwrap();
    let original =
        paintcraft_render::spatial::with_workspace(original.clone(), operation::Parent8::capture)
            .unwrap();
    let operation = operation::Operation8::preparation(original, factory.clone()).unwrap();
    let kept = RefCell::new(Some(input));
    let mut expected = 17;
    for _ in 0..2 {
        let remaining = (q.work + 4095) - expected;
        if remaining >= 4096 {
            expected += 4096;
        }
        let input = kept.borrow_mut().take().unwrap();
        let (error, input) = match track(&factory, &|n| operation.spend(n), input, make) {
            Ok(_) => panic!("future escaped original work denial"),
            Err(value) => value,
        };
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(!made.get());
        assert_eq!(operation.used(), expected);
        *kept.borrow_mut() = Some(input);
    }
    assert!(operation.used() >= 17 + 4096);
}

#[test]
fn denied_quote_prefix_does_not_construct_or_charge_an_application_future() {
    let (budget, factory, _released) = setup(1 << 20);
    let caller =
        paintcraft_render::spatial::WorkspaceAdmission::new(factory.scope().clone(), 1, 17 + 4095);
    caller.reserve_work(17).unwrap();
    let captured =
        paintcraft_render::spatial::with_workspace(caller.clone(), operation::Parent8::capture)
            .unwrap();
    let operation = operation::Operation8::preparation(captured, factory.clone()).unwrap();
    let mut input = args(&factory);
    let made = input.made.clone();
    let dropped = input.dropped.clone();
    let before = (
        factory.scope().footprint(),
        budget.usage(),
        budget.activity(),
    );
    for _ in 0..2 {
        let (error, returned) = match track(&factory, &|n| operation.spend(n), input, make) {
            Ok(_) => panic!("one-short prefix constructed own F"),
            Err(value) => value,
        };
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(operation.used(), 17);
        assert_eq!(caller.work_used(), 17);
        assert!(!made.get());
        assert!(!dropped.get());
        assert_eq!(
            (
                factory.scope().footprint(),
                budget.usage(),
                budget.activity()
            ),
            before
        );
        input = returned;
    }
}
