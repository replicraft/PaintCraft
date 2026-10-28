# Tablet input status

Tablet support is being implemented across native and browser frontends. Shared
input, pressure rendering and guarded editor controllers are implemented and
tested. Browser startup now installs dedicated Pointer Events capture. Desktop
has an opt-in AppKit host foundation; normal desktop startup does not activate it,
and the ordered native event-loop integration remains in progress. Physical
tablets, drivers and platform timing have not been qualified. A pen reaching the
existing mouse path is not evidence of pressure or tilt support.

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
  delivery and callback failure after a partial enqueue. Widget fallback records
  retain source sequence separately from internal cancellation releases. A held
  pen continues to suppress competing pointer input on frames with no new samples.
- UI ownership survives same-workspace document publication and canvas view
  changes. Canvas-owned input keeps exact document/transform guards. Changing
  workspace, interface, focus, modal or IME state retires affected private input.

The plain-round PaintCraft pressure policy rounds normalized pressure to Q16 and
uses it to scale the captured diameter. Missing pressure uses the full diameter;
zero contributes no endpoint disk. The geometry is the actual swept round tip
with linearly changing radius, using common tangents or disk containment. A 4×4
coverage grid unions the whole stroke before applying color opacity, so retracing
a translucent stroke does not repeatedly blend its joins.

Settings > Pen and tablet offers size-pressure control and Linear, Soft or Firm
response in desktop and browser. Apply persists the choice; Cancel leaves the
active policy unchanged. Old preferences retain the default linear size response,
and malformed explicit policies are rejected. The Q16 curves are deterministic;
disabling size pressure uses the selected diameter while preserving the original
pressure and axes in the retained samples. Each stroke captures its policy at
Down, so changing preferences cannot rewrite an existing preview or history item.
These controls do not activate the unfinished native capture integration.

Preview and release can share immutable source samples and resolved mask storage.
The existing editor preparation and guarded publication path produces one history
item with explicit Brush, Pencil or Erase intent. Tests compare prepared preview,
publication, undo and redo pixels; cancellation, a foreign session and stale
preparation preserve the current document and history. Both desktop and browser
editor controllers now use this handoff in headless controls. That establishes
controller behavior; it does not establish production OS/browser acquisition.

Three mask policies remain distinct:

| Policy | Effect |
| --- | --- |
| Source-over | Apply captured color alpha multiplied by coverage once. Existing omitted-mode commands keep this default. |
| Replace | Interpolate stored straight RGBA channels by coverage. Existing replacement behavior is preserved. |
| Destination-out | Multiply source alpha by coverage, then reduce destination alpha while preserving destination RGB, including hidden RGB at alpha zero. |

Studio transparent pen erasing must use destination-out. Classic canvas-color
pen erasing must retain its separate replacement policy and Erase history intent.
Controller controls cover inverted-end erasing with the corresponding Classic
and Studio policy. Production acquisition and physical eraser behavior remain
unqualified.

## Frontend integration status

The desktop `with_native_tablet` opt-in connects AppKit capture, the bounded
receiver, existing editing service and guarded publication. The AppKit adapter
retains actual pressure/tilt/rotation availability, device/proximity identity,
barrel buttons and native view/backing coordinates. Its delivery gate records
callback failure before the host can drain a partial enqueue. Window focus and
close cancel contacts; installation owns monitor removal and native lifetimes.
See the [native boundary](../crates/adapters/paintcraft-tablet-native/README.md).

This host remains disabled at normal startup. Mixing genuine mouse and pen
records before a GUI frame still needs one proven acquisition order through the
widget/tool consumer. Per-source sequence numbers or frame event positions cannot
prove that order. The supported integration path is an owned
`winit + egui-winit + egui-wgpu` event loop when eframe cannot expose the required
boundary. Collector and prototype checks alone do not complete that integration.

The browser editor controller has tests for changing-pressure pixels, identical
preview/release output, one history item, undo/redo, cancellation, stale owners,
workspace identity and inverted erasing. Plain-round pressure eligibility does
not replace the existing nine brush-media policies: fallback controls compare
those outputs against the ordinary pointer renderer.

