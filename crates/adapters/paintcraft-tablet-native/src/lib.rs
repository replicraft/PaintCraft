//! Native event capture. Safe packet logic is separate from reviewed OS FFI.
mod capabilities;
#[cfg(any(target_os = "macos", test))]
mod clock;
mod delivery;
pub use delivery::DeliveryHealth;
mod collector;
mod packet;
mod records;
#[cfg(any(target_os = "macos", test))]
use capabilities::Report;
pub use capabilities::{Axes, Backend, CapabilitySnapshot, Device};
pub use collector::{Collected, CollectedFrame, Collector, MouseRoute, PenRoute};
pub use packet::{Delivery, NativePacket, WindowPosition};
pub use records::{
    Boundary, CaptureEvent, CaptureRecord, MouseKind, MouseRecord, RecordFault, RecordSink,
    WheelPhase,
};
#[cfg(target_os = "macos")]
use paintcraft_core::types::{Error, ErrorCode, Result};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::{any::Any, marker::PhantomData, rc::Rc};
pub type Sink = Arc<dyn Fn(NativePacket) -> bool + Send + Sync>;
pub type Wake = Arc<dyn Fn() + Send + Sync>;
#[cfg(any(target_os = "macos", test))]
#[derive(Clone)]
struct Emitter {
    sink: Sink,
    wake: Wake,
    report: Report,
    gate: delivery::Gate,
}
#[cfg(any(target_os = "macos", test))]
impl Emitter {
    fn new(sink: Sink, wake: Wake, report: Report) -> Self {
        Self {
            sink,
            wake,
            report,
            gate: delivery::Gate::new(),
        }
    }
    fn emit(&self, packet: NativePacket) -> bool {
        self.report.sample(packet.raw);
        // Lock order: delivery gate -> receiver ingress. The native state lock
        // is released before calling Sink; Wake runs after releasing the gate.
        let owned = match self.gate.emit(|| (self.sink)(packet)) {
            delivery::Emission::Delivered(owned) => owned,
            delivery::Emission::FailedNow => {
                self.report.sink_failure();
                true
            }
            delivery::Emission::Fenced => return true,
        };
        self.wake();
        owned
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
impl std::fmt::Debug for Installation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Installation")
            .field("capabilities", &self.capabilities())
            .finish_non_exhaustive()
    }
}
#[cfg(target_os = "macos")]
impl Installation {
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
#[cfg(target_os = "macos")]
pub fn install<W>(window: Arc<W>, sink: Sink, wake: Wake) -> Result<Installation>
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
    let report = Report::new(Backend::AppKit);
    let emitter = Emitter::new(sink, wake, report.clone());
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
