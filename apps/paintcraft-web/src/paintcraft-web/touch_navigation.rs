//! Canvas-owned contacts arbitrate before eframe's synthetic pointer releases.
use crate::WebApp;
use eframe::egui::{self, Event, Pos2, Rect, TouchDeviceId, TouchId, TouchPhase};
use paintcraft_tools::Tool;

#[derive(Default)]
pub(crate) struct TouchNavigation {
    contacts: Vec<Contact>,
    latched: bool,
    overflow: bool,
    orphaned: bool,
    owned: bool,
    pub(crate) suppress: bool,
    pub(crate) block_ui: bool,
    pub(crate) delivered_tap: Option<Pos2>,
    viewport: Option<(Rect, egui::LayerId)>,
    owner: Option<egui::Id>,
    anchor: Option<Pos2>,
    pair: Option<(Pos2, f32)>,
    deferred: Option<Pos2>,
    tool: Option<Tool>,
    events: Option<Vec<Event>>,
}
struct Contact {
    device: TouchDeviceId,
    id: TouchId,
    pos: Pos2,
    eligible: bool,
}
impl TouchNavigation {
    pub(crate) fn retire_for_pen(&mut self, ctx: &egui::Context) {
        self.retire_owner(ctx);
        // The shared pen receiver now fences these palm tails. Navigation keeps
        // terminal bookkeeping but cannot disable the replacement pen owner.
        self.owned = false;
        self.block_ui = false;
    }
    pub(crate) fn retire_owner(&mut self, ctx: &egui::Context) {
        let held = !self.contacts.is_empty() || self.latched || self.overflow;
        for contact in &mut self.contacts {
            contact.eligible = false;
        }
        self.latched = held;
        self.owned = held;
        self.suppress = held;
        self.block_ui = held;
        self.anchor = None;
        self.pair = None;
        self.deferred = None;
        self.delivered_tap = None;
        self.events = None;
        self.owner = None;
        self.tool = None;
        self.viewport = None;
        if held {
            ctx.stop_dragging();
            ctx.input_mut(|i| i.pointer = egui::PointerState::default());
        }
    }

