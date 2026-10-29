//! Exercise the actual shared executor without starting a codec/helper process.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
fn unexpected() -> Result<PreparedJxrExport> {
    Err(Error::new(ErrorCode::InputSchema, "admitted control task"))
}
#[test]
fn impossible_quote_and_external_global_quota_never_start_a_jxr_task() {
    let budget = SharedBudget::new(PREPARATION_CHARGE);
    let service = Service::with_budget(egui::Context::default(), budget.clone()).unwrap();
    let called = Arc::new(AtomicBool::new(false));
    for external in [false, true] {
        let held = external.then(|| budget.reserve(PREPARATION_CHARGE).unwrap());
        let callback = called.clone();
        let quote = if external { 1 } else { PREPARATION_CHARGE + 1 };
        let error = service
            .try_submit_jxr_export(quote, move |_| {
                callback.store(true, Ordering::SeqCst);
                unexpected()
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(
            budget.usage().reserved,
            if external { PREPARATION_CHARGE } else { 0 }
        );
        if !external {
            assert_eq!(budget.usage().peak, 0);
        }
        drop(held);
    }
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn live_and_canceled_executor_contention_retries_only_after_actual_retirement() {
    let budget = SharedBudget::new(3 << 30);
    let service = Service::with_budget(egui::Context::default(), budget.clone()).unwrap();
    let refused_called = Arc::new(AtomicBool::new(false));
    let (started, wait_started) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (finished, wait_finished) = mpsc::channel();
    let pending = service
        .try_submit_jxr_export(1, move |cancel| {
            started.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
            let error = cancel.check().unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            finished.send(()).unwrap();
            Err(error)
        })
        .unwrap()
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(budget.usage().reserved, PREPARATION_CHARGE);
    let rejected = refused_called.clone();
    assert!(
        service
            .try_submit_jxr_export(1, move |_| {
                rejected.store(true, Ordering::SeqCst);
                unexpected()
            })
            .unwrap()
            .is_none()
    );
    drop(pending);
    assert_eq!(
        budget.usage().reserved,
        PREPARATION_CHARGE,
        "dropping pending did not reclaim a live task"
    );
    let rejected = refused_called.clone();
    assert!(
        service
            .try_submit_jxr_export(1, move |_| {
                rejected.store(true, Ordering::SeqCst);
                unexpected()
            })
            .unwrap()
            .is_none()
    );
    release.send(()).unwrap();
    wait_finished.recv_timeout(Duration::from_secs(5)).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while budget.usage().reserved != 0 {
        service.reap_abandoned();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!refused_called.load(Ordering::SeqCst));
    let until = Instant::now() + Duration::from_secs(5);
    let retry = service
        .try_submit_jxr_export(1, move |_| unexpected())
        .unwrap()
        .unwrap();
    let error = loop {
        if let Some(value) = retry.poll() {
            break value.unwrap_err();
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(error.message, "admitted control task");
    assert!(!refused_called.load(Ordering::SeqCst));
    drop(retry);
    service.reap_abandoned();
    assert_eq!(budget.usage().reserved, 0);
}
