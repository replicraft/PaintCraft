//! Public winit pre-dispatch hook and same-thread, scoped HWND ownership.
//!
//! No callback pumps messages or calls GUI code. Windows-supplied pointers are
//! dereferenced only in private callbacks registered through typed public APIs.
//! History buffers and retained Target state are logically budgeted. The small
//! bounded observer Rc, callback Rc boxes, Report locks and OS-owned handles are
//! bootstrap metadata, outside this resource account; allocator overhead is not
//! estimated. No model/contact arrays are stored in that bootstrap Rc.
//! Uncertain native removal permanently poisons this Hook against reinstall,
//! bounding retained callback metadata instead of accumulating it on retries.
use super::cutoff::Barrier;
use super::model::{self, Info, Message, Output, State};
use super::resident::Resident;
use crate::{
    Backend, Boundary, CapabilitySnapshot, CaptureEvent, DeliveryHealth, Emitter, RecordSink,
    Report, Wake,
};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::ScreenToClient,
    System::{
        Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        SystemInformation::GetTickCount64,
        Threading::GetCurrentThreadId,
    },
    UI::{
        HiDpi::*,
        Input::{KeyboardAndMouse::*, Pointer::*},
        Shell::{RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
use winit::platform::windows::EventLoopBuilderExtWindows;
mod control;
mod devices;
mod lifecycle;
mod mouse;
mod pen_native;
#[cfg(test)]
mod tests;
mod touch_native;
const LIMIT: usize = 4096;

fn error(message: &'static str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}

/// Actual acquisition context. Sent messages have no retrieved MSG clock;
/// pen APIs supply their own source clock. Never invent MSG.time for them.
#[derive(Clone, Copy)]
struct NativeMessage {
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
    ticks: Option<u32>,
    screen: Option<POINT>,
}
impl From<&MSG> for NativeMessage {
    fn from(message: &MSG) -> Self {
        Self {
            hwnd: message.hwnd,
            message: message.message,
            w_param: message.wParam,
            l_param: message.lParam,
            ticks: Some(message.time),
            screen: Some(message.pt),
        }
    }
}

/// Must be attached before constructing the owned winit event loop. This hook
/// replaces winit pen translation; the host suppresses genuine mouse GUI mirrors
/// and consumes canonical records instead. Client PT_TOUCH is replaced by
/// canonical Finger records. The owned host must map these to GUI touch before
/// enabling this backend. Attaching replaces the builder's prior msg hook.
#[derive(Clone, Default)]
pub struct Hook(Rc<Inner>);
#[derive(Default)]
struct Inner {
    target: RefCell<Option<Resident<Target>>>,
    processing: Cell<bool>,
    active: Cell<bool>,
    hwnd: Cell<usize>,
    reentered: Cell<bool>,
    installation: Cell<u64>,
    retirement_failed: Cell<bool>,
}
/// Refdata belongs to one installation, even when Windows cannot retire it.
/// Never borrow it again after cloning: Sink may drop its Installation.
struct Context {
    inner: Rc<Inner>,
    installation: u64,
}
impl Context {
    fn current(&self) -> bool {
        self.inner.active.get()
            && !self.inner.retirement_failed.get()
            && self.inner.installation.get() == self.installation
    }
}
struct Target {
    hwnd: HWND,
    emitter: Emitter,
    state: State,
    touch: super::touch::State,
    barrier: Barrier,
    mouse_recovery: bool,
    storage: Storage,
    _window: Arc<dyn Any + Send + Sync>,
}
struct Storage {
    history: Vec<POINTER_PEN_INFO>,
    touch_history: Vec<POINTER_TOUCH_INFO>,
    fingers: Vec<super::touch::Finger>,
    transforms: Vec<INPUT_TRANSFORM>,
    copied: Vec<Info>,
    output: Vec<Output>,
    events: Vec<CaptureEvent>,
    // Storage is freed before its admission is released.
    _account: BudgetAccount,
}
impl Storage {
    fn new(budget: &SharedBudget) -> Result<Self> {
        let mut account = budget.account(AllocationClass::PaintMetadata);
        let bytes = LIMIT
            .checked_mul(
                size_of::<POINTER_PEN_INFO>()
                    + size_of::<POINTER_TOUCH_INFO>()
                    + size_of::<super::touch::Finger>()
                    + size_of::<INPUT_TRANSFORM>()
                    + size_of::<Info>()
                    + size_of::<Output>()
                    + size_of::<CaptureEvent>(),
            )
            .ok_or_else(|| error("Windows history size overflow"))?;
        account.resize(bytes as u64 + (3 * size_of::<CaptureEvent>()) as u64)?;
        let mut history = Vec::new();
        let mut touch_history = Vec::new();
        let mut fingers = Vec::new();
        let mut transforms = Vec::new();
        let mut copied = Vec::new();
        let mut output = Vec::new();
        let mut events = Vec::new();
        history
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Windows history allocation refused"))?;
        touch_history
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Touch history allocation refused"))?;
        fingers
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Touch record allocation refused"))?;
        transforms
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Windows transform allocation refused"))?;
        copied
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Windows history allocation refused"))?;
        output
            .try_reserve_exact(LIMIT)
            .map_err(|_| error("Windows history allocation refused"))?;
        events
            .try_reserve_exact(LIMIT + 3)
            .map_err(|_| error("Windows record allocation refused"))?;
        let actual = history.capacity() * size_of::<POINTER_PEN_INFO>()
            + touch_history.capacity() * size_of::<POINTER_TOUCH_INFO>()
            + fingers.capacity() * size_of::<super::touch::Finger>()
            + transforms.capacity() * size_of::<INPUT_TRANSFORM>()
            + copied.capacity() * size_of::<Info>()
            + output.capacity() * size_of::<Output>()
            + events.capacity() * size_of::<CaptureEvent>();
        account.resize(actual as u64)?;
        history.resize(LIMIT, POINTER_PEN_INFO::default());
        touch_history.resize(LIMIT, POINTER_TOUCH_INFO::default());
        transforms.resize(LIMIT, INPUT_TRANSFORM::default());
        Ok(Self {
            history,
            touch_history,
            fingers,
            transforms,
            copied,
            output,
            events,
            _account: account,
        })
    }
}
impl Hook {
    /// Registration is typed: callers cannot invoke a safe raw-pointer decoder.
    pub fn attach<T>(&self, builder: &mut winit::event_loop::EventLoopBuilder<T>) {
        let inner = self.0.clone();
        builder.with_msg_hook(move |pointer| {
            // SAFETY: winit's documented Windows message hook supplies a live
            // MSG during this synchronous pre-DispatchMessage invocation.
            let message = unsafe { &*pointer.cast::<MSG>() };
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch_hook(&inner, message)
            })) {
                Ok(owned) => owned,
                Err(_) => {
                    let emitter = inner.target.borrow().as_ref().map(|t| t.emitter.clone());
                    if let Some(emitter) = emitter {
                        emitter.source_failure();
                    }
                    true
                }
            }
        });
    }
    pub fn install<W>(
        &self,
        window: Arc<W>,
        sink: RecordSink,
        wake: Wake,
        budget: &SharedBudget,
    ) -> Result<Installation>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        if self.0.retirement_failed.get() {
            return Err(error("Previous native callback retirement is uncertain"));
        }
        let handle = window
            .window_handle()
            .map_err(|_| error("Windows handle unavailable"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err(error("Win32 window required"));
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        // SAFETY: a retained HasWindowHandle owns this HWND; no global hooks.
        unsafe {
            if GetWindowThreadProcessId(hwnd, None) != GetCurrentThreadId() {
                return Err(error("Windows acquisition requires the window thread"));
            }
            if GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(hwnd))
                != DPI_AWARENESS_PER_MONITOR_AWARE
            {
                return Err(error("Per-monitor DPI awareness required"));
            }
            if IsMouseInPointerEnabled().as_bool() {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Owned legacy mouse acquisition requires mouse-in-pointer disabled",
                ));
            }
            if mouse::held() {
                return Err(error("Lift mouse buttons before starting ordered capture"));
            }
        }
        if self.0.processing.get() || self.0.target.borrow().is_some() {
            return Err(error("Windows hook already installed"));
        }
        let mut frequency = 0;
        // SAFETY: valid output pointer; native clock frequency is immutable.
        unsafe { QueryPerformanceFrequency(&mut frequency) }
            .map_err(|_| error("Windows performance clock unavailable"))?;
        if frequency <= 0 {
            return Err(error("Invalid Windows performance clock"));
        }
        let storage = Storage::new(budget)?;
        let barrier = Barrier::observe()?;
        let report = Report::new(Backend::WindowsPointer);
        report.reserve_devices(budget)?;
        let emitter = Emitter::new_records(sink, wake, report.clone());
        let epoch = emitter.gate.source_epoch();
        if epoch == 0 {
            return Err(error("Windows source epoch exhausted"));
        }
        emitter.record_epoch.store(epoch, Ordering::Release);
        let gate = emitter.gate.clone();
        // Admit and allocate retained fixed state before any native side effect.
        let target = Resident::new(budget, || Target {
            hwnd,
            emitter,
            state: State::new(epoch, frequency as u64),
            touch: super::touch::State::new(epoch, frequency as u64),
            barrier,
            mouse_recovery: false,
            storage,
            _window: window.clone(),
        })?;
        let notifications = devices::Notifications::new(&self.0, epoch)?;
        let context = Box::new(Context {
            inner: self.0.clone(),
            installation: epoch,
        });
        let context = Box::into_raw(context);
        let id = context as usize;
        // SAFETY: same owning thread, private callback, stable boxed Rc retained
        // until RemoveWindowSubclass succeeds or WM_NCDESTROY retires it.
        if !unsafe { SetWindowSubclass(hwnd, Some(lifecycle::subclass), id, id) }.as_bool() {
            unsafe {
                drop(Box::from_raw(context));
            }
            return Err(error("Windows lifecycle observer installation failed"));
        }
        *self.0.target.borrow_mut() = Some(target);
        self.0.hwnd.set(hwnd.0 as usize);
        self.0.installation.set(epoch);
        self.0.active.set(true);
        report.installed(true);
        Ok(Installation {
            inner: self.0.clone(),
            hwnd,
            context,
            report,
            gate,
            _window: window,
            _notifications: Some(notifications),
            _thread: PhantomData,
        })
    }
}
/// Same-thread guard. Its native observation is retired before HWND ownership.
pub struct Installation {
    inner: Rc<Inner>,
    hwnd: HWND,
    context: *mut Context,
    report: Report,
    gate: crate::delivery::Gate,
    _window: Arc<dyn Any + Send + Sync>,
    _notifications: Option<devices::Notifications>,
    _thread: PhantomData<Rc<()>>,
}
impl Installation {
    pub fn capabilities(&self) -> CapabilitySnapshot {
        self.report.snapshot()
    }
    pub fn delivery_check(&self) -> crate::DeliveryCheck {
        self.gate.check()
    }
    pub fn with_delivery<T>(&self, take: impl FnOnce(DeliveryHealth) -> T) -> T {
        self.gate.with_delivery(take)
    }
    pub fn cancel(&self) {
        if let Some(target) = self.inner.target.borrow_mut().as_mut() {
            target.fence();
        }
    }
}
impl Drop for Installation {
    fn drop(&mut self) {
        self.inner.active.set(false);
        self.gate.fail();
        let retired = self.inner.target.borrow_mut().take();
        self.report.installed(false);
        // SAFETY: guard is !Send and retained HWND has the owning thread.
        if unsafe {
            RemoveWindowSubclass(self.hwnd, Some(lifecycle::subclass), self.context as usize)
        }
        .as_bool()
            || !unsafe { IsWindow(Some(self.hwnd)) }.as_bool()
        {
            unsafe {
                drop(Box::from_raw(self.context));
            }
        } else {
            // Do not free refdata if Windows may still call it. Disable ingress
            // and retain only this small Rc box rather than risking UAF.
            self.gate.fail();
            self.inner.retirement_failed.set(true);
        }
        drop(self._notifications.take());
        drop(retired);
    }
}
fn ingress(inner: &Rc<Inner>, message: &NativeMessage) -> bool {
    if !inner.active.get() || inner.hwnd.get() != message.hwnd.0 as usize {
        return false;
    }
    if inner.processing.replace(true) {
        inner.reentered.set(true);
        return true;
    }
    let emitter = inner.target.borrow().as_ref().map(|t| t.emitter.clone());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(mut target) = inner.target.borrow_mut().take() else {
            return (true, false);
        };
        let processed = process(&mut target, message);
        // This observation follows the actual MSG's records. It cannot grant
        // the current pen edge ownership retroactively over an uncertain mouse.
        if let Some(event) = target.mouse_idle() {
            target.storage.events.push(event);
        }
        // Target retains the charged scratch/output buffers and HWND throughout
        // external delivery, even if Sink drops Installation reentrantly.
        let emitter = target.emitter.clone();
        let owned = match processed {
            Ok((owned, native_default)) => {
                let delivered =
                    if target.storage.events.is_empty() && !owned && !inner.reentered.get() {
                        false
                    } else {
                        super::batch::deliver(&emitter, &target.storage.events, owned, || {
                            !inner.reentered.replace(false) && inner.active.get()
                        })
                    };
                (delivered, native_default)
            }
            Err(_) => {
                emitter.report.unclassified();
                emitter.source_failure();
                (true, false)
            }
        };
        target.storage.output.clear();
        target.storage.events.clear();
        if inner.active.get() {
            *inner.target.borrow_mut() = Some(target);
        }
        owned
    }));
    inner.processing.set(false);
    match result {
        Ok((owned, native_default)) => {
            if native_default && inner.active.get() {
                // Preserve native chrome through the documented OS default,
                // bypassing winit's frame-wide Touch cast/SkipPointerFrameMessages.
                // State and Gate are released before this potentially modal call.
                let window = inner.target.borrow().as_ref().map(|t| t._window.clone());
                if window.is_some() {
                    unsafe {
                        DefWindowProcW(
                            message.hwnd,
                            message.message,
                            message.w_param,
                            message.l_param,
                        );
                    }
                }
                true
            } else {
                owned
            }
        }
        Err(_) => {
            if let Some(emitter) = emitter {
                emitter.source_failure();
            }
            true
        }
    }
}
fn process(target: &mut Target, message: &NativeMessage) -> Result<(bool, bool)> {
    target.storage.output.clear();
    target.storage.events.clear();
    let phase = match message.message {
        WM_POINTERDOWN | WM_NCPOINTERDOWN => Some(Message::Down),
        WM_POINTERUP | WM_NCPOINTERUP => Some(Message::Up),
        WM_POINTERUPDATE | WM_NCPOINTERUPDATE => Some(Message::Update),
        WM_POINTERENTER => Some(Message::Enter),
        WM_POINTERLEAVE => Some(Message::Leave),
        _ => None,
    };
    if let Some(phase) = phase {
        let id = message.w_param.0 as u16 as u32;
        let mut kind = POINTER_INPUT_TYPE::default();
        // SAFETY: native pointer query while this exact MSG is current; no pump.
        unsafe { GetPointerType(id, &mut kind) }.map_err(|_| error("Pointer type query failed"))?;
        let native_chrome = matches!(
            message.message,
            WM_NCPOINTERDOWN | WM_NCPOINTERUP | WM_NCPOINTERUPDATE
        );
        let flags = (message.w_param.0 >> 16) as u32;
        let owned = if kind == PT_PEN {
            pen_native::acquire(target, id, phase, flags, native_chrome)?
        } else if kind == PT_TOUCH {
            let owned = touch_native::acquire(target, id, phase, flags, native_chrome)?;
            for finger in &target.storage.fingers {
                target.storage.events.push(CaptureEvent::Finger {
                    confidence: if finger.confidence == Some(true) {
                        paintcraft_input::tablet::FingerConfidence::High
                    } else {
                        paintcraft_input::tablet::FingerConfidence::Unspecified
                    },
                    in_contact: finger.in_contact,
                    primary: finger.primary,
                    identity: finger.identity,
                    phase: finger.phase,
                    time: finger.time,
                    provenance: finger.provenance,
                    position: finger.position,
                });
            }
            if let Some((identity, time)) = target.touch.retired() {
                target
                    .storage
                    .events
                    .push(CaptureEvent::IdentityRetired { identity, time });
            }
            owned
        } else {
            return Ok((false, false));
        };
        return Ok((owned, !owned));
    }
    mouse::observe(target, message).map(|owned| (owned, false))
}
/// A same-domain source boundary, never a sort joining clocks. Historical
/// coordinates/modifiers before this cutoff cannot be reinterpreted using the
/// consumer's later geometry or queue keyboard state.
impl Barrier {
    fn observe() -> Result<Self> {
        let mut performance = 0;
        unsafe { QueryPerformanceCounter(&mut performance) }
            .map_err(|_| error("Native boundary clock unavailable"))?;
        Ok(Self {
            performance: u64::try_from(performance)
                .map_err(|_| error("Native boundary clock invalid"))?,
            ticks: unsafe { GetTickCount64() },
        })
    }
    fn admits(&self, performance: u64, ticks: u32) -> bool {
        self.admits_at(performance, ticks, unsafe { GetTickCount64() })
    }
}
impl std::fmt::Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsHook")
            .field("installed", &self.0.active.get())
            .finish()
    }
}
impl std::fmt::Debug for Installation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsInstallation")
            .field("capabilities", &self.capabilities())
            .finish_non_exhaustive()
    }
}