Browser capture retains pen identity and coalesced samples, owns pointer capture,
and orders genuine mouse and pen records at acquisition. It suppresses pen's
compatibility mouse events without matching GUI events by position or count.
The GUI hook classifies input; canvas replay uses the existing editor controller
and checks captured document, view and permission ownership before publication.
Focus loss, capture loss, resize, Escape and IME retire affected private work.
Scoped abort signals remove capture listeners before public runner teardown.

### Local browser qualification, 2026-10-27

An immutable production bundle was exercised in headless Chrome 156 on an
Apple-silicon Mac, through trusted CDP-emulated pen, mouse and touch delivery.
Classic 11 and Studio checks covered:

- First contact with focus on the page body or another ordinary input;
  varying-pressure and constant-pressure strokes produced different saved pixels.
- Coalesced input, identical held-preview/release screenshot regions, and exact
  saved baseline pixels after one Undo.
- Capture loss, Escape, resize and trusted window blur, followed by a fresh stroke
  and exact Undo restoration in both interfaces.
- Competing mouse/pen contacts in both ownership orders, held-button tails,
  secondary-button capture across barrel release, and palm tails after pen Up.
  Independent PCND reconstruction checked all 40 mixed-input saved projects.
- Two serial public destroy/start cycles, listener counts returning to zero and
  then their original baseline, and fresh trusted strokes after each restart.
- A mobile-sized Studio MSP2 preview, consent and download regression: whole
  encoded bytes matched the qualified golden, before/after project bytes matched,
  and the result lease returned to zero after download.

The original teardown run found capture listeners left registered. Its evidence
is retained separately from the corrected bundle's passing restart and pressure
regressions. These runs qualify the recorded production snapshot and its owned
input source files; they do not establish a full current-main release build.

CDP did not deliver its requested inverted-eraser bit as an eraser event, so real
browser eraser-end delivery remains unqualified. Host/controller eraser controls
are separate evidence. Web pressure can also be an API fallback value; these
tests do not demonstrate a physical pressure sensor. Other browsers, physical
devices, free/drop without explicit destroy, concurrent async starts/destroy,
panic recovery and sustained input/display timing still require qualification.

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
[retained pen strokes](../crates/editing/paintcraft-tools/tests/pen_stroke.rs),
[editor handoff](../apps/paintcraft/tests/pen_mask_handoff.rs),
[desktop host](../apps/paintcraft/src/app/tablet/tests.rs), and
[browser controller](../apps/paintcraft-web/src/paintcraft-web/editor/pen/tests.rs), and
[browser input host](../apps/paintcraft-web/src/paintcraft-web/pen_input/tests.rs).
These controls exercise synthetic samples in headless tests. The actual browser
delivery scope above and physical native traces remain distinct evidence.

## Remaining integration and qualification

| Area | Remaining work |
| --- | --- |
| Native capture | Complete ordered AppKit source-to-widget/tool integration; implement Windows pointer/history, XInput2 and Wayland tablet-v2 acquisition. Qualify safe installation/removal, IME, display conversion and cancellation through the actual frontend. The committed AppKit boundary and opt-in host are foundations. |
| Browser capture | Cross-browser and physical-device qualification, real eraser-end delivery, concurrent startup/drop/panic lifecycle cases, and sustained acquisition/display timing. Preserve the qualified mouse and palm behavior while extending finger navigation. |
| Controller adoption | Complete native production source-to-controller wiring and mixed-device ordering. Retain browser/native ownership/IME/draft arbitration, pressure-changing pixels, preview/release equality, one undo item and eraser-end/button semantics. |
| Brush settings and dynamics | Extend the captured width/curve preferences to appropriate opacity/media dynamics, capability feedback and applicable tilt/twist behavior. Recording optional axes does not implement their tool dynamics. |
| Other brush/media profiles | Preserve all existing nine media policies and sampled tips. The plain-round pressure path must not silently replace them; their pressure/dynamic integration remains required. |
| Physical qualification | Real tablet/driver traces on each platform, high-rate acquisition, disconnect/focus/modal recovery and input/display timing. Synthetic pen events and mouse injection do not establish these results. |

The workspace implementation specification remains authoritative: architecture
`NATIVE_INPUT_INTEGRATION.md` and `INPUT_BRIDGE.md`, and features
`BRUSH_DETAILS.md`, under `docs/temp/paintcraft-implementation-spec-v5-hyperfine`.
Comprehensive tablet acceptance is not complete.
