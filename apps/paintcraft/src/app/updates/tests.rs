use super::*;
use std::sync::Barrier;

fn finished(result: Completion, cancelled: bool) -> Pending {
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        sender.send(result).unwrap();
    });
    while !worker.is_finished() {
        std::thread::yield_now();
    }
    Pending {
        receiver,
        worker: Some(worker),
        cancelled: Arc::new(AtomicBool::new(cancelled)),
        received: Arc::new(AtomicU64::new(0)),
        total: 0,
    }
}
#[test]
fn state_default_and_about_controls_do_not_create_threads_or_install_authority() {
    let mut state = State::default();
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(Default::default(), |ui| {
        assert_eq!(state.show(ui), Action::None);
    });
    output.textures_delta.clear();
    assert!(!state.busy());
    assert!(!state.verified_ready());
    assert!(state.take_verified_for_apply().is_none());
}
#[test]
fn cancellation_discards_a_late_success_instead_of_reporting_current() {
    let mut state = State {
        pending: Some(finished(Completion::Checked(Ok(None)), true)),
        status: Status::Checking,
        ..Default::default()
    };
    state.poll(&egui::Context::default());
    assert!(matches!(state.status, Status::Cancelled));
    assert!(!state.busy());
    assert!(state.available.is_none());
}
#[test]
fn normal_check_completion_is_owned_and_joined_without_a_pending_worker() {
    let mut state = State {
        pending: Some(finished(Completion::Checked(Ok(None)), false)),
        status: Status::Checking,
        ..Default::default()
    };
    state.poll(&egui::Context::default());
    assert!(matches!(state.status, Status::Current));
    assert!(!state.busy());
}
#[test]
fn an_active_task_prevents_another_check_and_can_be_cancelled() {
    let barrier = Arc::new(Barrier::new(2));
    let wait = Arc::clone(&barrier);
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        wait.wait();
        drop(sender);
    });
    let mut state = State {
        pending: Some(Pending {
            receiver,
            worker: Some(worker),
            cancelled: Arc::new(AtomicBool::new(false)),
            received: Arc::new(AtomicU64::new(0)),
            total: 0,
        }),
        status: Status::Checking,
        ..Default::default()
    };
    state.start_check(&egui::Context::default());
    state.cancel();
    assert!(
        state
            .pending
            .as_ref()
            .unwrap()
            .cancelled
            .load(Ordering::Relaxed)
    );
    barrier.wait();
    while !state
        .pending
        .as_ref()
        .unwrap()
        .worker
        .as_ref()
        .unwrap()
        .is_finished()
    {
        std::thread::yield_now();
    }
    state.poll(&egui::Context::default());
    assert!(matches!(state.status, Status::Cancelled));
}
#[test]
fn a_disconnected_worker_is_a_visible_terminal_failure() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || drop(sender));
    while !worker.is_finished() {
        std::thread::yield_now();
    }
    let mut state = State {
        pending: Some(Pending {
            receiver,
            worker: Some(worker),
            cancelled: Arc::new(AtomicBool::new(false)),
            received: Arc::new(AtomicU64::new(0)),
            total: 0,
        }),
        ..Default::default()
    };
    state.poll(&egui::Context::default());
    assert!(matches!(state.status, Status::TaskEnded));
    assert!(!state.busy());
}
#[test]
fn status_alone_cannot_manufacture_verified_restart_ownership() {
    let mut state = State {
        status: Status::Ready,
        ..Default::default()
    };
    assert!(!state.verified_ready());
    assert!(state.take_verified_for_apply().is_none());
    state.fail(Error::SigningKeyUnconfigured);
    assert!(matches!(state.status, Status::Unconfigured));
}
