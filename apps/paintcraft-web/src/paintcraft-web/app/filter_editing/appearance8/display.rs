//! Separate committed Source8 host lane; never relabel or bypass old4–7.
use super::{
    bridge::{self, Client8, OpenFailure8, Opened8, Reply8},
    capture::{CurrentStamp8, Identity8, Key8},
    future::host::{Host8, Slot8},
};
use crate::app::{WebApp, filter_editing::render::View};
use eframe::egui::Context;
use paintcraft_core::types::{Error, LayerBlendMode, Result};
use paintcraft_raster::Admission;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub(super) type OpeningPayload8 = Option<std::result::Result<Opened8, OpenFailure8>>;
pub(super) type FramePayload8 = Option<Result<Reply8>>;
pub(in crate::app::filter_editing) struct Display8 {
    // One ORIGINAL trusted host metadata input, captured/reused for everyhelper.
    admission: Admission,
    identity: Option<Identity8>,
    normal: bool,
    stamp: Option<Rc<RefCell<Option<CurrentStamp8>>>>,
    stopped: Option<Rc<Cell<bool>>>,
    opening: Option<Rc<RefCell<OpeningPayload8>>>,
    client: Option<Client8>,
    flight: Option<(View, Rc<RefCell<FramePayload8>>, bool)>,
    closing: Option<Rc<RefCell<Option<Result<()>>>>>,
    accepted: Option<View>,
    error: Option<(Option<View>, Error)>,
    blocked: bool,
    host: Option<Host8>,
}
impl Default for Display8 {
    fn default() -> Self {
        // Explicit application host setup ONCE. Acquisition never substitutes a
        // default for an injected reserved factory, nor constructs a new pool.
        Self::new(Admission::new(0))
    }
}
impl std::fmt::Debug for Display8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source8Display")
            .field("opening", &self.opening.is_some())
            .field("flight", &self.flight.is_some())
            .field("closing", &self.closing.is_some())
            .field("blocked", &self.blocked)
            .finish()
    }
}
impl Display8 {
    fn new(admission: Admission) -> Self {
        Self {
            admission,
            identity: None,
            normal: false,
            stamp: None,
            stopped: None,
            opening: None,
            client: None,
            flight: None,
            closing: None,
            accepted: None,
            error: None,
            blocked: false,
            host: None,
        }
    }
    fn active(&self) -> bool {
        self.opening.is_some()
            || self.client.is_some()
            || self.flight.is_some()
            || self.closing.is_some()
    }
    fn refresh(&self, app: &WebApp) {
        let current = CurrentStamp8::read(app);
        if let Some(stamp) = &self.stamp {
            let old = stamp.borrow_mut().replace(current.clone());
            drop(old); // no RefMut across last original Document Arc teardown.
        }
        if let Some(client) = &self.client {
            client.refresh(current, app.canvas.filter_view);
        }
    }
    fn stop(&self) {
        if let Some(stopped) = &self.stopped {
            stopped.set(true);
        }
        if let Some(client) = &self.client {
            client.cancel();
        }
    }
    fn close(&mut self, ctx: &Context) {
        if self.closing.is_none()
            && self.flight.is_none()
            && self.opening.is_none()
            && let Some(client) = self.client.take()
        {
            let promise = match client.close() {
                Ok(promise) => promise,
                Err(error) => {
                    self.blocked = true;
                    self.error = Some((None, error));
                    return;
                }
            };
            let result = Rc::new(RefCell::new(None));
            self.closing = Some(result.clone());
            let Some(host) = &self.host else {
                self.blocked = true;
                return;
            };
            let args = CloseArgs8 {
                promise,
                result,
                ctx: ctx.clone(),
            };
            if let Err((error, args)) = host.spawn(Slot8::Close, args, close_task8) {
                host.clone().quarantine();
                drop(args);
                self.blocked = true;
                self.error = Some((None, error));
            }
        }
    }
    fn report(&self, app: &mut WebApp) {
        if let Some((_, error)) = &self.error {
            app.message = format!("Source8 preview unavailable: {error}");
        }
    }
    fn poll(&mut self, app: &mut WebApp, ctx: &Context) -> bool {
        if let Some(slot) = &self.closing {
            let result = slot.borrow_mut().take();
            let Some(result) = result else {
                ctx.request_repaint_after(std::time::Duration::from_millis(25));
                return false;
            };
            self.closing = None;
            drop(self.host.take());
            self.identity = None;
            self.normal = false;
            self.accepted = None;
            if let Err(error) = result {
                self.blocked = true;
                self.error = Some((None, error));
            }
        }
        if let Some(slot) = &self.opening {
            let result = slot.borrow_mut().take();
            if let Some(result) = result {
                self.opening = None;
                self.stopped = None;
                self.stamp = None;
                match result {
                    Ok(Opened8::Normal(identity)) => {
                        drop(self.host.take());
                        self.identity = Some(identity);
                        self.normal = true;
                    }
                    Ok(Opened8::Blend(client)) => {
                        self.client = Some(client);
                        self.normal = false;
                    }
                    Err(failure) => {
                        if failure.joined {
                            drop(self.host.take());
                        }
                        self.blocked = !failure.joined;
                        self.error = Some((None, failure.error));
                    }
                }
            }
        }
        if let Some((view, slot, _)) = &self.flight {
            let view = *view;
            let result = slot.borrow_mut().take();
            if let Some(result) = result {
                self.flight = None;
                match result {
                    Ok(reply) => {
                        let current = reply.identity.current(app)
                            && app.canvas.filter_view == Some(reply.view)
                            && self
                                .identity
                                .as_ref()
                                .is_some_and(|s| s.same_source(&reply.identity));
                        if current {
                            let result = self
                                .client
                                .as_ref()
                                .ok_or_else(super::denied)
                                .and_then(|c| c.install(reply.publication));
                            match result {
                                Ok(texture) => {
                                    app.canvas.texture = Some(texture);
                                    // Footprint is padded texel extent; region clips last
                                    // nativecell and would stretch partial LOD output.
                                    app.canvas.filter_footprint = Some(reply.view.footprint);
                                    app.mark_filter_canvas_current();
                                    self.accepted = Some(reply.view);
                                }
                                Err(error) => self.error = Some((Some(view), error)),
                            }
                        }
                    }
                    Err(error) => {
                        if self.identity.as_ref().is_some_and(|s| s.current(app))
                            && app.canvas.filter_view == Some(view)
                        {
                            self.error = Some((Some(view), error));
                        }
                    }
                }
            }
        }
        true
    }
    /// None explicitly selects existingroute; Some(true) owns this displayslot.
    fn step(&mut self, app: &mut WebApp, ctx: &Context) -> Option<bool> {
        self.refresh(app);
        if !self.poll(app, ctx) {
            self.report(app);
            return Some(true);
        }
        if self.blocked {
            self.report(app);
            return Some(true);
        }
        let source_changed = self.identity.as_ref().is_some_and(|s| !s.current(app));
        if source_changed {
            self.stop();
            if self.active() {
                self.close(ctx);
                return Some(true);
            }
            self.identity = None;
            self.normal = false;
            self.accepted = None;
            self.error = None;
        }
        if app.filter_editing.is_some()
            || app.filter_display.copy_action.is_some()
            || app.filter_display.picker_wanted.is_some()
            || app.filter_display.transient.is_some()
        {
            // Actual private/flat actions retain their original route/refusal.
            // Yield the singleton only after the genuine Source8 join retires.
            self.stop();
            if self.active() {
                self.close(ctx);
                return Some(true);
            }
            self.identity = None;
            self.normal = false;
            self.accepted = None;
            self.error = None;
            return None;
        }
        if self.normal {
            return None;
        }
        if self.identity.is_none() {
            let doc = app.editor.session.document();
            // O(1) common old single-Normal gate, not an unpaid HashMap walk.
            if let [id] = doc.roots.as_slice()
                && doc.layers.len() == 1
                && doc
                    .layers
                    .get(id)
                    .is_some_and(|l| l.blend_mode == LayerBlendMode::Normal)
            {
                return None;
            }
            match app.filter_display.renderer.suspend_for_copy(ctx) {
                Ok(true) => {}
                Ok(false) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(25));
                    return Some(true);
                }
                Err(error) => {
                    self.blocked = true;
                    self.error = Some((None, error));
                    self.report(app);
                    return Some(true);
                }
            }
            let key = match Key8::capture(app) {
                Ok(k) => k,
                Err(error) => {
                    self.error = Some((None, error));
                    self.report(app);
                    return Some(true);
                }
            };
            self.identity = Some(key.identity.clone());
            let (controller, host) = match Host8::begin() {
                Ok(pair) => pair,
                Err(error) => {
                    self.error = Some((None, error));
                    self.report(app);
                    return Some(true);
                }
            };
            self.host = Some(host.clone());
            let pending = bridge::Pending8::new(controller, host.clone());
            let stamp = Rc::new(RefCell::new(Some(CurrentStamp8::read(app))));
            let stopped = Rc::new(Cell::new(false));
            let slot = Rc::new(RefCell::new(None));
            self.stamp = Some(stamp.clone());
            self.stopped = Some(stopped.clone());
            self.opening = Some(slot.clone());
            let args = OpenArgs8 {
                key,
                admission: self.admission.clone(),
                ctx: ctx.clone(),
                stamp,
                stopped,
                slot,
                pending,
            };
            if let Err((error, args)) = host.spawn(Slot8::Open, args, open_task8) {
                drop(args); // Pending8 schedules the already-priced real cleanup.
                self.opening = None;
                self.error = Some((None, error));
                self.blocked = true;
                self.report(app);
            }
            return Some(true);
        }
        if self.opening.is_some() || self.closing.is_some() {
            return Some(true);
        }
        let Some(view) = app.canvas.filter_view else {
            return Some(true);
        };
        if let Some((active, _, cancelled)) = &mut self.flight {
            if *active != view && !*cancelled {
                if let Some(client) = &self.client {
                    client.cancel();
                }
                *cancelled = true;
            }
            return Some(true);
        }
        if self.accepted == Some(view) {
            app.mark_filter_canvas_current();
            return Some(true);
        }
        if self
            .error
            .as_ref()
            .is_some_and(|(failed, _)| failed.is_none() || *failed == Some(view))
        {
            self.report(app);
            return Some(true);
        }
        self.error = None;
        let Some(client) = &self.client else {
            self.report(app);
            return Some(true);
        };
        match client.ready() {
            Ok(false) => return Some(true),
            Err(error) => {
                self.blocked = true;
                self.error = Some((Some(view), error));
                self.report(app);
                return Some(true);
            }
            Ok(true) => {}
        }
        client.refresh(CurrentStamp8::read(app), Some(view));
        match client.prepare_frame(view, frame_task8) {
            Ok((run, credit)) => {
                let result = Rc::new(RefCell::new(None));
                let args = FrameArgs8 {
                    run,
                    result: result.clone(),
                    ctx: ctx.clone(),
                };
                match credit.construct(args, frame_task8) {
                    Ok(tracked) => {
                        self.flight = Some((view, result, false));
                        wasm_bindgen_futures::spawn_local(tracked);
                    }
                    Err((error, args)) => {
                        drop(args); // Includes the pre-first-poll Running8 guard.
                        self.error = Some((Some(view), error));
                        self.report(app);
                    }
                }
            }
            Err(error) => {
                self.error = Some((Some(view), error));
                self.report(app);
            }
        }
        Some(true)
    }
}
impl WebApp {
    pub(in crate::app::filter_editing) fn source8_canvas(&mut self, ctx: &Context) -> Option<bool> {
        let mut state = std::mem::take(&mut self.filter_display.blend8);
        let result = state.step(self, ctx);
        self.filter_display.blend8 = state;
        result
    }
    /// Called after ALL synchronous UI actions and before yielding to JS tasks.
    pub(crate) fn refresh_source8_current_stamp(&mut self) {
        let state = &self.filter_display.blend8;
        state.refresh(self);
    }
}

