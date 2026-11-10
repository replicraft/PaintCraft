use super::*;
use std::{sync::atomic::AtomicUsize, thread, time::Duration};
const DEADLINE: Duration = Duration::from_secs(5);

fn executor<T: Send + 'static>(budget: &SharedBudget, pending: usize) -> Executor<T> {
    Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: pending,
            transient_bytes: 32,
        },
        budget.clone(),
    )
    .unwrap()
}

#[test]
fn blocked_cancelled_task_holds_original_charge_until_actual_return() {
    let budget = SharedBudget::new(32);
    let mut executor = executor::<()>(&budget, 2);
    let (started, start) = mpsc::channel();
    let (cancelled, cancel) = mpsc::channel();
    let (release, released) = mpsc::channel();
    executor
        .submit(
            8,
            move |token| {
                started.send(()).unwrap();
                while token.check().is_ok() {
                    thread::yield_now();
                }
                cancelled.send(()).unwrap();
                released.recv_timeout(DEADLINE).unwrap();
                token.check()
            },
            || {},
        )
        .unwrap();
    start.recv_timeout(DEADLINE).unwrap();
    let (done, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        done.send(executor.shutdown()).unwrap();
    });
    cancel.recv_timeout(DEADLINE).unwrap();
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert_eq!(budget.usage().reserved, 8);
    release.send(()).unwrap();
    result.recv_timeout(DEADLINE).unwrap().unwrap();
    worker.join().unwrap();
    assert_eq!(budget.usage().reserved, 0);
}

struct Value {
    drops: Arc<AtomicUsize>,
    panic: bool,
}
impl Drop for Value {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        assert!(!self.panic, "test-only result destructor panic");
    }
}

#[test]
fn blocked_wake_and_retained_result_must_both_retire_before_shutdown_returns() {
    let budget = SharedBudget::new(32);
    let mut executor = executor(&budget, 2);
    let drops = Arc::new(AtomicUsize::new(0));
    let value = Value {
        drops: drops.clone(),
        panic: false,
    };
    let (entered, wake) = mpsc::channel();
    let (release, released) = mpsc::channel();
    executor
        .submit(
            8,
            move |_| Ok(value),
            move || {
                entered.send(()).unwrap();
                released.recv_timeout(DEADLINE).unwrap();
            },
        )
        .unwrap();
    wake.recv_timeout(DEADLINE).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let completion = executor.completion.clone();
    let (done, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        done.send(executor.shutdown()).unwrap();
    });
    assert_eq!(completion.lock().active, 1);
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    result.recv_timeout(DEADLINE).unwrap().unwrap();
    worker.join().unwrap();
    assert_eq!(completion.lock().exited, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn task_and_wake_panics_fail_closed_after_actual_retirement() {
    for task_panics in [true, false] {
        let budget = SharedBudget::new(32);
        let mut executor = executor::<()>(&budget, 2);
        let (done, ready) = mpsc::channel();
        executor
            .submit(
                8,
                move |_| {
                    assert!(!task_panics, "test-only task panic");
                    Ok(())
                },
                move || {
                    done.send(()).unwrap();
                    assert!(task_panics, "test-only wake panic");
                },
            )
            .unwrap();
        ready.recv_timeout(DEADLINE).unwrap();
        let completion = executor.completion.clone();
        assert_eq!(
            executor.shutdown().unwrap_err().code,
            ErrorCode::InvariantViolation
        );
        assert_eq!(completion.lock().active, 0);
        assert_eq!(completion.lock().exited, 1);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn panicking_result_drop_does_not_skip_other_results_or_original_admission() {
    let budget = SharedBudget::new(32);
    let mut executor = executor(&budget, 2);
    let drops = Arc::new(AtomicUsize::new(0));
    let (done, ready) = mpsc::channel();
    for panic in [true, false] {
        let value = Value {
            drops: drops.clone(),
            panic,
        };
        let done = done.clone();
        executor
            .submit(
                8,
                move |_| Ok(value),
                move || {
                    done.send(()).unwrap();
                },
            )
            .unwrap();
    }
    ready.recv_timeout(DEADLINE).unwrap();
    ready.recv_timeout(DEADLINE).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(budget.usage().reserved, 16);
    assert_eq!(
        executor.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn rejected_submission_does_not_leave_an_outstanding_guard() {
    let budget = SharedBudget::new(32);
    let mut executor = executor::<()>(&budget, 1);
    let (entered, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    executor
        .submit(
            8,
            move |_| {
                entered.send(()).unwrap();
                released.recv_timeout(DEADLINE).unwrap();
                Ok(())
            },
            || {},
        )
        .unwrap();
    ready.recv_timeout(DEADLINE).unwrap();
    assert_eq!(
        executor.submit(8, |_| Ok(()), || {}).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(executor.completion.lock().active, 1);
    release.send(()).unwrap();
    executor.shutdown().unwrap();
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn poisoned_bookkeeping_refuses_success_without_stranding_shutdown() {
    let budget = SharedBudget::new(32);
    let executor = executor::<()>(&budget, 2);
    let completion = executor.completion.clone();
    let poison = completion.clone();
    assert!(
        thread::spawn(move || {
            let _guard = poison.state.lock().unwrap();
            panic!("test-only bookkeeping poison");
        })
        .join()
        .is_err()
    );
    assert_eq!(
        executor.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
    assert_eq!(completion.lock().exited, 1);
    assert_eq!(budget.usage().reserved, 0);
}
