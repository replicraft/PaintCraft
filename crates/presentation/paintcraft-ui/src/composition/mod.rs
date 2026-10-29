//! App-private composition transaction for owned single-line String widgets.
//! Provisional text stays visible in the actual egui TextEdit. Only an actual
//! IME result advances the editable draft; focus loss rolls back provisional
//! replacement and its local undo history. This is not a native IME registry.
mod history;
mod prepare;
mod reduce;
mod snapshot;
use egui::{
    Event, Id, ImeEvent, Ui,
    text::CCursorRange,
    text_edit::{TextEditOutput, TextEditState},
};
use history::Bound;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use reduce::Reduced;
use snapshot::Snapshot;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub session: u128,
    pub state: u128,
    pub publication: u64,
    pub interface: u8,
}
/// One owner from the first mount; do not clone or reuse across semantic fields.
pub struct Field {
    namespace: Option<u64>,
    mounted_id: Option<Id>,
    restored_state: Option<TextEditState>,
    revision: Option<u64>,
    last_frame: Option<u64>,
    retired_frame: Option<u64>,
    completed_owner: Option<Owner>,
    mounted: bool,
    trusted: bool,
    notice: bool,
    bound: Bound,
    active: Option<Active>,
}
struct Active {
    owner: Owner,
    baseline: String,
    working: String,
    baseline_cursor: CCursorRange,
    composing: bool,
    // Strings above drop before the retained admission below.
    snapshot: Snapshot,
}
impl Default for Field {
    fn default() -> Self {
        Self {
            namespace: NEXT
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .ok(),
            mounted_id: None,
            restored_state: None,
            revision: Some(0),
            last_frame: None,
            retired_frame: None,
            completed_owner: None,
            mounted: false,
            trusted: true,
            notice: false,
            bound: Bound::default(),
            active: None,
        }
    }
}
#[derive(Debug)]
pub struct Request<'a> {
    pub host_id: Id,
    pub owner: Owner,
    pub cancel: bool,
    pub budget: &'a SharedBudget,
}
pub struct Output {
    pub editor: TextEditOutput,
    pub refused: bool,
    pub cancelled: bool,
    /// An exact owned interval ended through explicit window focus loss.
    pub finished: bool,
}
impl Field {
    pub fn id(&self, host: Id) -> Id {
        host.with(("private-composition-v1", self.namespace))
    }
    pub fn revision(&self) -> Option<(u64, u64)> {
        self.namespace.zip(self.revision)
    }
    pub fn retire(&mut self, ctx: &egui::Context) {
        if let Some(active) = self.active.take() {
            if let Some(id) = self.mounted_id {
                let state = active.snapshot.restore();
                self.restored_state = Some(state.clone());
                state.store(ctx, id);
            }
            self.revision = self.revision.and_then(|n| n.checked_add(1));
            ctx.request_repaint();
        }
    }
    fn rollback(&mut self, ui: &Ui, _id: Id) {
        self.retire(ui.ctx());
    }

