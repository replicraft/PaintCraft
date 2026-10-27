# Native tablet capture boundary

This active adapter contains a real AppKit event-monitor implementation and safe
packet/delivery logic. It is exposed by `paintcraft-platform/tablet`; App startup
is not hooked in this source group. It is an initial implementation slice, not a
claim of comprehensive native tablet support or physical hardware qualification.
Windows, XInput2 and Wayland acquisition remain required work.

The native App now has an **opt-in host foundation** (`with_native_tablet`) using
the shared bounded receiver, actual pressure edit path and guarded legacy tool
fallback. Default startup does not call it. It is **not acquisition-ready**:
mixed genuine-mouse/pen ordering is still unresolved before the GUI frame. A
physical mouse Down followed by pen Down before a frame can currently assign
priority to the pen; translated widget records can also reorder a pen release
and later mouse press. Raw-frame bookkeeping and passing headless host tests do
not establish source-time chronology. Do not enable capture by default or claim
native tablet support until a maintained ordered acquisition/GUI bridge passes
those counterexamples, focus/IME/capture tests and the native input integration
spike. No physical tablet or live monitor qualification is claimed here.

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

## Source-observed collector slice

`install_records` is an additive AppKit acquisition API for the owned-loop
prototype. It observes real ordinary mouse motion, button edges (native numbers
0–31), wheel delta/unit/phase/momentum, key boundaries and tablet records through
one local monitor. It never labels a mouse observation as a fabricated pen or
mouse device ID. Genuine mouse/key events still pass unchanged to AppKit, and
native chrome remains native. Keyboard characters and IME interpretation are
not read or replaced. Installation requires idle native mouse buttons; it does
not invent an initial Down midway through an existing contact.

A checked per-installation ordinal is assigned under the delivery gate. Tablet
record mode uses that same source ordinal for normalized sequence, retaining its
actual clock and axes. The old `install` mode remains separate and unchanged.
The safe `Collector` admits valid mixed pointer streams in this observed order;
it does not attempt to recover order from translated GUI events. Aggregate mouse
holds, all five common button identities, chrome initial contact and competing
pen/button tails remain distinct. Wheel navigation exposes a `cancel_pen` flag.
Magnify, smart-magnify, rotate and swipe emit explicit Navigation boundaries,
retain original framework gesture delivery and are not relabelled as motion.
Actual tablet cancellation is `PenRoute::Control`, which must reach the host
receiver independently of ink admission. Keyboard/focus native boundaries and
explicit framework IME/reentrant boundaries fence private pen tails; framework
boundaries do not fabricate a native ordinal. The consumer must handle those
controls before further private GUI/tool publication.

Queue storage, pointer metadata and one maximum handoff are admitted through the
caller's `SharedBudget`; both storage charges remain alive through their data.
Record-sink refusals and panics mark failed delivery before a handoff can drain a
partial enqueue. Overflow/order/epoch/clock failures are sticky and discard the
unpublished collector prefix. Per-pen clock history remains across Up and handoff,
separate from reusable contact slots. It is bounded to 64 identities for one
installation; exhaustion refuses rather than evicting history or rebasing time.
Handoff storage reconciles actual Vec capacity before draining any prefix.
This is bounded logical input storage, not an RSS
or complete GUI-framework allocation guarantee.

Source-record tests exercise accepted mouse-release/pen-down and pen-release/
mouse-down sequences, five-button chords, earlier-device priority, wheel units,
zero pressure/axes, boundaries and failures. They do not install a live monitor.
The final source→GUI merge, display/view capture, native execution, Windows
history, XI2/Wayland acquisition and physical hardware qualification are still
required. `install_records` and passing collector controls alone do not make the
unactivated App host acquisition-ready. Default startup remains disabled.