// One bounded source/open, frame, close, and current-identity slot per lane.
pub(super) fn control_bytes() -> Result<u64> {
    use super::{controls::shared, denied};
    let sizes = [
        shared::<RefCell<OpeningPayload8>>()?,
        shared::<RefCell<FramePayload8>>()?,
        shared::<RefCell<Option<Result<()>>>>()?,
        shared::<RefCell<Option<CurrentStamp8>>>()?,
        shared::<Cell<bool>>()?,
        (2 * std::mem::size_of::<Display8>()) as u64,
    ];
    sizes
        .into_iter()
        .try_fold(0u64, |a, n| a.checked_add(n).ok_or_else(denied))
}

#[derive(Debug)]
pub(super) struct OpenArgs8 {
    key: Key8,
    admission: Admission,
    ctx: Context,
    stamp: Rc<RefCell<Option<CurrentStamp8>>>,
    stopped: Rc<Cell<bool>>,
    slot: Rc<RefCell<OpeningPayload8>>,
    pending: bridge::Pending8,
}
pub(super) async fn open_task8(args: OpenArgs8) {
    let result = bridge::open(
        args.key,
        args.admission,
        args.ctx.clone(),
        args.stamp,
        args.stopped,
        args.pending,
    )
    .await;
    *args.slot.borrow_mut() = Some(result);
    args.ctx.request_repaint();
}
#[derive(Debug)]
pub(super) struct CloseArgs8 {
    promise: wasm_bindgen::JsValue,
    result: Rc<RefCell<Option<Result<()>>>>,
    ctx: Context,
}
pub(super) async fn close_task8(args: CloseArgs8) {
    let result = super::js::wait(args.promise).await.map(|_| ());
    *args.result.borrow_mut() = Some(result);
    args.ctx.request_repaint();
}
#[derive(Debug)]
struct FrameArgs8 {
    run: bridge::FrameRun8,
    result: Rc<RefCell<FramePayload8>>,
    ctx: Context,
}
async fn frame_task8(args: FrameArgs8) {
    let result = bridge::run_frame8(args.run).await;
    *args.result.borrow_mut() = Some(result);
    args.ctx.request_repaint();
}
