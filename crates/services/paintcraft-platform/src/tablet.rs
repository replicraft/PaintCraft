//! Safe native tablet boundary. Advertisements/observations are not hardware qualification.
pub use paintcraft_tablet_native::{
    Axes, Backend, Boundary, CapabilitySnapshot, CaptureEvent, CaptureRecord, Collected,
    CollectedFrame, Collector, ControlSink, Delivery, DeliveryCheck, DeliveryHealth, Device,
    MouseKind, MouseRecord, MouseRoute, NativePacket, PenRoute, RecordFault, RecordSink, Sink,
    Wake, WheelPhase, WindowPosition,
};
// The first source group supplies the actual AppKit monitor. Other native
// backends are being implemented separately; no fallback or support is implied.
#[cfg(target_os = "macos")]
pub use paintcraft_tablet_native::{Installation, install, install_records, install_with_controls};
