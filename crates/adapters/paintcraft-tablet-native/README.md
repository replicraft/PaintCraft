# Native tablet capture boundary

This adapter contains AppKit event-monitor capture, Windows pointer/history
acquisition and shared packet/delivery logic. Normal macOS startup installs native
capture automatically, with ordinary mouse recovery if installation fails.
Windows consumer integration, Linux acquisition, canonical keyboard/IME ordering and
physical hardware qualification remain unfinished. See the current
[frontend status and startup behavior](../../../docs/TABLET_INPUT.md).

The legacy `with_native_tablet` host is a separate integration path. Its
frame-based genuine mouse/pen merge does not prove source-time chronology and
must not be enabled as the default. The owned macOS loop uses
`install_records` and source-observed mouse/pen ordering. An actual window
startup/render/close smoke test retained native delivery health `Ready` before
close, with zero reported failures. That establishes installation and startup,
not physical pen, mixed-device or keyboard/IME qualification.

Windows acquisition uses a public winit message hook and scoped HWND lifecycle
observers. Its scoped HWND subclass also captures bounded raw key, character,
focus, layout and IME observations before framework parsing. IMM composition
data has one owned reader; actual result, preedit and cancellation remain distinct.
The App consumer refuses these UI records until target binding and transactional
widgets are integrated. Native histories are validated before delivery; callback registration,
record clocks and publication checks share one installation identity. Fixed
capture storage is admitted before native registration. Uncertain callback
removal disables reinstall rather than freeing callable context. Normal close
retires publication authority atomically and wakes the host. Host model tests
and x86-64/ARM64 SDK checks pass; the Windows owned-loop consumer, native touch
consumer and actual Windows runtime remain unqualified.

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
The owned macOS integration now consumes source records and captures display/view
state. Canonical keyboard/IME ordering, Windows consumer integration,
XI2/Wayland acquisition and physical hardware qualification remain required.
Passing collector controls do not establish those results. Native capture
starts automatically on macOS; this does not establish physical qualification.

### Focus recovery in the ordered collector

Focus loss makes a previously observed local mouse hold uncertain: its Up may have occurred in
another application. The retained AppKit monitor observes combined native button
idle on key-window acquisition or a later native callback and emits the explicit
`PointerIdle` recovery boundary. Only that source observation clears uncertain
mouse ownership; framework receipt callbacks cannot authenticate it. No mouse Up
or new Down is fabricated. Close retires the old collector; a new installation
must meet its real native-idle admission condition.

The monitor and its genuine proximity/device metadata remain retained across
focus recovery, so an idle pen already in proximity needs no out/in ritual on the
proven idle path. Interrupted held pen tails remain fenced until their actual
release. When a local mouse hold was lost, if the first activation is a pen contact before any idle observation,
combined pointer button bits cannot establish whether the old physical mouse is
idle. That first-contact recovery case remains unresolved; the implementation
does not subtract pen bits or claim comprehensive focus-return behavior. Headless
source controls establish the state policy, not live AppKit or hardware behavior.

Pen-only focus loss with no local mouse hold does not require an idle observation;
unrelated mouse activity in another application creates no captured ownership.
