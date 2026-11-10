//! Mint-only opaque Main authority; no constructor, domain visitor or Vec ABI.
use super::{
    capture::{CurrentStamp8, Expected8, Identity8},
    denied,
    factory::Factory8,
    frame::{FrameStage8, Funding8},
    future, js,
    operation::Operation8,
    publication::{CpuPublication8, Retirement8},
    snapshot::Snapshot8,
};
use crate::app::filter_editing::render::View;
use eframe::egui::{Context, TextureHandle};
use paintcraft_core::types::{Error, Result, policy::hex};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;
pub(super) struct OwnerData8 {
    snapshot: RefCell<Option<Snapshot8>>,
    active: RefCell<Option<FrameStage8>>,
    funding: RefCell<Option<Funding8>>,
    retirement: RefCell<Retirement8>,
    current: RefCell<Option<CurrentStamp8>>,
    desired: Cell<Option<View>>,
    expected: Cell<Option<Expected8>>,
    failure: RefCell<Option<Error>>,
    issued: Cell<bool>,
    rust_busy: Cell<bool>,
    closing: Cell<bool>,
    cpu_polling: Cell<bool>,
    ctx: Context,
    factory: Factory8,
}
pub(super) struct Owner8(Option<Rc<OwnerData8>>);
impl Clone for Owner8 {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for Owner8 {
    fn drop(&mut self) {
        if let Some(rc) = self.0.take()
            && let Some(data) = Rc::into_inner(rc)
        {
            drop(data);
        }
    }
}
impl std::fmt::Debug for Owner8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainOwner8")
            .field("closing", &self.data().closing.get())
            .field("active", &self.data().expected.get().is_some())
            .finish_non_exhaustive()
    }
}
impl Owner8 {
    fn data(&self) -> &OwnerData8 {
        self.0.as_ref().expect("live Main owner")
    }
    pub fn new(snapshot: Snapshot8, ctx: Context, current: CurrentStamp8) -> Self {
        let factory = snapshot.factory().clone();
        Self(Some(Rc::new(OwnerData8 {
            snapshot: RefCell::new(Some(snapshot)),
            active: RefCell::new(None),
            funding: RefCell::new(None),
            retirement: RefCell::new(Retirement8::default()),
            current: RefCell::new(Some(current)),
            desired: Cell::new(None),
            expected: Cell::new(None),
            failure: RefCell::new(None),
            issued: Cell::new(false),
            rust_busy: Cell::new(false),
            closing: Cell::new(false),
            cpu_polling: Cell::new(false),
            ctx,
            factory,
        })))
    }
    pub fn identity(&self) -> Result<Identity8> {
        self.data()
            .snapshot
            .try_borrow()
            .map_err(|_| denied())?
            .as_ref()
            .map(|s| s.identity.clone())
            .ok_or_else(denied)
    }
    pub fn factory(&self) -> Factory8 {
        self.data().factory.clone()
    }
    fn spend_cleanup(&self, n: u64) -> Result<()> {
        let funding = self.data().funding.borrow().as_ref().cloned();
        if let Some(funding) = funding {
            return funding.spend(n);
        }
        let snapshot = self
            .data()
            .snapshot
            .borrow_mut()
            .take()
            .ok_or_else(denied)?;
        let result = snapshot.spend(n);
        *self.data().snapshot.borrow_mut() = Some(snapshot);
        result
    }
    pub fn refresh(&self, current: CurrentStamp8, desired: Option<View>) {
        let old = self.data().current.borrow_mut().replace(current);
        drop(old); // Last Document teardown must run after the RefMut ends.
        self.data().desired.set(desired);
    }
    fn poll_cpu(&self) -> Result<bool> {
        let data = self.data();
        if data.cpu_polling.replace(true) {
            return Err(denied());
        }
        let mut retirement = std::mem::take(&mut *data.retirement.borrow_mut());
        let result = retirement.poll(&data.ctx);
        *data.retirement.borrow_mut() = retirement;
        data.cpu_polling.set(false);
        result
    }
    pub fn ready(&self) -> Result<bool> {
        if self.data().closing.get()
            || self.data().factory.is_quarantined()
            || self.data().cpu_polling.get()
        {
            return Err(denied());
        }
        self.poll_cpu()
    }
    pub fn prepare_view<A, F: std::future::Future>(
        &self,
        token: u64,
        view: View,
        make: fn(A) -> F,
    ) -> Result<future::SourceCredit8<A, F>> {
        if token == 0 || self.data().rust_busy.get() || !self.ready()? {
            return Err(denied());
        }
        if let Some(funding) = self.data().funding.borrow().as_ref()
            && !funding.js_is_retired()
        {
            return Err(denied());
        }
        let retired = self.data().active.borrow_mut().take();
        drop(retired);
        let retired = self.data().funding.borrow_mut().take();
        drop(retired);
        let operation = Operation8::view(self.data().factory.clone())?;
        operation.spend(1024)?;
        self.identity()?.validate_view(view)?;
        let credit = future::reserve(&self.data().factory, &|n| operation.spend(n), make)?;
        let stage = FrameStage8::admit(operation, view.size)?;
        let columns = u64::from(view.size[0]).div_ceil(64);
        let patches = columns
            .checked_mul(u64::from(view.size[1]).div_ceil(64))
            .ok_or_else(denied)?;
        let calls = patches
            .checked_mul(4)
            .and_then(|n| n.checked_add(8))
            .ok_or_else(denied)?;
        *self.data().funding.borrow_mut() = Some(stage.funding());
        *self.data().active.borrow_mut() = Some(stage);
        self.data()
            .expected
            .set(Some(Expected8 { token, view, calls }));
        self.data().issued.set(false);
        self.data().rust_busy.set(true);
        Ok(credit)
    }
    fn matches(&self, request: &JsValue) -> Result<bool> {
        let data = self.data();
        if data.closing.get() {
            return Ok(false);
        }
        if data.factory.is_quarantined() {
            return Err(denied());
        }
        let Some(mut expected) = data.expected.get() else {
            return Ok(false);
        };
        if expected.calls == 0 {
            return Err(denied());
        }
        if data.desired.get() != Some(expected.view) {
            return Ok(false);
        }
        expected.calls -= 1;
        data.expected.set(Some(expected));
        let identity = self.identity()?;
        if !data
            .current
            .try_borrow()
            .map_err(|_| denied())?
            .as_ref()
            .is_some_and(|s| s.matches(&identity))
        {
            return Ok(false);
        }
        if js::decimal(request, "token")? != expected.token
            || js::decimal(request, "generation")? != expected.token
            || js::data(request, "level")?.as_f64() != Some(f64::from(expected.view.level))
            || js::rect(&js::data(request, "visible")?)? != expected.view.visible
        {
            return Ok(false);
        }
        for (key, value) in [
            ("owner", identity.owner.to_string()),
            ("document", identity.document.document_id.to_string()),
            ("sourceState", identity.document.state_id.to_string()),
            ("draft", identity.draft.to_string()),
            ("profileSha256", hex(&identity.profile)),
        ] {
            if !js::string_equal(&js::data(request, key)?, &value) {
                return Ok(false);
            }
        }
        let ticket = js::data(request, "publicTicket")?;
        if !js::string_equal(
            &js::data(&ticket, "state")?,
            &identity.ticket.state_id.to_string(),
        ) || js::decimal(&ticket, "epoch")? != identity.ticket.publication_epoch
        {
            return Ok(false);
        }
        let source = data.snapshot.try_borrow().map_err(|_| denied())?;
        if !js::string_equal(
            &js::data(request, "sourceSha256")?,
            &hex(&source.as_ref().ok_or_else(denied)?.demand.source_sha256),
        ) {
            return Ok(false);
        }
        Ok(true)
    }
    pub fn source(&self) -> Result<(JsValue, js_sys::ArrayBuffer)> {
        // No RefCell borrow crosses actual external JS calls/account callbacks.
        let mut snapshot = self
            .data()
            .snapshot
            .borrow_mut()
            .take()
            .ok_or_else(denied)?;
        let result = (|| {
            let control = js::open(&snapshot)?;
            snapshot.begin_transfer()?;
            let array = js_sys::Uint8Array::from(snapshot.bytes()?);
            let source = array.buffer();
            drop(array);
            Ok((control, source))
        })();
        *self.data().snapshot.borrow_mut() = Some(snapshot);
        result
    }
    pub fn opened(&self) -> Result<()> {
        let mut snapshot = self
            .data()
            .snapshot
            .borrow_mut()
            .take()
            .ok_or_else(denied)?;
        snapshot.opened();
        *self.data().snapshot.borrow_mut() = Some(snapshot);
        Ok(())
    }
    pub fn finish(&self, result: &JsValue) -> Result<CpuPublication8> {
        let expected = self.data().expected.get().ok_or_else(denied)?;
        let identity = self.identity()?;
        if self.data().closing.get()
            || self.data().desired.get() != Some(expected.view)
            || !self
                .data()
                .current
                .borrow()
                .as_ref()
                .is_some_and(|s| s.matches(&identity))
        {
            return Err(denied());
        }
        if js::data(result, "width")?.as_f64() != Some(f64::from(expected.view.size[0]))
            || js::data(result, "height")?.as_f64() != Some(f64::from(expected.view.size[1]))
            || js::rect(&js::data(result, "region")?)? != expected.view.region
            || js::data(result, "level")?.as_f64() != Some(f64::from(expected.view.level))
        {
            return Err(denied());
        }
        let mut stage = self.data().active.borrow_mut().take().ok_or_else(denied)?;
        // The result rgba getter is the trusted session's borrow. Extracted
        // JsValue/ArrayBuffer/Uint8Array aliases ALL die before release below.
        {
            let rgba = js::field(result, "rgba")?;
            let buffer = rgba.dyn_into::<js_sys::ArrayBuffer>().map_err(js::error)?;
            let array = js_sys::Uint8Array::new(&buffer);
            stage.fill(&array)?;
            drop(array);
            drop(buffer);
        }
        let publication = stage.into_publication()?;
        let _ = js::call(result, "release", &[])?;
        Ok(publication)
    }
    pub fn finish_run(&self) {
        self.data().rust_busy.set(false);
        // Session cannot allocate JS final data before the real admitted guard.
        // If none was issued, this is prospectivecredit only, not a detachproof.
        if !self.data().issued.get() {
            let funding = self.data().funding.borrow().as_ref().cloned();
            if let Some(funding) = funding {
                let _ = funding.js_retired();
            }
            let stage = self.data().active.borrow_mut().take();
            drop(stage);
        }
    }
    pub fn install(&self, publication: CpuPublication8) -> Result<TextureHandle> {
        if self.data().retirement.borrow().is_pending() {
            return Err(denied());
        }
        let texture = publication.install(&self.data().ctx);
        self.data().retirement.borrow_mut().admit(publication)?;
        Ok(texture)
    }
    pub fn take_failure(&self) -> Option<Error> {
        self.data().failure.borrow_mut().take()
    }
    fn record_failure(&self, error: Error) {
        let mut slot = self.data().failure.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }
    fn failed(&self, error: Error) -> JsValue {
        let value = js::fail(&error);
        self.record_failure(error);
        value
    }
    fn quarantine(self) {
        self.data().factory.quarantine();
        std::mem::forget(self);
    }
    fn retire_step(&self) -> Result<bool> {
        let data = self.data();
        data.closing.set(true);
        if data.rust_busy.get() {
            let funding = data.funding.borrow().as_ref().cloned().ok_or_else(denied)?;
            funding.spend(1024)?;
            data.ctx
                .request_repaint_after(std::time::Duration::from_millis(25));
            return Ok(false);
        }
        let stage = data.active.borrow_mut().take();
        drop(stage);
        if let Some(funding) = data.funding.borrow().as_ref()
            && !funding.js_is_retired()
        {
            return Err(denied());
        }
        if !self.poll_cpu()? {
            return Ok(false);
        }
        // On failed OPEN, only the actual controller's validated closed=true
        // correlation may have retired transfer. Unknown close cannot assume it.
        let transfer_pending = data
            .snapshot
            .borrow()
            .as_ref()
            .is_some_and(Snapshot8::transfer_pending);
        if transfer_pending {
            return Err(denied());
        }
        let snapshot = data.snapshot.borrow_mut().take();
        drop(snapshot);
        let funding = data.funding.borrow_mut().take();
        drop(funding);
        let current = data.current.borrow_mut().take();
        drop(current);
        data.expected.set(None);
        Ok(true)
    }
}
/// Minted ONLY by Main Rust after actual original source/grant preparation.
#[derive(Debug)]
#[wasm_bindgen]
pub struct MainBlendOwner8 {
    state: Option<Owner8>,
}
impl MainBlendOwner8 {
    pub(super) fn new(state: Owner8) -> Self {
        Self { state: Some(state) }
    }
}
#[wasm_bindgen]
impl MainBlendOwner8 {
    #[wasm_bindgen(js_name=admitFrame)]
    pub fn admit_frame(
        &self,
        byte_length: f64,
        request: JsValue,
    ) -> std::result::Result<MainFrameGuard8, JsValue> {
        let owner = self.state.as_ref().ok_or_else(|| js::fail(&denied()))?;
        let result = (|| {
            if !owner.matches(&request)? {
                return Err(super::stale());
            }
            if owner.data().issued.get() {
                return Err(denied());
            }
            let funding = owner
                .data()
                .funding
                .borrow()
                .as_ref()
                .cloned()
                .ok_or_else(denied)?;
            let expected = owner.data().expected.get().ok_or_else(denied)?;
            let n = u64::from(expected.view.size[0]) * u64::from(expected.view.size[1]) * 4;
            if byte_length != n as f64 {
                return Err(denied());
            }
            owner.data().issued.set(true);
            Ok(MainFrameGuard8 {
                funding: Some(funding),
            })
        })();
        result.map_err(|e| owner.failed(e))
    }
    pub fn current(&self, request: JsValue) -> bool {
        let Some(owner) = &self.state else {
            return false;
        };
        match owner.matches(&request) {
            Ok(current) => current,
            Err(error) => {
                owner.record_failure(error);
                false
            }
        }
    }
    pub fn retire(mut self) -> std::result::Result<js_sys::Promise, JsValue> {
        let owner = self.state.take().ok_or_else(|| js::fail(&denied()))?;
        owner.data().closing.set(true);
        let factory = owner.factory();
        let meter = owner.clone();
        let args = RetireArgs8 { owner: Some(owner) };
        let tracked = match future::track(&factory, &|n| meter.spend_cleanup(n), args, retire_task8)
        {
            Ok(tracked) => tracked,
            Err((error, args)) => {
                drop(args); // complete paired owner quarantines before returning.
                return Err(js::fail(&error));
            }
        };
        drop(meter);
        drop(factory);
        Ok(wasm_bindgen_futures::future_to_promise(tracked))
    }
}
impl Drop for MainBlendOwner8 {
    fn drop(&mut self) {
        if let Some(owner) = self.state.take() {
            owner.quarantine();
        }
    }
}
/// Mint-only admitted guard, consumed after the actual JS final root clears.
#[derive(Debug)]
#[wasm_bindgen]
pub struct MainFrameGuard8 {
    funding: Option<Funding8>,
}
#[wasm_bindgen]
impl MainFrameGuard8 {
    pub fn release(mut self) -> std::result::Result<(), JsValue> {
        let funding = self.funding.take().ok_or_else(|| js::fail(&denied()))?;
        funding.js_retired().map_err(|e| js::fail(&e))
    }
}
impl Drop for MainFrameGuard8 {
    fn drop(&mut self) {
        if let Some(funding) = self.funding.take() {
            funding.factory().quarantine();
            std::mem::forget(funding);
        }
    }
}
pub(super) fn control_bytes() -> Result<u64> {
    use wasm_bindgen::__rt::{RcRef, WasmRefCell};
    let shared = super::controls::shared::<OwnerData8>()?;
    shared
        .checked_add(super::controls::shared::<WasmRefCell<MainBlendOwner8>>()?)
        .and_then(|n| {
            n.checked_add(super::controls::shared::<WasmRefCell<MainFrameGuard8>>().ok()?)
        })
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<RcRef<MainBlendOwner8>>() as u64))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<MainBlendOwner8>() as u64))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<RetireArgs8>() as u64))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<Result<RetireArgs8>>() as u64))
        .and_then(|n| {
            n.checked_add(
                2 * std::mem::size_of::<std::result::Result<js_sys::Promise, JsValue>>() as u64,
            )
        })
        .and_then(|n| {
            n.checked_add(
                2 * std::mem::size_of::<std::result::Result<MainFrameGuard8, JsValue>>() as u64,
            )
        })
        .ok_or_else(denied)
}

#[derive(Debug)]
struct RetireArgs8 {
    owner: Option<Owner8>,
}
impl Drop for RetireArgs8 {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            owner.quarantine();
        }
    }
}
async fn retire_task8(mut args: RetireArgs8) -> std::result::Result<JsValue, JsValue> {
    loop {
        let owner = args.owner.as_ref().ok_or_else(|| js::fail(&denied()))?;
        match owner.retire_step() {
            Ok(true) => {
                drop(args.owner.take());
                return Ok(JsValue::UNDEFINED);
            }
            Ok(false) => {}
            Err(error) => return Err(js::fail(&error)),
        }
        let promise = js::tick()?;
        JsFuture::from(promise).await?;
    }
}
