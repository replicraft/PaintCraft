//! Fixed-size evidence for the owned native host's ordered service barrier.
//! Created only from an actual successful Session publication, never inferred
//! from a changed ticket. The host must still prove its initiating source edge.
use super::*;
use paintcraft_editor::session::{Receipt, Ticket};
#[derive(Clone, Copy, Debug)]
pub struct Witness {
    owner: Uuid,
    before: Ticket,
    state: Uuid,
    publication: u64,
    history: Option<Uuid>,
    committed: bool,
}
impl Witness {
    pub fn owner(&self) -> Uuid {
        self.owner
    }
    pub fn before(&self) -> Ticket {
        self.before
    }
    pub fn state(&self) -> Uuid {
        self.state
    }
    pub fn publication(&self) -> u64 {
        self.publication
    }
    pub fn history(&self) -> Option<Uuid> {
        self.history
    }
    pub fn committed(&self) -> bool {
        self.committed
    }
}
impl Editor {
    /// Hosts clear immediately before their exact tagged activation and consume
    /// after actual completion. History navigation has a distinct result and is
    /// deliberately not relabeled as an authored edit Receipt.
    pub fn take_native_publication(&mut self) -> Option<Witness> {
        self.native_publication.take()
    }
    pub(super) fn record_native_publication(&mut self, before: Ticket, receipt: &Receipt) {
        self.native_publication = Some(Witness {
            owner: self.session.owner_id(),
            before,
            state: receipt.state_id,
            publication: receipt.publication_epoch,
            history: receipt.history_id,
            committed: receipt.committed,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_authored_receipt_is_consumed_once_and_navigation_is_not_fabricated() {
        let mut editor = Editor::new_document(8, 8).unwrap();
        let before = editor.session.ticket();
        let owner = editor.session.owner_id();
        assert!(editor.take_native_publication().is_none());
        editor.request(1, Action::AddLayer);
        editor.run_pending();
        let witness = editor
            .take_native_publication()
            .expect("actual commit receipt");
        assert_eq!(witness.owner(), owner);
        assert_eq!(witness.before(), before);
        assert_eq!(witness.state(), editor.session.ticket().state_id);
        assert_eq!(witness.publication(), before.publication_epoch + 1);
        assert!(witness.committed());
        assert!(witness.history().is_some());
        assert!(editor.take_native_publication().is_none());
        editor.request(2, Action::Undo);
        editor.run_pending();
        assert_eq!(editor.publications(), witness.publication() + 1);
        assert!(editor.take_native_publication().is_none());
    }
}

#[cfg(test)]
mod asynchronous {
    use super::*;
    use paintcraft_core::types::resources::SharedBudget;
    use paintcraft_input::{Modifiers, tablet::*};
    fn complete(editor: &mut Editor) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            editor.run_pending();
            if !editor.has_pending_edits() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "actual edit service did not complete"
            );
            std::thread::yield_now();
        }
    }
    fn sample(editor: &Editor, sequence: u64, phase: Phase) -> Sample {
        Sample {
            identity: Identity {
                device: 1,
                pointer: 2,
                kind: DeviceKind::Pen,
            },
            sequence,
            time: Time {
                micros: sequence * 100,
                epoch: 1,
                clock: Clock::HostReceipt,
            },
            phase,
            position: (3.0, 4.0),
            transform: Transform {
                owner: editor.session.owner_id().as_u128(),
                epoch: editor.publications(),
                origin: (0.0, 0.0),
                points_per_pixel: 1.0,
            },
            buttons: Buttons(if phase == Phase::Up { 0 } else { 1 }),
            modifiers: Modifiers::default(),
            pressure: Some(0.7),
            tilt: None,
            twist_degrees: None,
            contact: None,
            provenance: Provenance::Actual,
        }
    }
    #[test]
    fn actual_prepared_edit_and_pen_edit_only_expose_receipts_after_publication() {
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let mut editor = Editor::new_document(16, 16).unwrap();
        editor.connect_editing(service.clone());
        let before = editor.session.ticket();
        let release = service.pause();
        editor.request(1, Action::AddLayer);
        editor.run_pending();
        assert!(editor.take_native_publication().is_none());
        release.send(()).unwrap();
        complete(&mut editor);
        let edit = editor
            .take_native_publication()
            .expect("actual prepared Edit receipt");
        assert_eq!(edit.before(), before);
        assert!(edit.committed());
        assert!(edit.history().is_some());
        editor.select_tool(Tool::Brush);
        editor.options.size = 5;
        editor.color = Rgba([31, 79, 151, 255]);
        let budget = SharedBudget::new(16 << 20);
        let before = editor.session.ticket();
        for (seq, phase) in [(1, Phase::Down), (2, Phase::Up)] {
            let s = sample(&editor, seq, phase);
            assert!(editor.pen_sample(s, &budget).unwrap());
        }
        let release = service.pause();
        editor.run_pending();
        assert!(editor.take_native_publication().is_none());
        release.send(()).unwrap();
        complete(&mut editor);
        let pen = editor
            .take_native_publication()
            .expect("actual prepared PenEdit receipt");
        assert_eq!(pen.before(), before);
        assert!(pen.committed());
        assert!(pen.history().is_some());
        assert_eq!(pen.publication(), before.publication_epoch + 1);
        assert_eq!(pen.state(), editor.session.ticket().state_id);
    }
    #[test]
    fn noop_and_error_do_not_fabricate_committed_history_or_a_receipt() {
        let service = background::Service::new(eframe::egui::Context::default()).unwrap();
        let mut editor = Editor::new_document(8, 8).unwrap();
        editor.connect_editing(service);
        let before = editor.session.ticket();
        editor.request(
            1,
            Action::SetOpacity {
                layer: editor.target().unwrap(),
                opacity: 255,
            },
        );
        complete(&mut editor);
        let noop = editor
            .take_native_publication()
            .expect("actual no-change receipt");
        assert!(!noop.committed());
        assert_eq!(noop.before(), before);
        assert_eq!(noop.state(), before.state_id);
        assert_eq!(noop.publication(), before.publication_epoch);
        assert!(noop.history().is_none());
        editor.request(
            2,
            Action::SetOpacity {
                layer: Uuid::new_v4(),
                opacity: 127,
            },
        );
        complete(&mut editor);
        assert!(editor.take_native_publication().is_none());
        assert_eq!(editor.session.ticket(), before);
    }
}
