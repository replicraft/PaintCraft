# Tablet input status

Tablet support is being implemented across native and browser frontends. Shared
input, pressure rendering and guarded editor controllers are implemented and
tested. Browser startup now installs dedicated Pointer Events capture. Desktop
has an experimental owned AppKit event loop; normal desktop startup does not
activate it. Canonical keyboard/IME ordering and physical input qualification
remain in progress. Physical tablets, drivers and platform timing have not been
qualified. A pen reaching the
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

The default plain-round pressure policy rounds normalized pressure to Q16 and
uses it to scale the captured diameter. Missing pressure uses the full diameter;
zero contributes no endpoint disk. The geometry is the actual swept round tip
with linearly changing radius, using common tangents or disk containment. A 4×4
coverage grid unions the whole stroke before applying color opacity, so retracing
a translucent stroke does not repeatedly blend its joins.

Settings > Pen and tablet offers pressure-controlled size and opacity, Linear,
Soft or Firm response, optional textured-brush dynamics, and default, tilt or
twist nib direction in desktop and browser. Apply persists the choices; Cancel
leaves the active policy unchanged. Old preferences retain linear size response
with opacity and textured dynamics disabled. Malformed explicit policies are
rejected. The Q16 curves are deterministic; disabling size pressure uses the
selected diameter while preserving original pressure and axes in retained samples.
Each stroke captures its policy at Down, so changing preferences cannot rewrite
an existing preview or history item. These controls do not activate the unfinished
native capture integration.

Optional pressure opacity uses capped dabs: overlapping parts of one stroke do
not repeatedly build up opacity. Enabled media dynamics preserve each existing
brush's tip, texture and paint-load behavior. Tilt or twist can orient calligraphy
and marker tips; absent or ambiguous axes use their historical direction.
Sampled tips retain their existing behavior. Headless raster, tool and browser
editor tests cover media pressure, preview/release equality and one-step Undo;
physical brush feel and the newly expanded settings in production browser input
still need qualification.

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
The [Linux acquisition investigation](LINUX_TABLET_INTEGRATION.md) records the
locked dependency options and the remaining queue-ordering requirements.

The optional ordered App coordinator now projects native records into admitted
pointer frames and processes one semantic edge or service poll per step. An
accepted action retains its exact source scope through the outbox, FIFO, worker
admission and retry. Actual authored receipts and history-navigation results stay
distinct. Neither a changed document ticket nor an unrelated completion can
authorize later captured input.

Prepared publication checks both revocable Source ownership and a sealed native
installation authority. Its final metadata authorization defines the ordering
point; editor mutation follows outside delivery/ingress locks. Retirement removes
only the matching queued or prepared work, retaining unrelated edits and worker
resource leases until retirement. Headless App/service controls pause real
preparation and completed results to check failure, focus, modal and IME loss,
invalid framework input, exact Undo ordering and recovery. They also cover
unchanged legacy paint queues and floating workflows.

Normal startup keeps native capture disabled. On macOS, the experimental owned
`winit + egui-winit + egui-wgpu` path can be started explicitly:

```sh
PAINTCRAFT_NATIVE_TABLET=1 cargo run -p paintcraft
```

It uses the existing document, dialog, recovery and GPU services. Native failure
retires the capture installation and returns to ordinary mouse input while
preserving the document and saving. `PAINTCRAFT_NATIVE_TRACE=1` adds input-origin
and delivery-health diagnostics without logging text or document pixels.

An actual local macOS window opened, rendered and closed through this path.
Its pre-close snapshot reported native delivery `Ready`, capture installed and
zero callback, sink, source or wake failures. An earlier failing run exposed
out-of-order AppKit tracking timestamps; the corrected collector preserves those
observations without advancing or rebasing the physical mouse clock. The failed
traces are retained separately. This smoke test used no physical pen input or
keyboard/IME injection. The executable and owned startup sources were recorded;
it was not a frozen whole-workspace release qualification.

Canonical keyboard/IME order still requires qualification before default native
activation. Per-source sequence numbers or frame event positions cannot prove
that order. Windows pointer/history acquisition has cross-target SDK checks;
Windows runtime and shared native touch adoption remain unfinished. Linux still
needs an ordered acquisition integration.

The [macOS text-input investigation](MACOS_TEXT_INPUT_INTEGRATION.md) records an
isolated public AppKit API probe and the remaining keyboard/IME ownership gates.

