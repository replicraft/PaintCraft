//! Native event capture. Safe packet logic is separate from reviewed OS FFI.
mod capabilities;
#[cfg(any(target_os = "macos", test))]
mod clock;
mod delivery;
#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
pub use delivery::TestInstallation;
pub use delivery::{DeliveryCheck, DeliveryHealth};
mod collector;
mod packet;
mod records;
#[cfg(any(target_os = "windows", test))]
pub mod windows;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use capabilities::Report;
pub use capabilities::{Axes, Backend, CapabilitySnapshot, Device};
pub use collector::{Collected, CollectedFrame, Collector, MouseRoute, PenRoute};
pub use packet::{Delivery, NativePacket, WindowPosition};
#[cfg(target_os = "macos")]
use paintcraft_core::types::{Error, ErrorCode, Result};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
pub use records::{
    Boundary, CaptureEvent, CaptureRecord, MouseKind, MouseRecord, RecordFault, RecordSink,
    WheelPhase,
};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::{any::Any, marker::PhantomData, rc::Rc};
pub type Sink = Arc<dyn Fn(NativePacket) -> bool + Send + Sync>;
pub type Wake = Arc<dyn Fn() + Send + Sync>;
/// Context-free lifecycle control, serialized by the same delivery gate as Sink.
pub type ControlSink = Arc<dyn Fn(Boundary) -> std::result::Result<(), RecordFault> + Send + Sync>;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Clone)]
struct Emitter {
    #[cfg(any(target_os = "macos", test))]
    sink: Sink,
    #[cfg(any(target_os = "macos", test))]
    controls: Option<ControlSink>,
    records: Option<RecordSink>,
    record_epoch: Arc<std::sync::atomic::AtomicU64>,
    record_ordinal: Arc<std::sync::atomic::AtomicU64>,
    wake: Wake,
    report: Report,
    gate: delivery::Gate,
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
impl Emitter {
    fn new(sink: Sink, wake: Wake, report: Report) -> Self {
        #[cfg(target_os = "windows")]
        let _ = &sink;
        Self {
            #[cfg(any(target_os = "macos", test))]
            sink,
            #[cfg(any(target_os = "macos", test))]
            controls: None,
            records: None,
            record_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            record_ordinal: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            wake,
            report,
            gate: delivery::Gate::new(),
        }
    }
    #[cfg(any(target_os = "macos", test))]
    fn emit(&self, packet: NativePacket) -> bool {
        self.report.sample(packet.raw);
        // Lock order: delivery gate -> receiver ingress. The native state lock
        // is released before calling Sink; Wake runs after releasing the gate.
        let emission = if self.records.is_some() {
            self.gate
                .emit_checked(|| self.record(CaptureEvent::Tablet(packet)))
        } else {
            self.gate.emit(|| (self.sink)(packet))
        };
        self.finish(emission)
    }
    fn finish(&self, emission: delivery::Emission) -> bool {
        let owned = match emission {
            delivery::Emission::Delivered(owned) => owned,
            delivery::Emission::FailedNow => {
                self.report.sink_failure();
                true
            }
            delivery::Emission::FailedSource => {
                self.report.source_failure();
                true
            }
            delivery::Emission::Fenced => return true,
        };
        self.wake();
        owned
    }
    fn new_records(sink: RecordSink, wake: Wake, report: Report) -> Self {
        let mut emitter = Self::new(Arc::new(|_| true), wake, report);
        emitter.records = Some(sink);
        emitter
    }
    fn record(&self, mut event: CaptureEvent) -> std::result::Result<bool, delivery::Failure> {
        use std::sync::atomic::Ordering;
        let ordinal = self
            .record_ordinal
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| delivery::Failure::Source)?;
        let installation = self.record_epoch.load(Ordering::Acquire);
        if let CaptureEvent::Tablet(packet) = &mut event {
            // Both values were adapter-assigned counters; the record mode uses
            // the shared producer ordinal so widget records keep the same order.
            packet.raw.sequence = ordinal;
        }
        self.records.as_ref().ok_or(delivery::Failure::Source)?(CaptureRecord {
            installation,
            ordinal,
            event,
        })
        .map_err(|_| delivery::Failure::Sink)?;
        Ok(true)
    }
    #[cfg(any(target_os = "macos", test))]
    fn observe(&self, event: CaptureEvent) {
        if self.records.is_some() {
            self.finish(self.gate.emit_checked(|| self.record(event)));
        } else if let CaptureEvent::Boundary { kind, .. } = event {
            if let Some(controls) = &self.controls {
                self.finish(self.gate.emit_checked(|| {
                    controls(kind)
                        .map(|()| true)
                        .map_err(|_| delivery::Failure::Sink)
                }));
            } else if kind == Boundary::ViewInvalidated {
                // A packet-only client cannot safely keep using its old layout.
                // Never silently lose invalidation with publication authority Ready.
                self.source_failure();
            }
        } else {
            self.source_failure();
        }
    }
    fn wake(&self) {
        if self.report.wake_failed() {
            return;
        }
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.wake)())).is_err() {
            self.report.wake_failure();
        }
    }
    fn source_failure(&self) {
        self.gate.fail();
        self.report.source_failure();
        self.wake();
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
#[cfg(target_os = "macos")]
pub struct Installation {
    native: macos::Monitor,
    report: Report,
    gate: delivery::Gate,
    // Fields are retired in this order: native callbacks, then native window.
    _window: Arc<dyn Any + Send + Sync>,
    _main_thread: PhantomData<Rc<()>>,
}
#[cfg(target_os = "macos")]
impl Drop for Installation {
    fn drop(&mut self) {
        // Fail first, including reentrant Drop inside a callback; native fields
        // then retire callbacks before the retained window is released.
        self.gate.fail();
    }
}
#[cfg(target_os = "macos")]
impl std::fmt::Debug for Installation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Installation")
            .field("capabilities", &self.capabilities())
            .finish_non_exhaustive()
    }
}
#[cfg(target_os = "macos")]
impl Installation {
    pub fn delivery_check(&self) -> DeliveryCheck {
        self.gate.check()
    }
    pub fn capabilities(&self) -> CapabilitySnapshot {
        self.report.snapshot()
    }
    /// Gate only a short safe snapshot/discard handoff. Lock order is this gate
    /// before receiver ingress. Do work/publication after release; never invoke
    /// Sink or pump native events from this closure. Native cancel() is callback-free.
    pub fn with_delivery<T>(&self, take: impl FnOnce(DeliveryHealth) -> T) -> T {
        self.gate.with_delivery(take)
    }

