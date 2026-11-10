//! Owned desktop update checks/downloads; installation is a separate save-and-drain coordinator.
use crate::App;
use eframe::egui;
use paintcraft_update::{Available, Error, InstallLock, Progress, Updater, Verified};
use std::{
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
    time::Duration,
};
#[path = "updates/ui.rs"]
mod ui;

#[derive(Debug, Default)]
pub(super) struct State {
    status: Status,
    available: Option<Available>,
    verified: Option<Verified>,
    pending: Option<Pending>,
}
#[derive(Debug, Default)]
enum Status {
    #[default]
    Idle,
    Checking,
    Downloading,
    Current,
    Available,
    Ready,
    Cancelled,
    Unconfigured,
    Failed(String),
    TaskEnded,
    HandedOff,
}
#[derive(Debug)]
enum Task {
    Check,
    Download(Box<Available>),
}
#[derive(Debug)]
enum Completion {
    Checked(paintcraft_update::Result<Option<Available>>),
    Downloaded(paintcraft_update::Result<Verified>),
}
#[derive(Debug)]
struct Pending {
    receiver: Receiver<Completion>,
    worker: Option<JoinHandle<()>>,
    cancelled: Arc<AtomicBool>,
    received: Arc<AtomicU64>,
    total: u64,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Action {
    #[default]
    None,
    Restart,
}

impl State {
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn verified_ready(&self) -> bool {
        self.pending.is_none() && self.verified.is_some()
    }
    pub(super) fn take_verified_for_apply(&mut self) -> Option<Verified> {
        if self.pending.is_some() {
            return None;
        }
        let verified = self.verified.take();
        if verified.is_some() {
            self.status = Status::HandedOff;
        }
        verified
    }
    pub(super) fn set_restart_error(&mut self, detail: String) {
        self.status = Status::Failed(detail);
    }
    pub(super) fn cancel(&self) {
        if let Some(pending) = &self.pending {
            pending.cancelled.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        if !pending.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        // Only join a finished worker: network/file I/O never blocks the UI thread.
        let Some(pending) = self.pending.take() else {
            return;
        };
        let cancelled = pending.cancelled.load(Ordering::Relaxed);
        let result = pending.receiver.try_recv();
        // Pending has Drop, so consume the handle through a dedicated destructuring helper.
        let joined = pending.join_finished();
        if cancelled {
            self.verified = None;
            self.status = Status::Cancelled;
            return;
        }
        if !joined {
            self.status = Status::TaskEnded;
            return;
        }
        match result {
            Ok(Completion::Checked(Ok(Some(available)))) => {
                self.available = Some(available);
                self.status = Status::Available;
            }
            Ok(Completion::Checked(Ok(None))) => self.status = Status::Current,
            Ok(Completion::Downloaded(Ok(verified))) => {
                self.verified = Some(verified);
                self.status = Status::Ready;
            }
            Ok(Completion::Checked(Err(error)) | Completion::Downloaded(Err(error))) => {
                self.fail(error)
            }
            Err(_) => self.status = Status::TaskEnded,
        }
    }
    fn fail(&mut self, error: Error) {
        self.status = match error {
            Error::SigningKeyUnconfigured => Status::Unconfigured,
            Error::Cancelled => Status::Cancelled,
            error => Status::Failed(error.to_string()),
        };
    }
    fn start_check(&mut self, ctx: &egui::Context) {
        if self.busy() || matches!(self.status, Status::HandedOff) {
            return;
        }
        self.available = None;
        self.verified = None;
        self.start(Task::Check, ctx);
    }
    fn start_download(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        if let Some(available) = self.available.clone() {
            self.start(Task::Download(Box::new(available)), ctx);
        }
    }
    fn start(&mut self, task: Task, ctx: &egui::Context) {
        let total = match &task {
            Task::Check => 0,
            Task::Download(a) => a.asset_size.unwrap_or(0),
        };
        self.status = match &task {
            Task::Check => Status::Checking,
            Task::Download(_) => Status::Downloading,
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&cancelled);
        let received = Arc::new(AtomicU64::new(0));
        let progress = Arc::clone(&received);
        let wake = ctx.clone();
        let worker = std::thread::Builder::new()
            .name("paintcraft-update".into())
            .spawn(move || {
                let result = run(task, &cancel, &progress, &wake);
                let _ = sender.send(result);
                wake.request_repaint();
            });
        match worker {
            Ok(worker) => {
                self.pending = Some(Pending {
                    receiver,
                    worker: Some(worker),
                    cancelled,
                    received,
                    total,
                })
            }
            Err(error) => self.status = Status::Failed(error.to_string()),
        }
    }
}
impl Pending {
    fn join_finished(mut self) -> bool {
        self.worker
            .take()
            .is_some_and(|worker| worker.join().is_ok())
    }
}

fn run(task: Task, cancel: &AtomicBool, received: &AtomicU64, ctx: &egui::Context) -> Completion {
    match task {
        Task::Check => Completion::Checked((|| {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let result = Updater::detect(env!("CARGO_PKG_VERSION"))?.check();
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            result
        })()),
        Task::Download(available) => Completion::Downloaded((|| {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let updater = Updater::detect(env!("CARGO_PKG_VERSION"))?;
            updater.download(&available, &mut |progress: Progress| {
                received.store(progress.received, Ordering::Relaxed);
                ctx.request_repaint();
                if cancel.load(Ordering::Relaxed) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
        })()),
    }
}
impl App {
    /// Retains the startup lease already acquired before documents or worker services existed.
    pub fn with_installation_lock(mut self, lock: Option<InstallLock>) -> Self {
        self.installation_lock = lock;
        self
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(installation) = self.installation_lock.take() {
            // App teardown can leave cancelled worker callbacks alive. Keep this
            // one startup file lock until the OS closes the process descriptors;
            // releasing it at App drop would permit a racing installer too early.
            std::mem::forget(installation);
        }
    }
}

#[cfg(test)]
#[path = "updates/tests.rs"]
mod tests;
