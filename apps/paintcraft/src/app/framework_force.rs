//! Windows framework-force fallback. This is a contact policy, not native pen
//! classification or source-clock/history qualification. GUI events stay intact.
use super::*;
use paintcraft_tools::pen_stroke::FrameworkPoint;
type Contact = (egui::TouchDeviceId, egui::TouchId);

#[derive(Clone, Copy, PartialEq)]
struct Owner {
    tab: Uuid,
    session: Uuid,
    state: (Uuid, u64),
    target: Option<Uuid>,
    tool: Tool,
    options: paintcraft_tools::ToolOptions,
    color: Rgba,
    experience: Experience,
    variant: ClassicVariant,
    navigation: bool,
    view: paintcraft_ui::view::View,
}
#[derive(Default)]
pub(super) struct State {
    contacts: [Option<Contact>; 16],
    owner: Option<(Contact, Owner)>,
    fenced: bool,
    overflow: bool,
    buttons: u8,
}
enum Decision {
    Ordinary,
    Suppress,
    Cancel,
    Begin(FrameworkPoint),
    Append(Option<FrameworkPoint>, bool),
}
impl State {
    fn cancel(&mut self) {
        self.owner = None;
        self.fenced = self.contacts.iter().any(Option::is_some) || self.buttons != 0;
    }
    fn idle(&mut self) {
        if self.contacts.iter().all(Option::is_none) && self.buttons == 0 && !self.overflow {
            self.fenced = false;
        }
    }
    fn event(
        &mut self,
        event: &egui::Event,
        owner: Owner,
        point: Option<FrameworkPoint>,
        eligible: bool,
    ) -> Decision {
        let invalid = self.owner.is_some_and(|(_, captured)| captured != owner);
        if invalid {
            self.cancel();
        }
        match event {
            egui::Event::WindowFocused(false) => {
                let captured = self.owner.is_some() || invalid;
                self.cancel();
                // New Start observations after focus recovery establish fresh
                // framework contact lifetimes; Move never reacquires ink.
                self.contacts = [None; 16];
                self.buttons = 0;
                self.overflow = false;
                self.fenced = false;
                if captured {
                    Decision::Cancel
                } else {
                    Decision::Ordinary
                }
            }
            egui::Event::Ime(_) | egui::Event::Text(_) | egui::Event::Paste(_)
                if self.owner.is_some() || invalid =>
            {
                self.cancel();
                Decision::Cancel
            }
            egui::Event::Zoom(_) | egui::Event::Rotate(_) | egui::Event::MouseWheel { .. }
                if self.owner.is_some() || invalid =>
            {
                self.cancel();
                Decision::Cancel
            }
            egui::Event::Touch {
                device_id,
                id,
                phase,
                force,
                ..
            } => {
                let identity = (*device_id, *id);
                let prior = self
                    .contacts
                    .iter()
                    .position(|value| *value == Some(identity));
                match phase {
                    egui::TouchPhase::Start => {
                        // Duplicate Start cannot replace a held capture.
                        if prior.is_some() {
                            if self.owner.is_some() || point.is_some() || invalid {
                                self.cancel();
                                return Decision::Cancel;
                            }
                            return Decision::Ordinary;
                        }
                        let first = self.contacts.iter().all(Option::is_none);
                        let Some(slot) = self.contacts.iter_mut().find(|value| value.is_none())
                        else {
                            self.overflow = true;
                            if self.owner.is_some() || point.is_some() || invalid {
                                self.cancel();
                                return Decision::Cancel;
                            }
                            return Decision::Ordinary;
                        };
                        *slot = Some(identity);
                        if eligible && force.is_some() && point.is_none() {
                            self.cancel();
                            return Decision::Cancel;
                        }
                        if self.owner.is_some()
                            || (point.is_some() && (self.buttons != 0 || self.overflow))
                        {
                            self.cancel();
                            return Decision::Cancel;
                        }
                        if first
                            && !self.fenced
                            && !self.overflow
                            && self.buttons == 0
                            && eligible
                            && let Some(point) = point
                        {
                            self.owner = Some((identity, owner));
                            return Decision::Begin(point);
                        }
                    }
                    egui::TouchPhase::Move if self.owner.is_some_and(|(id, _)| id == identity) => {
                        if let Some(point) = point {
                            return Decision::Append(Some(point), false);
                        }
                        self.cancel();
                        return Decision::Cancel;
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        if let Some(index) = prior {
                            self.contacts[index] = None;
                        }
                        let captured = self.owner.is_some_and(|(id, _)| id == identity);
                        self.idle();
                        if captured {
                            self.owner = None;
                            return if *phase == egui::TouchPhase::End {
                                Decision::Append(point, true)
                            } else {
                                Decision::Cancel
                            };
                        }
                    }
                    _ => {}
                }
                if invalid {
                    Decision::Cancel
                } else {
                    Decision::Ordinary
                }
            }
            egui::Event::PointerButton {
                button, pressed, ..
            } => {
                if self.owner.is_some() {
                    return Decision::Suppress;
                }
                let bit = 1u8 << (*button as u8);
                if *pressed {
                    self.buttons |= bit;
                } else {
                    self.buttons &= !bit;
                }
                let fenced = self.fenced;
                self.idle();
                if invalid {
                    Decision::Cancel
                } else if fenced {
                    Decision::Suppress
                } else {
                    Decision::Ordinary
                }
            }
            egui::Event::PointerMoved(_)
            | egui::Event::PointerGone
            | egui::Event::MouseMoved(_)
                if self.owner.is_some() || self.fenced || invalid =>
            {
                if invalid {
                    Decision::Cancel
                } else {
                    Decision::Suppress
                }
            }
            _ if invalid => Decision::Cancel,
            _ => Decision::Ordinary,
        }
    }
}

