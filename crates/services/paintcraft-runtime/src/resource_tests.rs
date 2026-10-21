use super::*;
use std::time::{Duration, Instant};

fn ready<T: Send + 'static>(executor: &mut Executor<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((_, result)) = executor.poll() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
#[test]
fn external_resources_keep_both_local_and_shared_admission_after_result_consumption() {
    let budget = SharedBudget::new(256);
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 32,
        },
        budget.clone(),
    )
    .unwrap();
    executor
        .submit(
            24,
            |cancel| Ok((cancel.clone(), cancel.resource_lease().unwrap())),
            || {},
        )
        .unwrap();
    let (token, hold) = ready(&mut executor);
    assert_eq!(executor.pending(), 0);
    assert_eq!(executor.reserved_bytes(), 24);
    assert_eq!(budget.usage().reserved, 24);
    assert!(!executor.can_submit(9));
    assert!(
        executor
            .submit(
                9,
                |_| unreachable!("local outstanding charge must reject"),
                || {}
            )
            .is_err()
    );
    drop(hold);
    assert_eq!(executor.reserved_bytes(), 0);
    assert_eq!(budget.usage().reserved, 0);
    assert!(token.resource_lease().is_none());
}
#[test]
fn cancellation_tokens_alone_do_not_pin_completed_resources() {
    let mut executor = Executor::new(Limits {
        workers: 1,
        pending_jobs: 1,
        transient_bytes: 32,
    })
    .unwrap();
    executor.submit(32, Ok, || {}).unwrap();
    let token = ready(&mut executor);
    assert_eq!(executor.reserved_bytes(), 0);
    assert!(token.resource_lease().is_none());
}
#[test]
fn external_resource_ownership_survives_cancellation_and_executor_drop() {
    let budget = SharedBudget::new(64);
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 32,
        },
        budget.clone(),
    )
    .unwrap();
    let (send, receive) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let (done, wake) = mpsc::sync_channel(1);
    let handle = executor
        .submit(
            32,
            move |cancel| {
                send.send(cancel.resource_lease().unwrap()).unwrap();
                wait.recv().unwrap();
                Ok(())
            },
            move || {
                done.send(()).unwrap();
            },
        )
        .unwrap();
    let hold = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(executor.cancel(handle));
    drop(executor);
    release.send(()).unwrap();
    wake.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(budget.usage().reserved, 32);
    drop(hold);
    assert_eq!(budget.usage().reserved, 0);
}