impl Target {
    fn fence(&mut self) {
        self.state.fence();
        self.touch.fence();
    }
    fn mouse_idle(&mut self) -> Option<CaptureEvent> {
        if self.mouse_recovery
            && unsafe { GetForegroundWindow() } == self.hwnd
            && unsafe { GetFocus() } == self.hwnd
            && !unsafe { mouse::held() }
        {
            self.mouse_recovery = false;
            return Some(CaptureEvent::Boundary {
                kind: Boundary::PointerIdle,
                time: None,
            });
        }
        None
    }
}

fn pointer_flags_match(flags: u32, message_flags: u32) -> bool {
    let mask = model::IN_RANGE | model::CONTACT;
    flags & mask == message_flags & mask
}

fn dispatch_hook(inner: &Rc<Inner>, message: &MSG) -> bool {
    // Thread/WM_QUIT/foreign messages retain the exact selected winit path.
    // No temporary native scope is exposed as a framework-event origin proof:
    // selected winit can flush older buffered events inside the current call.
    if !inner.active.get() || inner.hwnd.get() != message.hwnd.0 as usize {
        return false;
    }
    if matches!(message.message, WM_POINTERENTER | WM_POINTERLEAVE) {
        return false;
    }
    ingress(inner, &NativeMessage::from(message))
}