    /// Fence native physical tails only; this never calls Sink. The host must
    /// also cancel/discard its safe receiver state. Safe inside with_delivery.
    pub fn cancel(&self) {
        self.native.cancel();
    }
}
/// Packet-only compatibility API. A backing/layout invalidation fails delivery
/// closed; clients retaining a GUI view should use install_with_controls.
#[cfg(target_os = "macos")]
pub fn install<W>(window: Arc<W>, sink: Sink, wake: Wake) -> Result<Installation>
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
{
    install_packets(window, sink, None, wake)
}
/// Retain the window and deliver native lifecycle changes before a later GUI
/// scale/layout receipt. Controls must not call GUI APIs or pump native events.
#[cfg(target_os = "macos")]
pub fn install_with_controls<W>(
    window: Arc<W>,
    sink: Sink,
    controls: ControlSink,
    wake: Wake,
) -> Result<Installation>
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
{
    install_packets(window, sink, Some(controls), wake)
}
#[cfg(target_os = "macos")]
fn install_packets<W>(
    window: Arc<W>,
    sink: Sink,
    controls: Option<ControlSink>,
    wake: Wake,
) -> Result<Installation>
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
{
    let handle = window.window_handle().map_err(|_| {
        Error::new(
            ErrorCode::CapabilityUnsupported,
            "Native window handle unavailable",
        )
    })?;
    let raw_window_handle::RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "AppKit window required",
        ));
    };
    let mut emitter = Emitter::new(sink, wake, Report::new(Backend::AppKit));
    emitter.controls = controls;
    install_emitter(window, handle, emitter)
}
/// Owned-loop acquisition mode. Genuine mouse events are observed and passed
/// unchanged to AppKit; the owned host must suppress their GUI mirror explicitly.
/// This API alone does not authenticate a join with framework receipt order.
#[cfg(target_os = "macos")]
pub fn install_records<W>(window: Arc<W>, sink: RecordSink, wake: Wake) -> Result<Installation>
where
    W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
{
    let raw = window.window_handle().map_err(|_| {
        Error::new(
            ErrorCode::CapabilityUnsupported,
            "Native window handle unavailable",
        )
    })?;
    let raw_window_handle::RawWindowHandle::AppKit(handle) = raw.as_raw() else {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "AppKit window required",
        ));
    };
    install_emitter(
        window,
        handle,
        Emitter::new_records(sink, wake, Report::new(Backend::AppKit)),
    )
}
#[cfg(target_os = "macos")]
fn install_emitter<W: Any + Send + Sync>(
    window: Arc<W>,
    handle: raw_window_handle::AppKitWindowHandle,
    emitter: Emitter,
) -> Result<Installation> {
    let report = emitter.report.clone();
    let gate = emitter.gate.clone();
    let native = macos::Monitor::new(handle, emitter)?;
    report.installed(true);
    Ok(Installation {
        native,
        report,
        gate,
        _window: window,
        _main_thread: PhantomData,
    })
}

#[cfg(test)]
mod tests;
