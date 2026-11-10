//! Actual Main controller consumer; opaque owners are minted only here.
use super::{
    capture::{CurrentStamp8, Identity8, Key8},
    denied,
    factory::{Credit8, Factory8},
    future::host::{Host8, Slot8},
    js,
    owner::{MainBlendOwner8, Owner8},
    publication::CpuPublication8,
    snapshot::Preparation8,
};
use crate::app::filter_editing::render::View;
use eframe::egui::Context;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_raster::Admission;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
#[derive(Debug)]
pub(super) struct Reply8 {
    pub identity: Identity8,
    pub view: View,
    pub publication: CpuPublication8,
}
#[derive(Debug)]
pub(super) enum Opened8 {
    Normal(Identity8),
    Blend(Client8),
}
#[derive(Debug)]
pub(super) struct OpenFailure8 {
    pub error: Error,
    pub joined: bool,
}
#[derive(Debug)]
pub(super) struct Pending8 {
    controller: JsValue,
    owner: Option<Owner8>,
    armed: bool,
    host: Host8,
}
impl Pending8 {
    pub fn new(controller: JsValue, host: Host8) -> Self {
        Self {
            controller,
            owner: None,
            armed: true,
            host,
        }
    }
}
#[derive(Debug)]
pub(super) struct CleanupArgs8 {
    controller: JsValue,
    owner: Option<Owner8>,
}
pub(super) async fn cleanup_task8(mut args: CleanupArgs8) {
    let result = js::call(&args.controller, "close", &[]);
    drop(args.owner.take()); // no strong S shell across mainGone.
    if let Ok(promise) = result {
        let _ = js::wait(promise).await;
    }
}
impl Drop for Pending8 {
    fn drop(&mut self) {
        if self.armed {
            let args = CleanupArgs8 {
                controller: self.controller.clone(),
                owner: self.owner.take(),
            };
            if let Err((_, args)) = self.host.spawn(Slot8::Cleanup, args, cleanup_task8) {
                self.host.clone().quarantine();
                drop(args);
            }
        }
    }
}
#[derive(Debug)]
pub(super) struct Client8 {
    controller: JsValue,
    session: JsValue,
    owner: Option<Owner8>,
    serial: Cell<u64>,
    cancel: RefCell<Option<JsValue>>,
    closed: bool,
    host: Host8,
}
impl Drop for Client8 {
    fn drop(&mut self) {
        if !self.closed {
            let args = CleanupArgs8 {
                controller: self.controller.clone(),
                owner: self.owner.take(),
            };
            if let Err((_, args)) = self.host.spawn(Slot8::Cleanup, args, cleanup_task8) {
                self.host.clone().quarantine();
                drop(args);
            }
        }
    }
}
pub(super) async fn open(
    key: Key8,
    admission: Admission,
    ctx: Context,
    stamp: Rc<RefCell<Option<CurrentStamp8>>>,
    stopped: Rc<Cell<bool>>,
    mut pending: Pending8,
) -> std::result::Result<Opened8, OpenFailure8> {
    let identity = key.identity.clone();
    let check = || {
        if stopped.get()
            || !stamp
                .borrow()
                .as_ref()
                .is_some_and(|s| s.matches(&identity))
        {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Source8 Main preparation cancelled",
            ))
        } else {
            Ok(())
        }
    };
    check().map_err(|error| OpenFailure8 {
        error,
        joined: true,
    })?;
    let result: Result<Opened8> = async {
        let ready = js::wait(js::field(&pending.controller, "ready")?).await?;
        js::ready(&ready, key.identity.profile)?;
        check()?;
        let original = admission.paint_metadata_scope().capacity_snapshot();
        let cap = if original.reserved {
            original.ceiling
        } else {
            super::factory::MAX
                .min(admission.paint_metadata_limit() as u64)
                .min(original.ceiling)
        };
        let credit = js::call(
            &pending.controller,
            "reserveMain",
            &[JsValue::from_f64(cap as f64)],
        )?;
        let factory = Factory8::new(&admission, cap, Credit8::new(credit)?)?;
        let prefix = Preparation8::new(key, factory, &check)?;
        if !prefix.blend {
            let identity = prefix.identity.clone();
            // Explicit paid NORMAL result; no generic error-driven fallback. No
            // source or opaqueowner ever existed in this actual empty channel.
            drop(prefix);
            let close = js::call(&pending.controller, "close", &[])?;
            js::wait(close).await?;
            pending.armed = false;
            return Ok(Opened8::Normal(identity));
        }
        let snapshot = prefix.finish(&check)?;
        let current = stamp.borrow().clone().ok_or_else(denied)?;
        let owner = Owner8::new(snapshot, ctx, current);
        pending.owner = Some(owner.clone());
        let (control, source) = owner.source()?;
        let opaque: JsValue = MainBlendOwner8::new(owner.clone()).into();
        let facade = js::facade(&opaque).map_err(js::error)?;
        drop(opaque);
        let opening = js::call(
            &pending.controller,
            "open",
            &[control, source.into(), facade],
        )?;
        match JsFuture::from(js_sys::Promise::resolve(&opening)).await {
            Ok(session) => {
                owner.opened()?;
                pending.armed = false;
                pending.owner = None;
                Ok(Opened8::Blend(Client8 {
                    controller: pending.controller.clone(),
                    session,
                    owner: Some(owner),
                    serial: Cell::new(0),
                    cancel: RefCell::new(None),
                    closed: false,
                    host: pending.host.clone(),
                }))
            }
            Err(raw) => {
                // closed=true is only trusted from this actual controller's strict
                // correlation/consuming-close path, never from caller metadata.
                if js::data(&raw, "closed").ok().and_then(|v| v.as_bool()) == Some(true) {
                    owner.opened()?;
                }
                Err(js::error(raw))
            }
        }
    }
    .await;
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            let closing = js::call(&pending.controller, "close", &[]);
            drop(pending.owner.take());
            let joined = match closing {
                Ok(promise) => js::wait(promise).await.is_ok(),
                Err(_) => false,
            };
            pending.armed = false;
            Err(OpenFailure8 { error, joined })
        }
    }
}

