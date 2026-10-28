//! Classified input handoff; canvas replay/publication occurs after GUI arbitration.
mod acquisition;
mod canvas;
mod diagnostics;
#[cfg(target_arch = "wasm32")]
pub(crate) mod dom;
#[cfg(any(target_arch = "wasm32", test))]
mod events;
mod frame;
mod gui;
#[cfg(test)]
mod overflow_host;
use crate::WebApp;
use acquisition::{Acquisition, MouseRecord};
use eframe::egui::{self, Event};
use frame::CanvasFrame;
use paintcraft_core::types::{
    Rgba,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use paintcraft_input::{
    PointerButton,
    tablet::{Phase, Sample, Transform, receiver::UiEvent},
};
use paintcraft_tools::{Tool, ToolOptions};
use paintcraft_ui::pen_input::{Batch, Owner, Pass, Source, View};
use paintcraft_ui::t;
use std::sync::{Arc, Mutex};
const CAPACITY: usize = 4096;
#[derive(Clone, Debug)]
pub(crate) enum CanvasEvent {
    Legacy(Event),
    Mouse { sequence: u64, event: Event },
    Pen(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Config {
    tool: Tool,
    options: ToolOptions,
    primary: Rgba,
    secondary: Rgba,
    target: Option<uuid::Uuid>,
    sampled: Option<uuid::Uuid>,
}
#[derive(Clone, Copy, Debug)]
struct Capture {
    owner: Owner,
    config: Config,
    ppp: f64,
}
#[derive(Debug)]
pub(crate) struct State {
    pub source: Option<Source>,
    pub(crate) acquisition: Option<Arc<Mutex<Acquisition>>>,
    mouse_events: Vec<MouseRecord>,
    budget: SharedBudget,
    pass: Pass,
    pending: Option<(Capture, Batch)>,
    retained: Option<Batch>,
    retained_config: Option<Config>,
    controller_owned: bool,
    pub(crate) legacy_canvas_owned: bool,
    replay_failed: bool,
    raw_frame: Option<u64>,
    _events: BudgetAccount,
    #[cfg(target_arch = "wasm32")]
    ctx: egui::Context,
    pub region: Option<(egui::Rect, egui::LayerId)>,
    previous_transform: Option<Transform>,
    epoch: u64,
    ppp: f64,
    mouse_buttons: u8,
    gui_button: Option<PointerButton>,
    fallback_button: Option<PointerButton>,
    active_owner: Option<Owner>,
    error: Option<String>,
}
impl State {
    pub fn new(_ctx: &egui::Context) -> Self {
        let budget = SharedBudget::new(64 << 20);
        let mut events = budget.account(AllocationClass::PaintMetadata);
        let mut mouse_events = Vec::new();
        let result = events
            .resize(
                (CAPACITY
                    * (11 * std::mem::size_of::<Event>() + 2 * std::mem::size_of::<CanvasEvent>())
                    + Acquisition::declared_bytes()
                    + CAPACITY * std::mem::size_of::<u64>()
                    + 6 * std::mem::size_of::<Event>()) as u64,
            )
            .and_then(|_| {
                mouse_events.try_reserve_exact(CAPACITY).map_err(|_| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::ResourceLimit,
                        "Mouse order storage unavailable",
                    )
                })?;
                let acquisition = Acquisition::new().map_err(|_| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::ResourceLimit,
                        "Pointer acquisition storage unavailable",
                    )
                })?;
                Source::new(CAPACITY, CAPACITY * 3, &budget)
                    .map(|source| (source, Arc::new(Mutex::new(acquisition))))
            });
        let (source, acquisition, error) = match result {
            Ok((source, acquisition)) => (Some(source), Some(acquisition), None),
            Err(error) => (None, None, Some(error.to_string())),
        };
        Self {
            source,
            acquisition,
            mouse_events,
            budget,
            pass: Pass::default(),
            pending: None,
            retained: None,
            retained_config: None,
            controller_owned: false,
            legacy_canvas_owned: false,
            replay_failed: false,
            raw_frame: None,
            _events: events,
            #[cfg(target_arch = "wasm32")]
            ctx: _ctx.clone(),
            region: None,
            previous_transform: None,
            epoch: 0,
            ppp: 1.0,
            mouse_buttons: 0,
            gui_button: None,
            fallback_button: None,
            active_owner: None,
            error,
        }
    }
}
fn is_pointer(e: &Event) -> bool {
    matches!(
        e,
        Event::PointerMoved(_)
            | Event::PointerButton { .. }
            | Event::PointerGone
            | Event::MouseMoved(_)
    )
}
fn modifiers(m: paintcraft_input::Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: m.control,
        shift: m.shift,
        mac_cmd: m.command,
        command: m.command || m.control,
    }
}
fn button(b: PointerButton) -> egui::PointerButton {
    if b == PointerButton::Primary {
        egui::PointerButton::Primary
    } else {
        egui::PointerButton::Secondary
    }
}
fn ui_event(event: UiEvent) -> Event {
    match event {
        UiEvent::Move { at, .. } => Event::PointerMoved(egui::pos2(at.0 as f32, at.1 as f32)),
        UiEvent::Button {
            at,
            button: b,
            pressed,
            modifiers: m,
        } => Event::PointerButton {
            pos: egui::pos2(at.0 as f32, at.1 as f32),
            button: button(b),
            pressed,
            modifiers: modifiers(m),
        },
        UiEvent::Gone => Event::PointerGone,
    }
}
fn legacy_event(sample: Sample, held: &mut Option<PointerButton>) -> Option<Event> {
    let pos = egui::pos2(
        (sample.position.0 * sample.transform.points_per_pixel + sample.transform.origin.0) as f32,
        (sample.position.1 * sample.transform.points_per_pixel + sample.transform.origin.1) as f32,
    );
    match sample.phase {
        Phase::Down => {
            let b = sample.buttons.ink_button();
            *held = Some(b);
            Some(Event::PointerButton {
                pos,
                button: button(b),
                pressed: true,
                modifiers: modifiers(sample.modifiers),
            })
        }
        Phase::Up => held.take().map(|b| Event::PointerButton {
            pos,
            button: button(b),
            pressed: false,
            modifiers: modifiers(sample.modifiers),
        }),
        Phase::Move => Some(Event::PointerMoved(pos)),
        _ => None,
    }
}
impl WebApp {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn pen_context(&self) -> egui::Context {
        self.pen.ctx.clone()
    }
    fn pen_owner(&self) -> Owner {
        let ticket = self.editor.session.ticket();
        Owner {
            session: self.editor.session.owner_id().as_u128(),
            state: ticket.state_id.as_u128(),
            publication: ticket.publication_epoch,
            interface: if self.frontend == paintcraft_ui::actions::Frontend::Studio {
                0
            } else {
                self.variant as u8 + 1
            },
        }
    }
    fn pen_config(&self) -> Config {
        Config {
            tool: self.controls.tool,
            options: self.controls.options,
            primary: self.controls.primary,
            secondary: self.controls.secondary,
            target: self.editor.target,
            sampled: self.editor.tools.sampled_source,
        }
    }
    fn retire_pen_host(&mut self, ctx: &egui::Context) {
        if let Some(source) = &self.pen.source {
            source.cancel();
        }
        self.retire_pen_private(ctx);
    }
    fn retire_pen_private(&mut self, ctx: &egui::Context) {
        self.retire_pen_private_scoped(ctx, true);
    }
    fn retire_pen_private_scoped(&mut self, ctx: &egui::Context, reset_gui_pointer: bool) {
        // Unconsumed input cannot destroy unrelated text/paste/shape drafts.
        if self.pen.controller_owned || (reset_gui_pointer && self.pen.legacy_canvas_owned) {
            self.editor.cancel_pen_capture();
        }
        if reset_gui_pointer || self.pen.controller_owned {
            self.pen.legacy_canvas_owned = false;
        }
        self.pen.pending = None;
        self.pen.retained = None;
        self.pen.retained_config = None;
        self.pen.controller_owned = false;
        self.pen.gui_button = None;
        self.pen.fallback_button = None;
        self.pen.active_owner = None;
        if reset_gui_pointer {
            ctx.stop_dragging();
            ctx.input_mut(|i| i.pointer = egui::PointerState::default());
        }
    }
    pub(crate) fn pen_raw_input(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let Some(source) = self.pen.source.clone() else {
            return;
        };
        let frame = ctx.cumulative_frame_nr();
        if self.pen.raw_frame == Some(frame) {
            if self.pen.pass.suppress_legacy_pointer() {
                raw.events.retain(|e| !is_pointer(e));
            }
            return;
        }
        self.pen.raw_frame = Some(frame);
        if self
            .pen
            .pending
            .as_ref()
            .is_some_and(|(_, b)| !b.frame.samples.is_empty())
        {
            self.retire_pen_host(ctx);
        }
        self.pen.pending = None;
        self.pen.retained = None;
        self.pen.retained_config = None;
        self.pen.replay_failed = false;
        if let Some(error) = self.pen.error.take() {
            self.pen_failure(ctx, error);
        }
        if let Some(acquisition) = &self.pen.acquisition {
            acquisition
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(&mut self.pen.mouse_events);
        }
        let mut palm = false;
        let has_touch = raw.events.iter().any(|e| matches!(e, Event::Touch { .. }));
        for event in &raw.events {
            if let Event::Touch { id, phase, .. } = event {
                let phase = match phase {
                    egui::TouchPhase::Start => Phase::Down,
                    egui::TouchPhase::Move => Phase::Move,
                    egui::TouchPhase::End => Phase::Up,
                    egui::TouchPhase::Cancel => Phase::Cancel,
                };
                palm |= source.finger(id.0, phase).unwrap_or(true);
            }
            if !has_touch
                && let Event::PointerButton {
                    button, pressed, ..
                } = event
            {
                let bit = 1_u8 << (*button as u8);
                if *pressed {
                    if source.mouse_capture(true) {
                        self.pen.mouse_buttons |= bit;
                    }
                } else {
                    self.pen.mouse_buttons &= !bit;
                    let owned = self
                        .pen
                        .acquisition
                        .as_ref()
                        .map_or(0, |a| a.lock().unwrap_or_else(|e| e.into_inner()).buttons());
                    source.mouse_capture(self.pen.mouse_buttons != 0 || owned != 0);
                }
            }
        }
        if !raw.focused
            || raw.events.iter().any(|e| {
                matches!(
                    e,
                    Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    } | Event::Ime(egui::ImeEvent::Preedit { .. })
                )
            })
        {
            self.retire_pen_host(ctx);
            self.pen.mouse_buttons = 0;
            if let Some(a) = &self.pen.acquisition {
                a.lock().unwrap_or_else(|e| e.into_inner()).reset(&source);
            }
            self.pen.mouse_events.clear();
            source.mouse_capture(false);
        }
        if palm {
            raw.events.retain(|e| {
                !is_pointer(e)
                    && !matches!(
                        e,
                        Event::Touch {
                            phase: egui::TouchPhase::Start | egui::TouchPhase::Move,
                            ..
                        }
                    )
            });
        }
        let capture = Capture {
            owner: self.pen_owner(),
            config: self.pen_config(),
            ppp: self.pen.ppp,
        };
        let batch = match source.take(frame, capture.owner, false) {
            Ok(batch) => batch,
            Err(error) => {
                self.retire_pen_host(ctx);
                self.pen_failure(ctx, error);
                raw.events.retain(|e| !is_pointer(e));
                return;
            }
        };
        self.pen.pass.begin(frame, &batch.frame);
        if batch.frame.cancelled {
            // A drained cancellation may already precede fresh valid Down data.
            // Do not feed it back into Source.cancel and fence that new stroke.
            self.retire_pen_private_scoped(ctx, batch.frame.reset_gui_pointer);
        }
        let contact = batch.frame.samples.iter().any(|s| s.phase == Phase::Down)
            || batch
                .frame
                .ui
                .iter()
                .any(|e| matches!(e.event, UiEvent::Button { pressed: true, .. }));
        if contact {
            self.canvas.touch.retire_for_pen(ctx);
        }
        if batch.frame.suppress_legacy_pointer {
            raw.events.retain(|e| !is_pointer(e));
        }
        if raw
            .events
            .try_reserve(batch.frame.ui.len() + batch.frame.samples.len())
            .is_err()
        {
            self.retire_pen_host(ctx);
            self.pen_failure(ctx, "GUI input capacity");
            return;
        }
        if !self.append_pen_gui(raw, &batch, ctx) {
            return;
        }
        self.pen.pending = Some((capture, batch));
    }
}
