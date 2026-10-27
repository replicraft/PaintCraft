# Native tablet capture boundary

This active adapter contains a real AppKit event-monitor implementation and safe
packet/delivery logic. It is exposed by `paintcraft-platform/tablet`; App startup
is not hooked in this source group. It is an initial implementation slice, not a
claim of comprehensive native tablet support or physical hardware qualification.
Windows, XInput2 and Wayland acquisition remain required work.

The AppKit monitor is scoped to the retained native view/window. It monitors
local tablet/proximity events and documented tablet-subtype mouse events, not
keys or IME. Actual capability bits govern pressure, tilt and rotation presence;
reported zero pressure stays `Some(0)`, unavailable axes stay `None`. Tip contact
is independent of pressure and barrel state. Eraser identity and lower/upper
barrel bits survive. Coordinates use AppKit's view/backing conversion, followed
by one host conversion to egui points under its captured-view lock.

All classified AppKit pen records use `Delivery::ReceiverPointer`: the safe host
routes unowned input to widgets and owns canvas arbitration. The native monitor
consumes their tablet mouse promotion, including exact duplicate/up records.
Unrelated mouse events pass unchanged, and tablet-only getters are never called
on them. Initial native-titlebar/button contacts stay native through their tail;
client contacts remain receiver-routed when dragged outside the view. A copied
mouse-up cannot switch route merely because focus changed. An unknown device or
proximity description is never fabricated into pen attributes. Positively
identified tablet-subtype client contacts are suppressed and counted in
`unclassified_points` until genuine proximity metadata arrives and every held
button lifts. Native chrome contacts retain their route through metadata arrival.
Local app-wide proximity metadata is observed even while this window is inactive;
point capture remains restricted to its retained window. Lift the pen and move it
out of range and back to retry acquisition. The unidentified route table is
bounded to 64 devices; overflow fails closed for further unidentified tablet
records until reinstall, and newly learned devices must first release their tail.
Genuine ordinary mouse events are unchanged. Startup while already in proximity
and driver-specific sequences still need physical qualification.
No AppKit historical-sample API or completeness beyond delivered events is claimed.

Window focus/close observers cancel contacts. Cancelled physical tails remain
fenced until all buttons lift; another device is not cancelled for one device's
malformed timestamp. Observer/monitor tokens and Cocoa objects remain retained
through main-thread removal. Callbacks execute without the native state lock;
Rust callback unwinds are contained. `Installation` is not Send and keeps the
native window owner until monitor retirement completes.

`NativePacket` carries native units separately from `RawSample`. Hosts call
`into_logical(pixels_per_point)` inside the same lock that captures owner/view,
then `RawSample::at(transform)`. Position-less lifecycle packets have no invented
origin. Advertised axes and observed values are separate from hardware evidence.

The delivery lock order is **delivery gate → receiver ingress**. Sink execution
and failure recording share this gate with `Installation::with_delivery`, so a
frame cannot drain a callback's partial enqueue before observing `Failed`. The
gated closure only snapshots/discards safe receiver state. On failure it must
retire queued/unpublished input and the private stroke; work/publication follows
after release. Native `Installation::cancel()` only fences native state and never
calls Sink, so it can be used in that retirement without reentering the gate.
Guard Drop removes native callbacks without delivering a fictional cancellation
through a failed sink. The host owns safe receiver retirement on removal.

Wake runs after the gate is released. A failed Sink/source callback fences later
delivery; its first wake makes the failure observable. Wake-only failure is
reported separately and does not invalidate data already completely delivered.
The application must still check delivery health at each handoff before any
publication, even if a broken Wake delays the next natural frame. No successful
cancellation callback is inferred after a Sink failure.

Unit controls use source records and a deterministic partial-enqueue/concurrent
handoff test. They do not create a native window, install a live monitor, or inject
OS input. ARM macOS tests and Intel macOS compilation are distinct from physical
pressure, hotplug, capture, mixed DPI, or original driver execution. The host
receiver integration and Windows/Linux acquisitions are not qualified by them.

Primary references are Apple's [local event-monitor lifecycle](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)
and [NSEvent tablet API](https://developer.apple.com/documentation/appkit/nsevent),
plus the public `NX_TABLET_CAPABILITY_*` constants in SDK `IOKit/hidsystem/IOLLEvent.h`.
The existing locked objc2 bindings expose these public APIs. Platform/core/App
keep `unsafe_code=forbid`; this crate denies unsafe by default and permits it only
in its small OS FFI module. Pure state/tests restore deny inside that module.