impl Client8 {
    pub fn refresh(&self, stamp: CurrentStamp8, view: Option<View>) {
        if let Some(owner) = &self.owner {
            owner.refresh(stamp, view);
        }
    }
    pub fn ready(&self) -> Result<bool> {
        self.owner.as_ref().ok_or_else(denied)?.ready()
    }
    pub fn cancel(&self) {
        if self.cancel.borrow().is_none()
            && let Ok(promise) = js::call(&self.session, "cancel", &[])
        {
            *self.cancel.borrow_mut() = Some(promise);
        }
    }
    pub fn prepare_frame<A, F: std::future::Future>(
        &self,
        view: View,
        make: fn(A) -> F,
    ) -> Result<PreparedFrame8<A, F>> {
        let owner = self.owner.as_ref().ok_or_else(denied)?.clone();
        let token = self.serial.get().checked_add(1).ok_or_else(denied)?;
        // Host only calls for a new changed desired view. Samefailed request is
        // held in Display8, never silently retried with a replacement counter.
        let credit = owner.prepare_view(token, view, make)?;
        self.serial.set(token);
        let session = self.session.clone();
        let cancel = self.cancel.borrow_mut().take();
        let running = Running8(owner.clone());
        Ok((
            FrameRun8 {
                owner,
                view,
                session,
                cancel,
                _running: running,
            },
            credit,
        ))
    }
    pub fn install(&self, publication: CpuPublication8) -> Result<eframe::egui::TextureHandle> {
        self.owner.as_ref().ok_or_else(denied)?.install(publication)
    }
    pub fn close(mut self) -> Result<JsValue> {
        self.closed = true;
        let closing = js::call(&self.controller, "close", &[]);
        drop(self.owner.take());
        drop(self); // Client/strong S root gone before any close await.
        closing
    }
}

// Fixed Rust host carriers; JS object/runtime backing is a separate boundary.
pub(super) fn control_bytes() -> Result<u64> {
    let bytes = 2 * std::mem::size_of::<Client8>()
        + 2 * std::mem::size_of::<Result<Client8>>()
        + 2 * std::mem::size_of::<Pending8>()
        + 2 * std::mem::size_of::<FrameRun8>()
        + 2 * std::mem::size_of::<Result<FrameRun8>>()
        + 2 * std::mem::size_of::<Result<PreparedFrame8<(), std::future::Ready<()>>>>()
        + 2 * std::mem::size_of::<Reply8>()
        + 2 * std::mem::size_of::<Result<Reply8>>();
    u64::try_from(bytes).map_err(|_| denied())
}

type PreparedFrame8<A, F> = (FrameRun8, super::future::SourceCredit8<A, F>);
#[derive(Debug)]
struct Running8(Owner8);
impl Drop for Running8 {
    fn drop(&mut self) {
        self.0.finish_run();
    }
}
#[derive(Debug)]
pub(super) struct FrameRun8 {
    owner: Owner8,
    view: View,
    session: JsValue,
    cancel: Option<JsValue>,
    _running: Running8,
}
pub(super) async fn run_frame8(run: FrameRun8) -> Result<Reply8> {
    let FrameRun8 {
        owner,
        view,
        session,
        cancel,
        _running,
    } = run;
    if let Some(cancel) = cancel {
        js::wait(cancel).await?;
    }
    let request = js_sys::Object::new();
    js::set(
        &request,
        "visible",
        js::numbers(&[
            view.visible.x0,
            view.visible.y0,
            view.visible.x1,
            view.visible.y1,
        ]),
    )?;
    js::set(&request, "level", view.level)?;
    let pending = js::call(&session, "view", &[request.into()])
        .map_err(|error| owner.take_failure().unwrap_or(error))?;
    let frame = match js::wait(pending).await {
        Ok(value) => value,
        Err(error) => return Err(owner.take_failure().unwrap_or(error)),
    };
    let publication = match owner.finish(&frame) {
        Ok(value) => value,
        Err(error) => {
            let _ = js::call(&frame, "release", &[]);
            return Err(owner.take_failure().unwrap_or(error));
        }
    };
    Ok(Reply8 {
        identity: owner.identity()?,
        view,
        publication,
    })
}
