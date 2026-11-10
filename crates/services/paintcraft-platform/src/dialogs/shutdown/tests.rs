use super::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};
const DEADLINE: Duration = Duration::from_secs(5);
fn dialogs(worker: std::thread::JoinHandle<()>) -> NativeDialogs {
    let (sender, receiver) = mpsc::channel();
    NativeDialogs {
        builder: rfd::AsyncFileDialog::new(),
        sender,
        receiver,
        pending: true,
        active_kind: Some(Kind::Open),
        worker: Some(worker),
        failed: false,
    }
}
#[test]
fn polled_result_does_not_replace_a_still_running_wake_or_skip_shutdown_join() {
    let (release, released) = mpsc::channel();
    let (ended, end) = mpsc::channel();
    let worker = thread::spawn(move || {
        released.recv_timeout(DEADLINE).unwrap();
        ended.send(()).unwrap();
    });
    let identity = worker.thread().id();
    let mut dialogs = dialogs(worker);
    dialogs
        .sender
        .send(Completion {
            kind: Kind::Open,
            path: None,
        })
        .unwrap();
    assert!(dialogs.poll().is_some());
    assert!(dialogs.pending());
    let calls = Arc::new(AtomicUsize::new(0));
    let wake = calls.clone();
    dialogs.request_with_filters(
        Kind::Open,
        None,
        &[Filter {
            name: "Images".into(),
            extensions: vec!["png".into()],
        }],
        str::to_owned,
        move || {
            wake.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(dialogs.worker.as_ref().unwrap().thread().id(), identity);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (done, result) = mpsc::channel();
    let join = thread::spawn(move || {
        done.send(dialogs.shutdown()).unwrap();
    });
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    end.recv_timeout(DEADLINE).unwrap();
    result.recv_timeout(DEADLINE).unwrap().unwrap();
    join.join().unwrap();
}
#[test]
fn default_drop_does_not_join_an_opaque_open_dialog() {
    let (release, released) = mpsc::channel();
    let (done, result) = mpsc::channel();
    let dialogs = dialogs(thread::spawn(move || {
        released.recv_timeout(DEADLINE).unwrap();
        done.send(()).unwrap();
    }));
    drop(dialogs);
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release.send(()).unwrap();
    result.recv_timeout(DEADLINE).unwrap();
}
#[test]
fn prior_panicked_thread_is_reaped_and_shutdown_fails_closed() {
    let mut dialogs = dialogs(thread::spawn(|| panic!("test-only native dialog panic")));
    dialogs.pending = false;
    while !dialogs.worker.as_ref().unwrap().is_finished() {
        thread::yield_now();
    }
    // Exercise the same admission gate used by a new request, without OS UI.
    assert!(dialogs.ready_for_request());
    assert!(dialogs.failed && dialogs.worker.is_none());
    assert!(!dialogs.pending());
    // Historical diagnostics must not permanently disable later admission.
    assert!(dialogs.ready_for_request());
    assert_eq!(
        dialogs.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
}

#[test]
fn finished_panic_before_reply_revokes_exact_save_owner_and_allows_future_admission() {
    let mut dialogs = dialogs(thread::spawn(|| {
        panic!("test-only panic before dialog reply")
    }));
    dialogs.active_kind = Some(Kind::SaveAs);
    while !dialogs.worker.as_ref().unwrap().is_finished() {
        thread::yield_now();
    }
    assert!(dialogs.pending());
    let cancelled = dialogs
        .poll()
        .expect("finished worker must release dialog owner");
    assert_eq!(cancelled.kind, Kind::SaveAs);
    assert!(cancelled.path.is_none());
    assert!(!dialogs.pending());
    assert!(dialogs.poll().is_none());
    assert!(dialogs.ready_for_request());
    assert!(dialogs.failed);
    assert_eq!(
        dialogs.shutdown().unwrap_err().code,
        ErrorCode::InvariantViolation
    );
}
