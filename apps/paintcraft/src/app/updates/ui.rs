//! About-page controls for the real signed update worker.
use super::{Action, State, Status};
use eframe::egui;
use paintcraft_ui::t;
use paintcraft_update::Plan;
use std::sync::atomic::Ordering;
impl State {
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) -> Action {
        self.poll(ui.ctx());
        ui.separator();
        ui.strong(t!(ui.ctx(), "Updates"));
        let mut action = Action::None;
        match &self.status {
            Status::Idle => {}
            Status::Checking => {
                ui.label(t!(ui.ctx(), "Checking for updates…"));
            }
            Status::Downloading => {
                ui.label(t!(ui.ctx(), "Downloading update…"));
            }
            Status::Current => {
                ui.label(t!(ui.ctx(), "Your version is up to date."));
            }
            Status::Available => {
                if let Some(available) = &self.available {
                    ui.label(t!(
                        ui.ctx(),
                        "Update available: { $version }",
                        &[("version", available.version.to_string())]
                    ));
                    if matches!(available.plan, Plan::Manual { .. }) {
                        ui.label(t!(ui.ctx(),"This installation is updated through its package manager or by installing a new copy."));
                    }
                }
            }
            Status::Ready => {
                if let Some(verified) = &self.verified {
                    ui.label(t!(
                        ui.ctx(),
                        "Verified update ready: { $version }",
                        &[("version", verified.version().to_string())]
                    ));
                    if ui.button(t!(ui.ctx(), "Restart to update")).clicked() {
                        action = Action::Restart;
                    }
                }
            }
            Status::Cancelled => {
                ui.label(t!(ui.ctx(), "Update cancelled."));
            }
            Status::Unconfigured => {
                ui.label(t!(ui.ctx(), "Updates are unavailable in this build."));
            }
            Status::Failed(detail) => {
                ui.label(t!(
                    ui.ctx(),
                    "Update failed: { $detail }",
                    &[("detail", detail.clone())]
                ));
            }
            Status::TaskEnded => {
                ui.label(t!(ui.ctx(), "Update task ended unexpectedly."));
            }
            Status::HandedOff => {
                ui.label(t!(ui.ctx(), "Restarting to update…"));
            }
        }
        // A coordinator may refuse while work is active without consuming the
        // verified owner. Keep its retry action available after that refusal.
        if self.verified_ready()
            && !matches!(self.status, Status::Ready)
            && ui.button(t!(ui.ctx(), "Restart to update")).clicked()
        {
            action = Action::Restart;
        }
        if let Some(pending) = &self.pending {
            if pending.total != 0 {
                let n = pending.received.load(Ordering::Relaxed).min(pending.total);
                ui.add(egui::ProgressBar::new(n as f32 / pending.total as f32).show_percentage());
            }
            if pending.cancelled.load(Ordering::Relaxed) {
                ui.label(t!(ui.ctx(), "Cancelling update…"));
            } else if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                self.cancel();
            }
        } else if !matches!(self.status, Status::Ready | Status::HandedOff) {
            if ui.button(t!(ui.ctx(), "Check for updates")).clicked() {
                self.start_check(ui.ctx());
            }
            let downloadable = matches!(self.status, Status::Available)
                && self
                    .available
                    .as_ref()
                    .is_some_and(|a| matches!(a.plan, Plan::InPlace(_)));
            if downloadable && ui.button(t!(ui.ctx(), "Download update")).clicked() {
                self.start_download(ui.ctx());
            }
        }
        if let Some(available) = &self.available {
            ui.hyperlink_to(t!(ui.ctx(), "Release notes"), &available.release_url);
        }
        action
    }
}