    #[cfg(test)]
    pub(crate) fn hit_region(&self) -> Option<(Rect, egui::LayerId)> {
        self.viewport
    }
    pub(crate) fn viewport(&mut self, rect: Rect, layer: egui::LayerId) {
        self.viewport = Some((rect, layer));
    }
}
impl WebApp {
    pub(crate) fn begin_touch_frame(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        let modal = self.modal_blocked() || egui::Popup::is_any_open(ctx);
        let ticket = self.editor.session.ticket();
        let owner = egui::Id::unique((
            self.editor.session.history_ticket().owner,
            ticket.state_id,
            ticket.publication_epoch,
            self.frontend as u8,
            self.variant as u8,
        ));
        let mut state = std::mem::take(&mut self.canvas.touch);
        state.events = None;
        state.delivered_tap = None;
        state.suppress = state.latched;
        state.block_ui = state.latched && state.owned;
        if state.contacts.is_empty() && !state.latched {
            state.owned = false;
            state.anchor = None;
            state.pair = None;
            state.tool = None;
        }
        let was_active = state.latched || !state.contacts.is_empty();
        let previous_pair = state.pair;
        let mut peak = state.contacts.len();
        let mut cancelled = !ctx.input(|i| i.focused);
        let mut ended = false;
        let mut has_touch = false;
        let mut composition_changed = false;
        let mut orphan_tail = false;
        for event in &events {
            if let Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = *event
            {
                has_touch = true;
                if !pos.is_finite() {
                    cancelled = true;
                    // Ending a known contact needs its identity, not coordinates.
                    if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                        composition_changed = true;
                        ended = true;
                        state
                            .contacts
                            .retain(|c| c.device != device_id || c.id != id);
                    }
                    continue;
                }
                match phase {
                    TouchPhase::Start => {
                        composition_changed = true;
                        state.orphaned = false;
                        let eligible = !modal
                            && state.viewport.is_some_and(|(rect, layer)| {
                                rect.contains(pos)
                                    && ctx.layer_id_at(pos) == Some(layer)
                                    && self.canvas.controls.is_none_or(|r| !r.contains(pos))
                            });
                        if state.contacts.len() < 32 {
                            if !state
                                .contacts
                                .iter()
                                .any(|c| c.device == device_id && c.id == id)
                            {
                                state.contacts.push(Contact {
                                    device: device_id,
                                    id,
                                    pos,
                                    eligible,
                                });
                            }
                        } else {
                            state.overflow = true;
                            cancelled = true;
                        }
                        state.owned |= eligible;
                        peak = peak.max(state.contacts.len());
                    }
                    TouchPhase::Move => {
                        orphan_tail |= state.orphaned;
                        if let Some(c) = state
                            .contacts
                            .iter_mut()
                            .find(|c| c.device == device_id && c.id == id)
                        {
                            c.pos = pos;
                        }
                    }
                    TouchPhase::End | TouchPhase::Cancel => {
                        orphan_tail |= state.orphaned;
                        composition_changed = true;
                        cancelled |= phase == TouchPhase::Cancel;
                        ended = true;
                        state
                            .contacts
                            .retain(|c| c.device != device_id || c.id != id);
                    }
                }
            }
        }
        cancelled |= state.owner.is_some_and(|old| old != owner) && was_active;
        cancelled |= was_active
            && state.owned
            && (modal || state.tool.is_some_and(|tool| tool != self.controls.tool));
        if !was_active && state.owned {
            state.tool = Some(self.controls.tool);
        }
        state.owner = Some(owner);
        // Focus loss may omit TouchCancel. Retire our contacts without trusting
        // egui's private touch inventory, and ignore old tails until a new Start.
        if !ctx.input(|i| i.focused) && state.owned {
            state.contacts.clear();
            state.overflow = false;
            state.orphaned = true;
        }
        if orphan_tail {
            state.owned = true;
            cancelled = true;
        }
        if cancelled {
            for contact in &mut state.contacts {
                contact.eligible = false;
            }
            state.anchor = None;
        }
        let multi = peak >= 2;
        if multi || (cancelled && state.owned) {
            state.latched = true;
            state.deferred = None;
        }
        state.suppress |= state.latched;
        state.block_ui |= state.latched && state.owned;
        if state.suppress && state.owned {
            self.canvas.pending_center = None;
            self.editor.cancel_floating();
            self.editor.tools.cancel_gesture();
            self.interrupt_paste_touch_drag();
            ctx.stop_dragging();
            ctx.input_mut(|i| i.pointer = egui::PointerState::default());
        }
        let eligible = state.contacts.len() == 2
            && state.contacts.iter().all(|c| c.eligible)
            && state.contacts[0].device == state.contacts[1].device;
        if state.latched && eligible && !cancelled && !modal {
            let a = state.contacts[0].pos;
            let b = state.contacts[1].pos;
            let center = a + (b - a) * 0.5;
            let distance = a.distance(b);
            if !center.is_finite() || !distance.is_finite() {
                state.anchor = None;
                state.pair = None;
                for contact in &mut state.contacts {
                    contact.eligible = false;
                }
                self.canvas.touch = state;
                return;
            }
            if let Some(rect) = self.canvas.rect {
                if composition_changed || previous_pair.is_none() {
                    state.anchor = None;
                }
                let anchor = *state
                    .anchor
                    .get_or_insert_with(|| ((center - rect.min) / self.zoom).to_pos2());
                if let Some((_, old_distance)) = previous_pair.filter(|_| !composition_changed) {
                    let ratio = distance / old_distance;
                    if ratio.is_finite() && ratio > 0.0 {
                        self.zoom = (self.zoom * ratio).clamp(0.125, 10.0);
                    }
                }
                if composition_changed || previous_pair.is_none_or(|p| p != (center, distance)) {
                    if self.frontend == paintcraft_ui::actions::Frontend::Studio {
                        paintcraft_studio::canvas::anchor_at(
                            ctx,
                            egui::Id::unique("web-studio-canvas"),
                            self.studio_canvas_scope(),
                            anchor,
                            center,
                            self.zoom,
                        );
                    } else {
                        paintcraft_classic::canvas::anchor_at(
                            ctx,
                            egui::Id::unique("web-editor-canvas"),
                            self.studio_canvas_scope(),
                            anchor,
                            center,
                            self.zoom,
                        );
                    }
                }
                state.pair = Some((center, distance));
            }
        } else {
            state.pair = None;
            if !state.latched {
                state.anchor = None;
            }
        }
        // These tools publish on pointer-down. A touch must remain a single tap
        // through release before that one-shot action is delivered.
        let one_shot = matches!(self.controls.tool, Tool::Text | Tool::Magnifier)
            || self.paste_touch_press_is_one_shot(events.iter().find_map(|e| match e {
                Event::PointerButton {
                    pos, pressed: true, ..
                } => Some(*pos),
                _ => None,
            }));
        if !state.suppress && (has_touch || !state.contacts.is_empty() || state.deferred.is_some())
        {
            if one_shot && state.owned && state.deferred.is_none() {
                state.deferred = events.iter().find_map(|e| match e {
                    Event::PointerButton {
                        pos,
                        pressed: true,
                        button: egui::PointerButton::Primary,
                        ..
                    } => Some(*pos),
                    _ => None,
                });
            }
            if state.deferred.is_some() {
                let mut filtered: Vec<_> = events
                    .iter()
                    .filter(|e| {
                        !matches!(
                            e,
                            Event::PointerButton { .. }
                                | Event::PointerMoved(_)
                                | Event::PointerGone
                        )
                    })
                    .cloned()
                    .collect();
                if ended && state.contacts.is_empty() && !cancelled {
                    let pos = state.deferred.take().unwrap();
                    state.delivered_tap = Some(pos);
                    for pressed in [true, false] {
                        filtered.push(Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
                state.events = Some(filtered);
            }
        }
        if cancelled {
            state.deferred = None;
        }
        if state.contacts.is_empty() && (!state.overflow || !ctx.input(|i| i.any_touches())) {
            state.overflow = false;
            state.latched = false;
            state.anchor = None;
            state.pair = None;
        }
        self.canvas.touch = state;
    }
    pub(crate) fn canvas_events(&self, ctx: &egui::Context) -> Vec<Event> {
        self.canvas
            .touch
            .events
            .clone()
            .unwrap_or_else(|| ctx.input(|i| i.events.clone()))
    }
}