impl App {
    fn framework_force_owner(&self, view: paintcraft_ui::view::View) -> Owner {
        Owner {
            tab: self.active_tab_id(),
            session: self.editor.session_owner_id(),
            state: self.editor.render_key(),
            target: self.editor.target(),
            tool: self.editor.tool,
            options: self.editor.options,
            color: self.editor.color,
            experience: self.experience(),
            variant: self.variant(),
            navigation: self.studio.space_pan || self.studio.pan.is_active(),
            view,
        }
    }
    /// Layout or editor changes revoke private force input even on frames with
    /// no events. A stale capture never waits for its next motion to disappear.
    pub(super) fn framework_force_validate(
        &mut self,
        ui: &egui::Ui,
        view: paintcraft_ui::view::View,
    ) {
        let owner =
            self.framework_force_owner(self.canvas_view(view.origin, ui.ctx().pixels_per_point()));
        if self
            .framework_force
            .owner
            .is_some_and(|(_, captured)| captured != owner)
            || (self.framework_force.owner.is_some()
                && (!self.editor.framework_force_active()
                    || !self.editor.framework_force_context_valid()
                    || !ui.is_enabled()
                    || self.action_context().blocked
                    || egui::Popup::is_any_open(ui.ctx())))
        {
            if self
                .framework_force
                .owner
                .is_some_and(|(_, captured)| captured.session == owner.session)
                && self.editor.framework_force_active()
            {
                self.editor.cancel_framework_force();
            }
            self.framework_force.cancel();
        }
    }
    pub(super) fn framework_force_event(
        &mut self,
        ui: &egui::Ui,
        layer: egui::LayerId,
        visible: egui::Rect,
        controls: Option<egui::Rect>,
        view: paintcraft_ui::view::View,
        event: &egui::Event,
    ) -> bool {
        // Recompute the current zoom too: navigation earlier in this same input
        // list must revoke the old mapping before a later contact terminal.
        let current_view = self.canvas_view(view.origin, ui.ctx().pixels_per_point());
        let owner = self.framework_force_owner(current_view);
        let mut state = std::mem::take(&mut self.framework_force);
        let cancel_force = state
            .owner
            .is_some_and(|(_, captured)| captured.session == owner.session)
            && self.editor.framework_force_active();
        let cancel_mouse = state.owner.is_none() && self.editor.framework_mouse_ink_active();
        if state.owner.is_some() && !self.editor.framework_force_active() {
            state.cancel();
        }
        let mut point = match event {
            egui::Event::Touch {
                pos,
                force: Some(force),
                ..
            } if force.is_finite() && (0.0..=1.0).contains(force) => view
                .to_doc_subpixel((pos.x, pos.y))
                .map(|at| FrameworkPoint { at, force: *force }),
            _ => None,
        };
        if state.owner.is_none() && !self.editor.framework_force_context_valid() {
            point = None;
        }
        // A new zoom can precede Start in this same event list, while layout
        // and the canvas rectangle still describe the old rendered view. Do not
        // authenticate a point mapped with that stale scale. Its contact is
        // fenced until terminal; a fresh layout and Start are required.
        if current_view != view {
            point = None;
        }
        let eligible = matches!(event,egui::Event::Touch {pos,..} if visible.contains(*pos)
            && controls.is_none_or(|r| !r.contains(*pos)) && ui.ctx().layer_id_at(*pos)==Some(layer))
            && ui.is_enabled()
            && !self.action_context().blocked
            && !egui::Popup::is_any_open(ui.ctx())
            && !owner.navigation
            && self.editor.framework_force_eligible();
        if matches!(event,egui::Event::Touch {phase:egui::TouchPhase::Start,pos,..}
            if !visible.contains(*pos) || controls.is_some_and(|r|r.contains(*pos))
                || ui.ctx().layer_id_at(*pos)!=Some(layer) || !ui.is_enabled()
                || self.action_context().blocked || egui::Popup::is_any_open(ui.ctx()))
        {
            point = None;
        }
        let decision = state.event(event, owner, point, eligible);
        let consumed = !matches!(decision, Decision::Ordinary);
        let result = match decision {
            Decision::Begin(point) => self
                .editor
                .begin_framework_force(point, &self.worker_budget)
                .map(|accepted| {
                    if !accepted {
                        state.cancel();
                    }
                }),
            Decision::Append(point, finish) => self.editor.append_framework_force(point, finish),
            Decision::Cancel => {
                if cancel_force {
                    self.editor.cancel_framework_force();
                } else if cancel_mouse {
                    self.editor.cancel_gesture();
                }
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            state.cancel();
            if self.editor.framework_force_active() {
                self.editor.cancel_framework_force();
            }
            self.editor
                .set_status(format!("Stroke not prepared: {error}"));
        }
        self.framework_force = state;
        consumed
    }
    /// Disabled/modal canvases do not draw, but real terminals still retire the
    /// bounded contact bank. A hidden canvas never replays a missed Start later.
    pub(super) fn framework_force_inactive(&mut self, events: &[egui::Event]) {
        if self
            .framework_force
            .owner
            .is_some_and(|(_, owner)| owner.session == self.editor.session_owner_id())
            && self.editor.framework_force_active()
        {
            self.editor.cancel_framework_force();
        }
        self.framework_force.cancel();
        let owner = self.framework_force_owner(self.canvas_view((0.0, 0.0), 1.0));
        for event in events {
            self.framework_force.event(event, owner, None, false);
        }
    }
}

#[cfg(test)]
#[path = "framework_force/tests.rs"]
mod tests;