    /// `show` must render the actual owned singleline String TextEdit using id.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        draft: &mut String,
        request: Request<'_>,
        show: impl FnOnce(&mut Ui, &mut String, Id) -> TextEditOutput,
    ) -> Output {
        let Request {
            host_id,
            owner,
            cancel,
            budget,
        } = request;
        let id = self.id(host_id);
        self.mounted_id = Some(id);
        self.last_frame = Some(ui.ctx().cumulative_frame_nr());
        if !self.mounted {
            self.trusted = self.namespace.is_some() && TextEditState::load(ui.ctx(), id).is_none();
            self.mounted = true;
        }
        let stale = self.active.as_ref().is_some_and(|a| a.owner != owner)
            || self
                .completed_owner
                .take()
                .is_some_and(|stamp| stamp != owner);
        if cancel || stale {
            self.rollback(ui, id);
        }
        if stale {
            self.restored_state = None;
        }
        let focused = ui.memory(|m| m.has_focus(id));
        let retired_frame = self.retired_frame == Some(ui.ctx().cumulative_frame_nr());
        let has_ime = focused
            && !retired_frame
            && ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Ime(_))));
        // Do not implement a second ordinary text editor. Mixed IME/ordinary
        // mutation batches refuse their IME part explicitly; ordinary events
        // still run through the existing widget and retain their history.
        let ordinary = focused
            && ui.input(|i| {
                i.events.iter().any(|e| match e {
                    Event::Text(_)
                    | Event::Paste(_)
                    | Event::MiddleClickPaste { .. }
                    | Event::Cut => true,
                    Event::AccessKitActionRequest(request) => {
                        request.target_node == id.accesskit_id()
                            && matches!(
                                request.action,
                                egui::accesskit::Action::ReplaceSelectedText
                                    | egui::accesskit::Action::SetValue
                            )
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => match key {
                        egui::Key::Backspace => true,
                        egui::Key::Delete => {
                            !modifiers.shift || ui.os() != egui::os::OperatingSystem::Windows
                        }
                        egui::Key::H | egui::Key::K | egui::Key::U | egui::Key::W => modifiers.ctrl,
                        egui::Key::Y => modifiers.matches_logically(egui::Modifiers::COMMAND),
                        egui::Key::Z => {
                            modifiers.matches_logically(egui::Modifiers::COMMAND)
                                || modifiers.matches_logically(
                                    egui::Modifiers::SHIFT | egui::Modifiers::COMMAND,
                                )
                        }
                        _ => false,
                    },
                    _ => false,
                })
            });
        if ordinary
            && !has_ime
            && ui.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, Event::Text(_) | Event::Paste(_)))
            })
        {
            self.notice = false;
        }
        if ordinary && self.active.is_some() {
            self.rollback(ui, id);
        }
        if has_ime {
            self.revision = self.revision.and_then(|n| n.checked_add(1));
        }
        let mut refused =
            has_ime && (!self.trusted || self.revision.is_none() || ordinary || cancel || stale);
        {
            // Count the raw batch before the widget can acquire focus during
            // its own pointer handling. This intentionally overcounts other
            // input rather than missing an insertion/undo on the first click.
            let text_bytes = self
                .active
                .as_ref()
                .map_or(draft.capacity(), |a| a.working.capacity());
            let next = ui.input(|i| {
                let mut next = self.bound;
                next.observe(text_bytes, &i.events).map(|()| next)
            });
            // Budget callbacks may reenter Context, so resize only after the
            // pure input read above has released its Context guard.
            let observed = next.and_then(|next| match &mut self.active {
                Some(active) => active.snapshot.admit_observed(&mut self.bound, next),
                None => {
                    self.bound = next;
                    Ok(())
                }
            });
            if observed.is_err() {
                refused = has_ime;
                self.trusted = false;
                self.rollback(ui, id);
            }
        }
        let mut event_storage = budget.account(AllocationClass::PaintMetadata);
        let events = if has_ime && !refused {
            let bytes = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Ime(event) => Some(
                            std::mem::size_of::<ImeEvent>() as u64
                                + match event {
                                    ImeEvent::Preedit { text, .. } | ImeEvent::Commit(text) => {
                                        text.len() as u64
                                    }
                                    _ => 0,
                                },
                        ),
                        _ => None,
                    })
                    .try_fold(0_u64, |sum, n| sum.checked_add(n))
            });
            if bytes.is_none_or(|bytes| event_storage.resize(bytes).is_err()) {
                refused = true;
                Vec::new()
            } else {
                let mut events = Vec::new();
                let count = ui.input(|i| {
                    i.events
                        .iter()
                        .filter(|e| matches!(e, Event::Ime(_)))
                        .count()
                });
                if events.try_reserve_exact(count).is_err() {
                    refused = true;
                }
                if !refused {
                    let exact = ((events.capacity() - count) as u64)
                        .checked_mul(std::mem::size_of::<ImeEvent>() as u64)
                        .and_then(|extra| bytes.and_then(|base| base.checked_add(extra)));
                    if exact.is_none_or(|bytes| event_storage.resize(bytes).is_err()) {
                        refused = true;
                    }
                }
                if !refused {
                    ui.input(|i| {
                        events.extend(i.events.iter().filter_map(|e| match e {
                            Event::Ime(event) => Some(event.clone()),
                            _ => None,
                        }))
                    });
                }
                events
            }
        } else {
            Vec::new()
        };
        if has_ime && !refused && self.active.is_none() {
            let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
            match Snapshot::take(&mut state, self.bound, budget) {
                Ok(snapshot) => {
                    self.restored_state = None;
                    let fallback = Reduced::at_end(draft.clone()).cursor;
                    let cursor = state.cursor.char_range().unwrap_or(fallback);
                    state.store(ui.ctx(), id);
                    self.active = Some(Active {
                        owner,
                        baseline: draft.clone(),
                        working: draft.clone(),
                        baseline_cursor: cursor,
                        composing: false,
                        snapshot,
                    });
                }
                Err(_) => refused = true,
            }
        }
        if refused {
            self.rollback(ui, id);
            // This field is the focused IME owner. Refusal must not pass those
            // same provisional records into its ordinary authoritative buffer.
            ui.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Ime(_))));
        }
        let mut reduced = self.active.as_ref().map(|active| Reduced {
            text: active.working.clone(),
            cursor: TextEditState::load(ui.ctx(), id)
                .and_then(|s| s.cursor.char_range())
                .unwrap_or(active.baseline_cursor),
            composing: active.composing,
            result: None,
        });
        if let Some(reduced) = &mut reduced {
            for event in &events {
                reduced.apply(event);
            }
        }
        // A retired old owner must not see post-boundary IME records that
        // remain available to the newly focused widget. Moving the raw Vec
        // needs no allocation; InputState's pointer snapshot remains intact.
        let hidden = retired_frame.then(|| ui.input_mut(|i| std::mem::take(&mut i.events)));
        let editor = match &mut self.active {
            Some(active) => show(ui, &mut active.working, id),
            None => show(ui, draft, id),
        };
        if let Some(events) = hidden {
            ui.input_mut(|i| i.events = events);
        }
        let mut finish = false;
        if let (Some(active), Some(reduced)) = (&mut self.active, reduced) {
            // Pointer/selection or unhandled semantic mutations must never let
            // an inferred result authorize a different actual widget value.
            if active.working != reduced.text {
                refused = true;
            } else {
                if let Some((result, cursor)) = reduced.result {
                    if active
                        .snapshot
                        .checkpoint(
                            &mut self.bound,
                            (active.baseline_cursor, &active.baseline),
                            (cursor, &result),
                        )
                        .is_ok()
                    {
                        draft.clone_from(&result);
                        active.baseline.clone_from(&result);
                        active.baseline_cursor = cursor;
                        active.snapshot.set_cursor(cursor);
                        finish = !reduced.composing;
                    } else {
                        refused = true;
                    }
                }
                active.composing = reduced.composing;
            }
        }
        if refused {
            self.rollback(ui, id);
        } else if finish {
            // Keep the actual committed cursor/state already stored by egui;
            // Drop restores only the original owned history into its live Arc.
            self.active.take();
        } else if editor.response.lost_focus()
            || (!editor.response.has_focus() && self.active.is_some())
        {
            self.rollback(ui, id);
        }
        if self.active.is_none() {
            let new_edit = focused
                && ui.input(|i| {
                    i.events.iter().any(|event| {
                        matches!(
                            event,
                            Event::Text(_)
                                | Event::Paste(_)
                                | Event::MiddleClickPaste { .. }
                                | Event::Key { pressed: true, .. }
                        )
                    })
                });
            let reentered = ui.ctx().interaction_snapshot(|s| s.clicked == Some(id));
            if new_edit || reentered {
                self.restored_state = None;
            } else if let Some(state) = &self.restored_state {
                state.clone().store(ui.ctx(), id);
            }
        }
        if has_ime || refused {
            self.notice = refused;
        }
        Output {
            editor,
            refused: self.notice,
            cancelled: stale || cancel,
            finished: retired_frame
                && ui.input(|i| {
                    !i.focused
                        && i.events
                            .iter()
                            .any(|e| matches!(e, Event::WindowFocused(false)))
                }),
        }
    }
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("mounted", &self.mounted)
            .field("trusted", &self.trusted)
            .field("notice", &self.notice)
            .field("composing", &self.active.is_some())
            .finish()
    }
}
impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("refused", &self.refused)
            .field("cancelled", &self.cancelled)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
