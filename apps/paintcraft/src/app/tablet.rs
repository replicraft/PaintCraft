//! Native capture uses the shared context-free ingress and existing edit service.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
use paintcraft_input::tablet::{
    Phase, Sample, Transform,
    receiver::{Delivery, UiEvent},
};
use paintcraft_platform::tablet::{self as native, DeliveryHealth};
use paintcraft_ui::pen_input::{Batch, Owner, Pass, Source, View as CaptureView};
const SOURCE_SLOTS: usize = 4096;
const UI_SLOTS: usize = 12288;
const UI_RELEASE_SLOTS: usize = UI_SLOTS + 3;

pub(super) struct Receiver {
    // Native callbacks retire before their ingress; Installation retains window.
    installation: Option<native::Installation>,
    source: Source,
    pass: Pass,
    pending: Option<(Owner, Batch)>,
    retained: Option<Batch>,
    controller_owned: bool,
    frame: u64,
    layer: Option<egui::LayerId>,
    controls: Option<egui::Rect>,
    fallback_button: Option<egui::PointerButton>,
    mouse: MouseCapture,
    view_epoch: u64,
    published_view: Option<CaptureView>,
    defer_view: bool,
    deferred_view: Option<CaptureView>,
    own_service_wait: bool,
    // Retained logical allowance for added egui DTO slots, not framework/RSS.
    // All host batches/events retire before this charge.
    _gui_events: BudgetAccount,
}
impl Receiver {
    fn new(
        source: Source,
        installation: Option<native::Installation>,
        budget: &SharedBudget,
    ) -> paintcraft_core::types::Result<Self> {
        let mut gui_events = budget.account(AllocationClass::PaintMetadata);
        gui_events.resize(
            (((UI_RELEASE_SLOTS + SOURCE_SLOTS) * std::mem::size_of::<egui::Event>())
                + 2 * std::mem::size_of::<CaptureView>()) as u64,
        )?;
        Ok(Self {
            installation,
            source,
            pass: Pass::default(),
            pending: None,
            retained: None,
            controller_owned: false,
            frame: 0,
            layer: None,
            controls: None,
            fallback_button: None,
            mouse: MouseCapture::default(),
            view_epoch: 0,
            published_view: None,
            defer_view: false,
            deferred_view: None,
            own_service_wait: false,
            _gui_events: gui_events,
        })
    }
    fn retire_presentation(&mut self) -> bool {
        let cancel_editor = self.controller_owned
            || self.fallback_button.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|(_, b)| !b.frame.samples.is_empty());
        self.pending = None;
        self.retained = None;
        self.controller_owned = false;
        self.fallback_button = None;
        cancel_editor
    }
    fn retire(&mut self) -> bool {
        let cancel_editor = self.retire_presentation();
        self.source.cancel();
        if let Some(native) = &self.installation {
            native.cancel();
        }
        cancel_editor
    }
    fn take(&mut self, owner: Owner, failed: bool) -> paintcraft_core::types::Result<()> {
        self.retained = None;
        self.frame = self.frame.checked_add(1).ok_or_else(|| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "Native tablet frame epoch exhausted",
            )
        })?;
        let take = |health| {
            self.source.take(
                self.frame,
                owner,
                failed || health == DeliveryHealth::Failed,
            )
        };
        let batch = match &self.installation {
            Some(native) => native.with_delivery(take),
            None => take(DeliveryHealth::Ready),
        }?;
        self.pass.begin(self.frame, &batch.frame);
        self.pending = Some((owner, batch));
        Ok(())
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        if let Some(native) = &self.installation {
            native.cancel();
        }
        self.source.fault();
    }
}
#[derive(Default)]
struct MouseCapture {
    buttons: u8,
    denied: bool,
}
impl MouseCapture {
    fn retain(&mut self, source: &Source, event: &egui::Event) -> bool {
        if let egui::Event::PointerButton {
            button, pressed, ..
        } = event
        {
            let bit = 1u8 << (*button as u8);
            if *pressed {
                if self.buttons == 0 {
                    self.denied = !source.mouse_capture(true);
                }
                self.buttons |= bit;
            } else {
                self.buttons &= !bit;
            }
            let keep = !self.denied;
            if self.buttons == 0 {
                source.mouse_capture(false);
                self.denied = false;
            }
            keep
        } else {
            !self.denied || !pointer_event(event)
        }
    }
}
fn pointer_event(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::PointerMoved(_)
            | egui::Event::PointerButton { .. }
            | egui::Event::PointerGone
            | egui::Event::MouseMoved(_)
    )
}
fn modifiers(m: paintcraft_input::Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt,
        ctrl: m.control,
        shift: m.shift,
        mac_cmd: m.command,
        command: m.command,
    }
}
fn button(b: paintcraft_input::PointerButton) -> egui::PointerButton {
    match b {
        paintcraft_input::PointerButton::Primary => egui::PointerButton::Primary,
        paintcraft_input::PointerButton::Secondary => egui::PointerButton::Secondary,
    }
}
fn ui_event(event: UiEvent) -> egui::Event {
    match event {
        UiEvent::Move { at, .. } => egui::Event::PointerMoved(egui::pos2(at.0 as f32, at.1 as f32)),
        UiEvent::Button {
            at,
            button: b,
            pressed,
            modifiers: m,
        } => egui::Event::PointerButton {
            pos: egui::pos2(at.0 as f32, at.1 as f32),
            button: button(b),
            pressed,
            modifiers: modifiers(m),
        },
        UiEvent::Gone => egui::Event::PointerGone,
    }
}
impl App {
    /// Explicit opt-in while native lifecycle qualification is in progress.
    /// Ordinary with_native_services does not install capture yet.
    pub fn with_native_tablet(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        if self.tablet.take().is_some() {
            self.editor.cancel_gesture();
        }
        let result = (|| {
            let source = Source::new(SOURCE_SLOTS, UI_SLOTS, &self.worker_budget)?;
            let mut receiver = Receiver::new(source.clone(), None, &self.worker_budget)?;
            let sink = source.clone();
            let controls = source.clone();
            let wake = cc.egui_ctx.clone();
            let window = cc.winit_window().cloned().ok_or_else(|| {
                paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                    "Native tablet window unavailable",
                )
            })?;
            let installation = native::install_with_controls(
                window,
                Arc::new(move |packet| {
                    let delivery = match packet.delivery {
                        native::Delivery::ReceiverPointer => Delivery::ReceiverPointer,
                        native::Delivery::SystemPointer => Delivery::SystemPointer,
                    };
                    sink.submit_native_with_button_edge(
                        delivery,
                        packet.raw,
                        packet.button_edge,
                        |scale| packet.into_logical(scale).map(|raw| raw.logical_position),
                    )
                    .map_or(true, |route| route.consume())
                }),
                Arc::new(move |boundary| {
                    if boundary == native::Boundary::ViewInvalidated {
                        controls.invalidate_view();
                    } else {
                        controls.cancel();
                    }
                    Ok(())
                }),
                Arc::new(move || wake.request_repaint()),
            )?;
            receiver.installation = Some(installation);
            Ok::<_, paintcraft_core::types::Error>(receiver)
        })();
        match result {
            Ok(receiver) => self.tablet = Some(receiver),
            Err(error) => self
                .editor
                .set_status(format!("Native tablet unavailable: {error}")),
        }
        self
    }
    pub fn tablet_capabilities(&self) -> Option<native::CapabilitySnapshot> {
        self.tablet
            .as_ref()?
            .installation
            .as_ref()
            .map(native::Installation::capabilities)
    }
    fn tablet_owner(&self) -> Owner {
        let (state, publication) = self.editor.render_key();
        Owner {
            session: self.editor.session_owner_id().as_u128(),
            state: state.as_u128(),
            publication,
            interface: if self.experience() == Experience::Studio {
                0
            } else {
                self.variant() as u8 + 1
            },
        }
    }
    fn tablet_transform(&self) -> Option<Transform> {
        let rect = self.canvas_rect?;
        Some(Transform {
            owner: self.editor.session_owner_id().as_u128(),
            // Document publication belongs to Owner, not coordinate geometry.
            epoch: self
                .tablet
                .as_ref()
                .map_or(0, |receiver| receiver.view_epoch),
            origin: (f64::from(rect.min.x), f64::from(rect.min.y)),
            points_per_pixel: f64::from(self.zoom),
        })
    }
    pub(super) fn tablet_raw_input(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let owner = self.tablet_owner();
        let Some(receiver) = &mut self.tablet else {
            return;
        };
        if receiver
            .pending
            .as_ref()
            .is_some_and(|(_, b)| !b.frame.samples.is_empty())
            && receiver.retire()
        {
            self.editor.cancel_gesture();
        }
        let failed = !input.focused
            || input.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::WindowFocused(false)
                        | egui::Event::Ime(egui::ImeEvent::Preedit { .. })
                )
            });
        if failed {
            receiver.mouse = MouseCapture::default();
            receiver.fallback_button = None;
            if receiver.retire() {
                self.editor.cancel_gesture();
            }
            receiver.source.mouse_capture(false);
            receiver.source.cancel();
            if let Some(native) = &receiver.installation {
                native.cancel();
            }
        }
        // These are genuine native pointer records: consumed tablet promotions
        // are absent. Account mouse ownership before appending widget fallback.
        input
            .events
            .retain(|event| receiver.mouse.retain(&receiver.source, event));
        if let Err(error) = receiver.take(owner, false) {
            receiver.source.fault();
            if let Some(native) = &receiver.installation {
                native.cancel();
            }
            receiver.retire();
            receiver.fallback_button = None;
            ctx.input_mut(|state| state.pointer = Default::default());
            self.editor.cancel_gesture();
            self.editor.set_status(error.to_string());
            return;
        }
        if receiver.pass.cancelled() {
            let cancel_editor = receiver.controller_owned
                || receiver.fallback_button.is_some()
                || receiver.pass.reset_gui_pointer();
            receiver.fallback_button = None;
            receiver.controller_owned = false;
            // Clear click/drag state before releases: cancelled UI contact is not
            // a click. No GUI/editor access occurs inside with_delivery.
            if receiver.pass.reset_gui_pointer() {
                ctx.input_mut(|state| state.pointer = Default::default());
            }
            if cancel_editor {
                self.editor.cancel_gesture();
            }
        }
        if receiver.pass.suppress_legacy_pointer() {
            input.events.retain(|event| !pointer_event(event));
        }
        let needed = receiver
            .pending
            .as_ref()
            .map_or(0, |(_, batch)| batch.frame.ui.len());
        if let Err(error) = reserve_events(&mut input.events, needed, UI_RELEASE_SLOTS) {
            let cancel_editor = receiver.retire();
            receiver.source.fault();
            ctx.input_mut(|state| state.pointer = Default::default());
            if cancel_editor {
                self.editor.cancel_gesture();
            }
            self.editor.set_status(error.to_string());
            return;
        }
        if let Some((_, batch)) = &mut receiver.pending {
            // Preserve the shared ordered record until this GUI-only translation.
            // Native cross-stream order is not inferred from its per-source ordinal.
            input.events.extend(
                batch
                    .frame
                    .ui
                    .drain(..)
                    .map(|record| ui_event(record.event)),
            );
        }
    }
    pub(super) fn tablet_canvas_input(
        &mut self,
        events: &mut Vec<egui::Event>,
        accept: bool,
        layer: egui::LayerId,
        controls: Option<Rect>,
    ) {
        let owner = self.tablet_owner();
        let transform = self.tablet_transform();
        let Some(mut receiver) = self.tablet.take() else {
            return;
        };
        receiver.layer = Some(layer);
        receiver.controls = controls;
        if receiver.pass.suppress_legacy_pointer() {
            events.retain(|event| !pointer_event(event));
        }
        if let Some((_, batch)) = receiver.pending.take() {
            // The opaque ingress binding also detects cancellation or a view
            // changed and restored after drain. Matching Owner fields alone do
            // not authorize replay of an already retired payload.
            let current = receiver.source.batch_is_current(&batch, owner);
            if !current
                || !accept
                || batch
                    .frame
                    .samples
                    .iter()
                    .any(|s| Some(s.transform) != transform)
            {
                receiver.source.cancel();
                receiver.controller_owned = false;
                receiver.fallback_button = None;
                if let Some(native) = &receiver.installation {
                    native.cancel();
                }
                if batch.frame.suppress_legacy_pointer || !batch.frame.samples.is_empty() {
                    self.editor.cancel_gesture();
                }
            } else if let Err(error) =
                reserve_events(events, batch.frame.samples.len(), SOURCE_SLOTS)
            {
                let cancel_editor = receiver.retire();
                receiver.source.fault();
                if cancel_editor {
                    self.editor.cancel_gesture();
                }
                self.editor.set_status(error.to_string());
            } else {
                for sample in &batch.frame.samples {
                    // A fallback contact stays on its captured legacy route
                    // even if media/options change before the terminal sample.
                    if receiver.fallback_button.is_some()
                        || (sample.phase == Phase::Down && !self.editor.pressure_eligible(*sample))
                    {
                        fallback(*sample, &mut receiver.fallback_button, events);
                        receiver.controller_owned = sample.phase != Phase::Up;
                        continue;
                    }
                    self.editor.options.eraser_color = if self.experience() == Experience::Classic {
                        self.editor.canvas_color()
                    } else {
                        Rgba::TRANSPARENT
                    };
                    match self.editor.pen_sample(*sample, &self.worker_budget) {
                        Ok(true) => {
                            receiver.controller_owned = sample.phase != Phase::Up;
                        }
                        Ok(false) => fallback(*sample, &mut receiver.fallback_button, events),
                        Err(error) => {
                            receiver.source.cancel();
                            receiver.fallback_button = None;
                            let owned = receiver.controller_owned;
                            receiver.controller_owned = false;
                            if let Some(native) = &receiver.installation {
                                native.cancel();
                            }
                            if owned {
                                self.editor.cancel_gesture();
                            }
                            self.editor.set_status(error.to_string());
                            events.retain(|event| !pointer_event(event));
                            break;
                        }
                    }
                    if sample.phase == Phase::Up {
                        receiver.controller_owned = false;
                    }
                }
            }
            receiver.retained = Some(batch);
        }
        self.tablet = Some(receiver);
    }
    pub(super) fn publish_tablet_view(&mut self, ctx: &egui::Context) {
        let owner = self.tablet_owner();
        let transform = self.tablet_transform();
        let blocked = self.action_context().blocked || self.document_count() == 0;
        let Some(receiver) = &mut self.tablet else {
            return;
        };
        if (blocked || self.ime_composing)
            && (receiver.controller_owned
                || receiver
                    .pending
                    .as_ref()
                    .is_some_and(|(_, b)| !b.frame.samples.is_empty()))
            && receiver.retire()
        {
            self.editor.cancel_gesture();
        }
        let transform = transform.unwrap_or(Transform {
            owner: owner.session,
            epoch: owner.publication,
            origin: (0.0, 0.0),
            points_per_pixel: 1.0,
        });
        let canvas = self.canvas_rect.unwrap_or(Rect::ZERO);
        let viewport = self.canvas_viewport.unwrap_or(Rect::ZERO);
        let mut exclusions = [None; 64];
        let mut overflow = false;
        let mut index = 0;
        if let Some(controls) = receiver.controls {
            exclusions[index] = Some(controls);
            index += 1;
        }
        ctx.memory(|memory| {
            for layer in memory.areas().visible_layer_ids() {
                if Some(layer) == receiver.layer || layer.order == egui::Order::Background {
                    continue;
                }
                if let Some(rect) = memory.area_rect(layer.id) {
                    if index == exclusions.len() {
                        overflow = true;
                        break;
                    }
                    exclusions[index] = Some(rect);
                    index += 1;
                }
            }
        });
        let mut view = CaptureView {
            owner,
            transform,
            pixels_per_point: f64::from(ctx.pixels_per_point()),
            canvas: canvas.intersect(viewport),
            exclusions,
            focused: ctx.input(|i| i.focused),
            blocked: blocked
                || overflow
                || (self.editor.painting_blocked() && !receiver.own_service_wait),
            ime: self.ime_composing,
        };
        if receiver
            .published_view
            .is_some_and(|old| !same_navigation(old, view))
        {
            let Some(epoch) = receiver.view_epoch.checked_add(1) else {
                receiver.source.fault();
                return;
            };
            receiver.view_epoch = epoch;
            view.transform.epoch = epoch;
        }
        // Only an unchanged, valid coordinate/window context can wait for the
        // current edge's actual receipt. Known modal/focus/IME/PPP/navigation
        // invalidation reaches Source immediately, before any continuation.
        let can_defer = receiver.defer_view
            && receiver.published_view.is_some_and(|old| {
                same_navigation(old, view)
                    && old.focused
                    && view.focused
                    && !old.blocked
                    && !view.blocked
                    && !old.ime
                    && !view.ime
            });
        if can_defer {
            receiver.deferred_view = Some(view);
        } else {
            if receiver.source.publish(view).is_err() {
                receiver.source.fault();
            }
            receiver.published_view = Some(view);
            receiver.deferred_view = None;
        }
    }
}
fn same_navigation(a: CaptureView, b: CaptureView) -> bool {
    a.owner.session == b.owner.session
        && a.owner.interface == b.owner.interface
        && a.transform.owner == b.transform.owner
        && a.transform.origin == b.transform.origin
        && a.transform.points_per_pixel == b.transform.points_per_pixel
        && a.canvas == b.canvas
        && a.pixels_per_point == b.pixels_per_point
}
fn reserve_events(
    events: &mut Vec<egui::Event>,
    additional: usize,
    limit: usize,
) -> paintcraft_core::types::Result<()> {
    if additional > limit || events.try_reserve_exact(additional).is_err() {
        return Err(paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::ResourceLimit,
            "Native pen GUI event capacity unavailable; lift the pen and reinstall capture",
        ));
    }
    Ok(())
}
fn fallback(sample: Sample, held: &mut Option<egui::PointerButton>, events: &mut Vec<egui::Event>) {
    let at = egui::pos2(
        (sample.transform.origin.0 + sample.position.0 * sample.transform.points_per_pixel) as f32,
        (sample.transform.origin.1 + sample.position.1 * sample.transform.points_per_pixel) as f32,
    );
    let button = if sample.buttons.0
        & (paintcraft_input::tablet::Buttons::BARREL
            | paintcraft_input::tablet::Buttons::BARREL_UPPER)
        != 0
    {
        egui::PointerButton::Secondary
    } else {
        egui::PointerButton::Primary
    };
    let button = match sample.phase {
        Phase::Down => {
            *held = Some(button);
            button
        }
        Phase::Up => {
            let Some(button) = held.take() else {
                return;
            };
            button
        }
        Phase::Cancel | Phase::Leave => {
            *held = None;
            return;
        }
        _ => button,
    };
    match sample.phase {
        Phase::Down | Phase::Up => events.push(egui::Event::PointerButton {
            pos: at,
            button,
            pressed: sample.phase == Phase::Down,
            modifiers: modifiers(sample.modifiers),
        }),
        Phase::Move | Phase::Hover => events.push(egui::Event::PointerMoved(at)),
        _ => {}
    }
}

#[cfg(test)]
#[path = "tablet/tests.rs"]
mod tests;

#[path = "tablet/owned.rs"]
mod owned;
