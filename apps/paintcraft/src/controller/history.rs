//! History presentation reads metadata; navigation restores recorded snapshots.
use super::*;
use paintcraft_editor::session::{HistoryTicket, HistoryView, Ticket};

#[cfg(test)]
#[path = "history/tests.rs"]
mod tests;

impl Editor {
    pub fn history_view(&self) -> HistoryView {
        self.session.history_view()
    }

    pub(super) fn navigate_history(
        &mut self,
        owner: Uuid,
        state_id: Uuid,
        epoch: u64,
        history_id: Option<Uuid>,
    ) {
        if self.has_image_paste() {
            self.status = "Finish placing the pasted image before editing or saving.".into();
            return;
        }
        if self.start_navigation(background::NavigationRequest::Seek {
            ticket: HistoryTicket {
                owner,
                base: Ticket {
                    state_id,
                    publication_epoch: epoch,
                },
            },
            history_id,
        }) {
            return;
        }
        match self.session.navigate_history(
            HistoryTicket {
                owner,
                base: Ticket {
                    state_id,
                    publication_epoch: epoch,
                },
            },
            history_id,
        ) {
            Ok(Some(_)) => {
                self.cancel_gesture();
                self.retarget();
                self.status = "History restored".into();
            }
            Ok(None) => {}
            Err(error) => self.status = format!("History not changed: {error}"),
        }
    }
}
