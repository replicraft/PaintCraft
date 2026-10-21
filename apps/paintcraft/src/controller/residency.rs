//! Storage-only preparation has its own owner slot; it is never an unsaved edit.
use super::*;
use background::Completion;
use paintcraft_core::types::{Error, ErrorCode};
use std::time::{Duration, Instant};
impl Editor {
    pub(crate) fn has_pending_maintenance(&self) -> bool {
        self.maintenance.is_some()
    }
    pub(crate) fn start_residency_maintenance(&mut self) -> bool {
        self.poll_residency_maintenance();
        if self.maintenance.is_some()
            || self.pending_edit.is_some()
            || self.file_barrier.is_some()
            || self
                .actions
                .iter()
                .any(|action| matches!(action, background::Queued::FileBarrier(_)))
            || self.residency_attempt.is_some_and(|(ticket, at)| {
                ticket == self.render_key() && at.elapsed() < Duration::from_secs(5)
            })
        {
            return false;
        }
        let Some(service) = self.editing.clone() else {
            return false;
        };
        if !service.available() {
            return false;
        }
        self.residency_attempt = Some((self.render_key(), Instant::now()));
        let result = self.session.preparation().and_then(|preparation| {
            service.submit(
                move |cancellation| {
                    preparation
                        .maintain_history(|| cancellation.check())
                        .map(Completion::Maintenance)
                },
                String::new(),
                "",
                None,
            )
        });
        match result {
            Ok(pending) => {
                self.maintenance = Some(pending);
                true
            }
            Err(error) => {
                self.status = format!("History memory cleanup not prepared: {error}");
                false
            }
        }
    }
    pub(crate) fn poll_residency_maintenance(&mut self) -> bool {
        let Some(result) = self
            .maintenance
            .as_ref()
            .and_then(background::Pending::poll)
        else {
            return false;
        };
        self.maintenance.take();
        let result = match result {
            Ok(Completion::Maintenance(prepared)) => self
                .session
                .publish_history_maintenance(prepared, || Ok(())),
            Ok(_) => Err(Error::new(
                ErrorCode::InvariantViolation,
                "edit result routed to maintenance consumer",
            )),
            Err(error) => Err(error),
        };
        match result {
            Ok(report) if report.made_progress() => {
                // One maintenance step yields to already accepted foreground work.
                self.residency_attempt = if self.has_pending_edits() {
                    Some((self.render_key(), Instant::now()))
                } else {
                    None
                };
            }
            Ok(_) => self.residency_attempt = Some((self.render_key(), Instant::now())),
            Err(error) => {
                self.residency_attempt = Some((self.render_key(), Instant::now()));
                self.status = format!("History memory cleanup not published: {error}");
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_only_work_is_clean_and_no_progress_has_a_retry_backoff() {
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let mut editor = Editor::new_document(8, 8).unwrap();
        editor.connect_editing(service);
        let ticket = editor.render_key();
        let view = editor.history_view();
        assert!(editor.start_residency_maintenance());
        assert!(editor.has_pending_maintenance());
        assert!(!editor.is_dirty());
        assert!(!editor.has_pending_edits());
        let deadline = Instant::now() + Duration::from_secs(10);
        while editor.has_pending_maintenance() {
            assert!(Instant::now() < deadline);
            editor.run_pending();
            std::thread::yield_now();
        }
        assert_eq!(editor.render_key(), ticket);
        assert_eq!(editor.history_view(), view);
        assert!(!editor.is_dirty());
        assert!(!editor.start_residency_maintenance());
    }
}
