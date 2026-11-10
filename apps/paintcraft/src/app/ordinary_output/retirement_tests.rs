use super::*;
use std::sync::Barrier;
#[test]
fn output_completion_receiver_absence_does_not_hide_a_live_callback_thread() {
    let release = Arc::new(Barrier::new(2));
    let wait = Arc::clone(&release);
    let retired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&retired);
    let mut state = State::default();
    state
        .threads
        .spawn("output-retirement-test", move || {
            wait.wait();
            flag.store(true, Ordering::Release);
        })
        .unwrap();
    assert!(state.pending.is_none());
    assert!(!state.busy());
    assert!(state.retirement_busy());
    let threads = state.retire_for_update().unwrap();
    assert!(state.retiring);
    let drain = std::thread::spawn(move || threads.shutdown());
    assert!(!retired.load(Ordering::Acquire));
    release.wait();
    drain.join().unwrap().unwrap();
    assert!(retired.load(Ordering::Acquire));
}
#[test]
fn a_retired_output_preparation_cannot_publish_another_export() {
    let app = App::new(None);
    let frozen = Frozen::capture(&app).unwrap();
    let mut state = State::default();
    state.retire_for_update().unwrap().shutdown().unwrap();
    let budget = paintcraft_runtime::SharedBudget::new(256 << 20);
    let error = state
        .start(
            frozen,
            Intent::Print("must not start".into()),
            &budget,
            &app.context,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(!state.busy());
    assert_eq!(budget.usage().reserved, 0);
}
