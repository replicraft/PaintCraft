# Tablet input status

Tablet support is being implemented across native and browser frontends. Shared
input and pressure-rendering foundations are implemented and tested. Dedicated
frontend pen capture is not activated yet, and physical tablets, drivers and
platform timing have not been qualified. A pen reaching the existing mouse path
is not evidence of pressure or tilt support.

## Implemented shared foundations

- Captured samples retain device/pointer identity, source clock and epoch,
  timestamp, sequence, button identities, modifiers, optional pressure, tilt,
  twist, contact size and provenance. Missing pressure differs from zero pressure.
- Each sample retains its capture-time document transform. Delayed work does not
  reinterpret earlier points using a new zoom or display scale.
- Bounded, fallible queue handoff preserves actual/coalesced ordering and removes
  semantic duplicates. Predictions cannot enter committed history. Invalid input,
  overflow and lifecycle loss cancel rather than silently truncate a stroke.
- Both barrel-button identities remain available. Contact, eraser-end identity
  and hovering barrel buttons are distinct; hovering alone must not start ink.
- Shared receiver tests cover canvas versus widget ownership, held-button
  transitions, focus/modal/IME cancellation, palm release tails, once-per-frame
  delivery and callback failure after a partial enqueue. Native and browser
  attachment to this receiver remains in progress.

The plain-round PaintCraft pressure policy rounds normalized pressure to Q16 and
uses it to scale the captured diameter. Missing pressure uses the full diameter;
zero contributes no endpoint disk. The geometry is the actual swept round tip
with linearly changing radius, using common tangents or disk containment. A 4×4
coverage grid unions the whole stroke before applying color opacity, so retracing
a translucent stroke does not repeatedly blend its joins.

Preview and release can share immutable source samples and resolved mask storage.
The existing editor preparation and guarded publication path produces one history
item with explicit Brush, Pencil or Erase intent. Tests compare prepared preview,
publication, undo and redo pixels; cancellation, a foreign session and stale
preparation preserve the current document and history. Native/browser controllers
still need to adopt this handoff.

Three mask policies remain distinct:

| Policy | Effect |
| --- | --- |
| Source-over | Apply captured color alpha multiplied by coverage once. Existing omitted-mode commands keep this default. |
| Replace | Interpolate stored straight RGBA channels by coverage. Existing replacement behavior is preserved. |
| Destination-out | Multiply source alpha by coverage, then reduce destination alpha while preserving destination RGB, including hidden RGB at alpha zero. |

Studio transparent pen erasing must use destination-out. Classic canvas-color
pen erasing must retain its separate replacement policy and Erase history intent.
The new policies are not a claim that the frontend pen eraser is already wired.

## Resource and test scope

The pressure handoff admits retained samples, scanner scratch and incremental
resolved-mask capacity through logical byte accounts. Copy-on-write snapshots do
not grow a non-full sample buffer. Only consecutive geometry-identical Q8/Q16
points may coalesce for this diameter policy; the complete raw samples, timestamps
and axes remain retained. Existing bounds include 65,536 geometry points and
1,048,576 subscan rows. A denial cancels private work; it does not publish a prefix.

Synthetic 100 px round-brush controls retain 480 samples over two seconds and
2,400 samples over ten seconds at 240 Hz. Their declared peaks were 7,310,240 and
36,495,264 bytes. These are logical accounting measurements, not measured heap,
RSS, latency, throughput or physical-device results.

Tracked controls include
[receiver arbitration](../crates/foundation/paintcraft-input/src/tablet/receiver/tests.rs),
[pressure geometry](../crates/foundation/paintcraft-raster/src/smooth/pressure.rs),
[retained pen strokes](../crates/editing/paintcraft-tools/tests/pen_stroke.rs), and
[editor handoff](../apps/paintcraft/tests/pen_mask_handoff.rs). They exercise
synthetic samples in headless tests. Actual browser pen delivery and physical
native traces are separate qualification steps.

## Remaining integration and qualification

| Area | Remaining work |
| --- | --- |
| Native capture | Scoped AppKit tablet/proximity, Windows pointer/history, XInput2 and Wayland tablet-v2 adapters, safe installation/removal, display conversion and delivery-health fencing. Native OS adapter work is a separate active slice from the shared/controller/browser work. |
| Browser capture | Dedicated pen identity and coalesced events, pointer capture/loss, cancellation, teardown/restart and real headless-browser delivery proof. Preserve ordinary mouse and finger navigation. |
| Controller adoption | Ownership/IME/draft arbitration, actual pressure-changing pixels, shared preview/release, one undo item, eraser-end/button handling and source cancellation. |
| Brush settings and dynamics | Appropriate captured pressure curves/opacity, persisted controls and capability feedback, and applicable tilt/twist behavior. Recording optional axes does not implement their tool dynamics. |
| Other brush/media profiles | Preserve all existing nine media policies and sampled tips. The plain-round pressure path must not silently replace them; their pressure/dynamic integration remains required. |
| Physical qualification | Real tablet/driver traces on each platform, high-rate acquisition, disconnect/focus/modal recovery and input/display timing. Synthetic pen events and mouse injection do not establish these results. |

The workspace implementation specification remains authoritative: architecture
`NATIVE_INPUT_INTEGRATION.md` and `INPUT_BRIDGE.md`, and features
`BRUSH_DETAILS.md`, under `docs/temp/paintcraft-implementation-spec-v5-hyperfine`.
Comprehensive tablet acceptance is not complete.