Native touch records now retain full device/pointer identity, actual confidence,
contact state and the system primary flag. The bounded collector tracks 32
contacts, preserves clocks across completed contacts, and reclaims them only on
verified lifetime retirement. Confidence withdrawal produces a separate policy
cancellation without changing the physical event phase. Historical transitions
cannot hide a lost contact, and internal boundaries cannot invent finger input.
The native App projection still explicitly refuses touch records: gesture and
widget delivery are not enabled by these metadata controls.

The browser editor controller has tests for changing-pressure pixels, identical
preview/release output, one history item, undo/redo, cancellation, stale owners,
workspace identity and inverted erasing. Default pressure eligibility does not
replace the existing nine brush-media policies: fallback controls compare those
outputs against the ordinary pointer renderer. Optional media-dynamics controls
also check each profile through preview, publication, Undo and Redo.

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

### Subsequent browser checks

The archived `d7678f16` bundle also passed actual GUI checks for all five pen
preferences: Cancel retained the original policy, Apply persisted the selected
values, and reopening settings showed those values. Classic and Studio each
recovered after a real pen release while the canvas layout had zero width.
Independent decoding of eight saved projects confirmed cancellation and exact
one-Undo restoration, followed by a working stroke from the same pen identity.

That bundle exposed a separate outside-canvas release failure after capture loss.
The corrected archived `e1b23b0b` bundle observes only trusted terminal metadata
for an already known pen. Both interfaces then accepted the next actual contact
from that same identity. Six saved projects independently confirmed changed
pixels and exact one-Undo restoration; cancellation was checked through screenshot
regions and separate host document controls, with no cancelled-project download
claimed. Product listeners on both the window and canvas retired through two
public destroy/start cycles and returned to their original counts. The harness's
own recorder and Playwright listeners were identified separately by script source.

The original product failure and the later failed listener-count assertion remain
separate evidence. These are scoped Chrome/CDP results on recorded bundles, not
physical hardware, cross-browser or full current-main release qualification.

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
[desktop host](../apps/paintcraft/src/app/tablet/tests.rs),
[ordered native App](../apps/paintcraft/src/app/owned_tablet/tests.rs),
[browser controller](../apps/paintcraft-web/src/paintcraft-web/editor/pen/tests.rs), and
[browser input host](../apps/paintcraft-web/src/paintcraft-web/pen_input/tests.rs).
These controls exercise synthetic samples in headless tests. The actual browser
delivery scope above and physical native traces remain distinct evidence.

## Remaining integration and qualification

| Area | Remaining work |
| --- | --- |
| Native capture | Complete canonical AppKit keyboard/IME ordering; integrate and qualify Windows pointer/history and native touch, and implement ordered XInput2 and Wayland tablet-v2 acquisition. Qualify safe installation/removal, IME, display conversion and cancellation through the actual frontend. The committed AppKit boundary and opt-in host are foundations. |
| Browser capture | Cross-browser and physical-device qualification, real eraser-end delivery, concurrent startup/drop/panic lifecycle cases, and sustained acquisition/display timing. Preserve the qualified mouse and palm behavior while extending finger navigation. |
| Controller adoption | Complete native production source-to-controller wiring and mixed-device ordering. Retain browser/native ownership/IME/draft arbitration, pressure-changing pixels, preview/release equality, one undo item and eraser-end/button semantics. |
| Brush settings and dynamics | Qualify the captured size, curve, opacity and textured-brush settings through production input; add device capability feedback and physical tilt/twist traces. Functional settings and headless pixel controls do not establish physical brush feel. |
| Other brush/media profiles | Extend appropriate pressure/dynamics behavior to sampled and other unsupported tips while preserving each media policy. Qualify high-rate and long-stroke behavior through actual acquisition and rendering. |
| Physical qualification | Real tablet/driver traces on each platform, high-rate acquisition, disconnect/focus/modal recovery and input/display timing. Synthetic pen events and mouse injection do not establish these results. |

The workspace implementation specification remains authoritative: architecture
`NATIVE_INPUT_INTEGRATION.md` and `INPUT_BRIDGE.md`, and features
`BRUSH_DETAILS.md`, under `docs/temp/paintcraft-implementation-spec-v5-hyperfine`.
Comprehensive tablet acceptance is not complete.
