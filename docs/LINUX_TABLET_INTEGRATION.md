# Linux tablet integration investigation

Status: source investigation, 2026-10-27. No Linux tablet adapter is installed
by PaintCraft. Linux source checks do not qualify compositor, X server or hardware
delivery. This records the acquisition choices still needed by the
[tablet integration](TABLET_INPUT.md).

## Findings in the locked dependencies

The workspace selects winit 0.30.13. Its public X11 and Wayland event-loop
extensions do not expose a raw input-event callback. The X11 error hook reports
Xlib errors; it cannot be used as an input hook. The ordinary `Touch.force` field
does not supply a pen's identity, proximity, barrel buttons, tilt or twist.

Wayland connection reuse is possible. Winit's `rwh_06` display handle contains
the actual `wl_display` pointer. Its Wayland backend uses `client_system`, and
the locked wayland-backend 0.3.17 provides `Backend::from_foreign_display` in
guest mode. A narrowly reviewed unsafe boundary could borrow this display while
retaining the owning window/event loop. It must destroy its protocol objects
before that display or its guest backend disappears. Guest mode does not transfer
ownership of the display.

This is not yet an ordered integration. A guest queue and winit's queue dispatch
their own callbacks. Polling one after the other cannot prove the ordering of
tablet events against framework mouse, keyboard or IME callbacks. Timestamps,
coordinates and frame positions cannot repair that missing provenance.

Sources inspected in the locked crates:

- `winit/src/platform/{x11,wayland}.rs`: public event-loop extensions.
- `winit/src/platform_impl/linux/wayland/event_loop/mod.rs`:
  `raw_display_handle_rwh_06`.
- `winit/Cargo.toml`: Wayland `client_system` feature.
- `wayland-backend/src/sys/mod.rs`: guest-display lifetime and cleanup contract.
- `wayland-client/src/event_queue.rs`: queue-specific dispatch and read contract.

## Wayland tablet-v2 candidate

The protocol supplies tool capabilities, per-tool events and frame boundaries.
Pressure, tilt and rotation must remain optional according to the advertised
capabilities. Tip contact and tool proximity are separate. A protocol proximity
exit can also describe movement between surfaces; it is not sufficient evidence
to reclaim an identity's complete physical clock lifetime. See the
[tablet protocol](https://wayland.app/protocols/tablet-v2), including its linked
upstream XML.

The next prototype must bind the actual tablet manager on the existing display,
retain the exact target surface and seat, and admit its bounded object/sample
storage before installing callbacks. It must assemble each protocol frame
without inventing unavailable axes or assigning a receipt timestamp to earlier
events. A wrapping frame clock needs its own explicit domain and lifetime.

Before enabling it, demonstrate a single authoritative order for mouse, pen,
keyboard and IME. An owned seat/input queue is one candidate to investigate;
its relationship to winit's duplicate input, text-input protocol, focus and
configure processing remains unproven. Sharing a display alone does not close
that requirement. Do not take over or replace winit-owned proxy dispatchers.

## X11 XInput2 candidate

The workspace already selects x11rb 0.13.2 for OS integration. A second X
connection may acquire tablet data, but that alone cannot establish its order
relative to winit's mouse, keyboard and IME stream. The prototype must first
establish a supported common event-intake boundary or own those input paths;
do not consume events from winit's connection behind its event loop.

Device discovery must use advertised valuator labels and ranges, rather than
assuming fixed axis numbers or identifying a pen from pressure alone. Hierarchy
changes, proximity, capture, promoted pointer events and clock reuse need exact
device and source identities. These semantics and their actual binding APIs
still need implementation and qualification.

## Required acceptance

For either backend, exercise actual source delivery through Classic and Studio,
Japanese IME, focus/modal/capture loss, display-scale changes, disconnect and
reconnect, pen/eraser/barrel input and competing mouse/touch contacts. Check saved
pixels, preview/release equality and one Undo in addition to recorded source
attributes. Preserve malformed/overflow refusal, resource admission and listener
teardown controls. Measure acquisition/display timing separately from synthetic
geometry checks. Physical tablet and compositor/X server traces remain required.
