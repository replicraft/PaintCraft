//! Fixed-size evidence for the owned native host's ordered service barrier.
//! Created only from an actual successful Session publication, never inferred
//! from a changed ticket. The host must still prove its initiating source edge.
use super::*;
#[path = "native_publication/outbox.rs"]
mod outbox;
pub(super) use outbox::{Activations, QueuedEdit};
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_editor::session::{Receipt, Ticket};
/// Actual native source installation/ordinal and document session. This is not
/// a generated operation ID. Only the owned host creates an activation scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activation {
    session: u128,
    source_epoch: u64,
    sequence: u64,
}
impl Activation {
    pub(crate) fn new(session: u128, source_epoch: u64, sequence: u64) -> Self {
        Self {
            session,
            source_epoch,
            sequence,
        }
    }
}
/// Actual source lifetime plus a sealed installation authority. Activation alone
/// only labels ownership; it never authorizes prepared publication.
#[derive(Clone, Debug)]
pub(crate) struct Scope {
    pub(super) activation: Activation,
    source: paintcraft_ui::pen_input::PublicationGuard,
    delivery: Option<paintcraft_platform::tablet::DeliveryCheck>,
}
impl Scope {
    pub(crate) fn new(
        activation: Activation,
        source: paintcraft_ui::pen_input::PublicationGuard,
        delivery: Option<paintcraft_platform::tablet::DeliveryCheck>,
    ) -> Self {
        Self {
            activation,
            source,
            delivery,
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        let current = || self.source.is_current();
        let valid = match &self.delivery {
            Some(delivery) => delivery.authorize(current),
            // Headless controls exercise the real Source guard separately from
            // adapter gate controls. Runtime never has a synthetic Ready handle.
            #[cfg(test)]
            None => current(),
            #[cfg(not(test))]
            None => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Native source retired before prepared publication",
            ))
        }
    }
}
pub(super) fn check(scope: Option<&Scope>) -> Result<()> {
    scope.map_or(Ok(()), Scope::check)
}
#[derive(Clone, Copy, Debug)]
pub struct Witness {
    owner: Uuid,
    before: Ticket,
    state: Uuid,
    publication: u64,
    history: Option<Uuid>,
    committed: bool,
    completion: Option<ContactCompletion>,
    activation: Option<Activation>,
}
/// Endpoint copied from the actual retained PenStroke published by this job.
/// This is source provenance, not an unrelated generated operation identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactCompletion {
    identity: paintcraft_input::tablet::Identity,
    sequence: u64,
    capture_owner: u128,
}
impl ContactCompletion {
    pub fn identity(&self) -> paintcraft_input::tablet::Identity {
        self.identity
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn capture_owner(&self) -> u128 {
        self.capture_owner
    }
    pub(super) fn from_last(sample: &paintcraft_input::tablet::Sample) -> Option<Self> {
        (sample.phase == paintcraft_input::tablet::Phase::Up).then_some(Self {
            identity: sample.identity,
            sequence: sample.sequence,
            capture_owner: sample.transform.owner,
        })
    }
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
    pub fn contact_completion(&self) -> Option<ContactCompletion> {
        self.completion
    }
    pub fn activation(&self) -> Option<Activation> {
        self.activation
    }
}
/// Actual history navigation is deliberately distinct from an authored Receipt.
#[derive(Clone, Copy, Debug)]
pub struct NavigationWitness {
    owner: Uuid,
    before: Ticket,
    state: Uuid,
    publication: u64,
    history: Option<Uuid>,
    activation: Option<Activation>,
}
impl NavigationWitness {
    pub fn activation(&self) -> Option<Activation> {
        self.activation
    }
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
}
impl Editor {
    pub(crate) fn begin_native_activation(&mut self, activation: Scope) {
        self.native_activation = Some(activation);
    }
    pub(crate) fn end_native_activation(&mut self) {
        self.native_activation = None;
    }
    pub(crate) fn has_native_work(&self, activation: Activation) -> bool {
        self.outbox.has(activation) || self.actions.iter().any(|queued| matches!(queued,
            background::Queued::Edit(edit) if edit.scope.as_ref().is_some_and(|s| s.activation == activation))) || self.pending_edit.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        }) || self.waiting_dispatch.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        }) || self.waiting_edit.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        })
    }
    /// Abandon only this precisely sourced job, including an already-ready
    /// result. Pending::drop retains the executor lease until worker retirement.
    pub(crate) fn abandon_native_activation(&mut self, activation: Activation) {
        let queued = self.outbox.has(activation) || self.actions.iter().any(|queued| matches!(queued,
            background::Queued::Edit(edit) if edit.scope.as_ref().is_some_and(|s| s.activation == activation)));
        self.outbox.retire(activation);
        self.actions.retain(|queued| {
            !matches!(queued, background::Queued::Edit(edit)
            if edit.scope.as_ref().is_some_and(|s| s.activation == activation))
        });
        let mut abandoned = queued;
        if self.pending_edit.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        }) {
            self.pending_edit.take();
            abandoned = true;
        }
        if self.waiting_dispatch.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        }) {
            self.waiting_dispatch.take();
            abandoned = true;
        }
        if self.waiting_edit.as_ref().is_some_and(|p| {
            p.native_activation
                .as_ref()
                .is_some_and(|scope| scope.activation == activation)
        }) {
            self.waiting_edit.take();
            abandoned = true;
        }
        if abandoned {
            self.publication_preview = None;
            self.native_publication = None;
            self.native_navigation = None;
            if let Some(service) = &self.editing {
                service.reap_abandoned();
            }
        }
    }
    pub(super) fn record_native_job_activation(&mut self, activation: Option<Activation>) {
        if let Some(w) = &mut self.native_publication {
            w.activation = activation;
        }
        if let Some(w) = &mut self.native_navigation {
            w.activation = activation;
        }
    }
    /// Hosts clear immediately before their exact tagged activation and consume
    /// after actual completion. History navigation has a distinct result and is
    /// deliberately not relabeled as an authored edit Receipt.
    pub fn take_native_publication(&mut self) -> Option<Witness> {
        self.native_publication.take()
    }
    pub(super) fn record_native_pen_publication(
        &mut self,
        before: Ticket,
        receipt: &Receipt,
        completion: Option<ContactCompletion>,
    ) {
        self.record_native_publication(before, receipt);
        if let Some(witness) = &mut self.native_publication {
            witness.completion = completion;
        }
    }
    pub fn take_native_navigation(&mut self) -> Option<NavigationWitness> {
        self.native_navigation.take()
    }
    pub(super) fn record_native_navigation(
        &mut self,
        before: Ticket,
        navigation: Option<&paintcraft_editor::session::Navigation>,
    ) {
        if let Some(navigation) = navigation {
            self.native_publication = None;
            self.native_navigation = Some(NavigationWitness {
                owner: self.session.owner_id(),
                before,
                state: navigation.state_id,
                publication: navigation.publication_epoch,
                history: navigation.history_id,
                activation: self
                    .native_activation
                    .as_ref()
                    .map(|scope| scope.activation),
            });
        }
    }
    pub(super) fn record_native_publication(&mut self, before: Ticket, receipt: &Receipt) {
        self.native_navigation = None;
        self.native_publication = Some(Witness {
            owner: self.session.owner_id(),
            before,
            state: receipt.state_id,
            publication: receipt.publication_epoch,
            history: receipt.history_id,
            committed: receipt.committed,
            completion: None,
            activation: self
                .native_activation
                .as_ref()
                .map(|scope| scope.activation),
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
        let navigation = editor
            .take_native_navigation()
            .expect("actual Undo Navigation");
        assert_eq!(navigation.owner(), owner);
        assert_eq!(navigation.before().state_id, witness.state());
        assert_eq!(navigation.state(), editor.session.ticket().state_id);
        assert_eq!(navigation.publication(), witness.publication() + 1);
        assert!(editor.take_native_navigation().is_none());
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
        assert!(edit.contact_completion().is_none());
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
        let endpoint = pen
            .contact_completion()
            .expect("published retained PenStroke endpoint");
        assert_eq!(endpoint.sequence(), 2);
        assert_eq!(endpoint.identity(), sample(&editor, 2, Phase::Up).identity);
        assert_eq!(
            endpoint.capture_owner(),
            editor.session.owner_id().as_u128()
        );
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
    #[test]
    fn actual_prepared_edit_and_pen_edit_recheck_lifetime_at_publication_without_a_host_poll() {
        use paintcraft_ui::pen_input::{Owner, Source, View};
        for pen in [false, true] {
            let service = background::Service::new(eframe::egui::Context::default()).unwrap();
            let mut editor = Editor::new_document(16, 16).unwrap();
            editor.connect_editing(service.clone());
            let before = editor.session.ticket();
            let depth = editor.undo_depth();
            let owner = Owner {
                session: editor.session.owner_id().as_u128(),
                state: before.state_id.as_u128(),
                publication: before.publication_epoch,
                interface: 1,
            };
            let budget = SharedBudget::new(16 << 20);
            let source = Source::new(16, 16, &budget).unwrap();
            let first = sample(&editor, 1, Phase::Down);
            source
                .publish(View {
                    owner,
                    transform: first.transform,
                    pixels_per_point: 1.0,
                    canvas: eframe::egui::Rect::from_min_max(
                        eframe::egui::pos2(0.0, 0.0),
                        eframe::egui::pos2(16.0, 16.0),
                    ),
                    exclusions: [None; 64],
                    focused: true,
                    blocked: false,
                    ime: false,
                })
                .unwrap();
            for (sequence, phase) in [(1, Phase::Down), (2, Phase::Up)] {
                let sample = sample(&editor, sequence, phase);
                source
                    .submit_with(receiver::Delivery::ReceiverPointer, |_| {
                        Ok(RawSample {
                            identity: sample.identity,
                            sequence,
                            time: sample.time,
                            phase,
                            logical_position: Some(sample.position),
                            buttons: sample.buttons,
                            modifiers: sample.modifiers,
                            pressure: sample.pressure,
                            tilt: sample.tilt,
                            twist_degrees: sample.twist_degrees,
                            contact: sample.contact,
                            provenance: sample.provenance,
                        })
                    })
                    .unwrap();
            }
            let batch = source.take(1, owner, false).unwrap();
            let guard = source.publication_guard(&batch, owner).unwrap();
            assert!(guard.is_current());
            editor.begin_native_activation(Scope::new(
                Activation::new(owner.session, 1, 2),
                guard,
                None,
            ));
            let release = service.pause();
            if pen {
                editor.select_tool(Tool::Brush);
                for (sequence, phase) in [(1, Phase::Down), (2, Phase::Up)] {
                    let sample = sample(&editor, sequence, phase);
                    assert!(editor.pen_sample(sample, &budget).unwrap());
                }
            } else {
                editor.request(77, Action::AddLayer);
            }
            editor.run_pending();
            editor.end_native_activation();
            release.send(()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while editor.pending_edit.is_none() {
                assert!(std::time::Instant::now() < deadline);
                editor.run_pending();
                std::thread::yield_now();
            }
            // No Host snapshot/retire call intervenes: the actual publication
            // callback itself must reject the now-invalid retained Source guard.
            source.cancel();
            complete(&mut editor);
            assert_eq!(editor.session.ticket(), before);
            assert_eq!(editor.undo_depth(), depth);
            assert!(editor.take_native_publication().is_none());
            assert_eq!(editor.status, "Edit cancelled");
        }
    }
}
