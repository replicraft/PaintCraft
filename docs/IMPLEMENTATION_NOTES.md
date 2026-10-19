# Implementation notes

What has been adopted from the PaintCraft specification pack (kept outside version
control under `docs/temp/`), where this implementation deliberately differs, and what
is still provisional. "Verified" means covered by tests in this repository; it never
means native-hardware, GPU, platform-compatibility or original-application evidence.

## Workspace boundaries

`paintcraft-core` is the shared foundation, exposing `paintcraft_core::types`; it has
no document, raster or session dependencies. The former session/execution core is
`paintcraft-editor`. Raster storage, document semantics, command parsing, history,
rendering, normalized input and tool state machines have separate crates. The desktop
shell owns the editor controller and settings. Filesystem save adapters belong to
`paintcraft-platform`; formats retain a save API re-export for their callers.

Version 0.1.0 and dependency declarations are inherited from the root workspace. The
desktop is the default member. The CLI uses Clap and separate command modules.

## Standard image codec adapter

PNG, BMP and JPEG use `image` with only their format features enabled. The direct
zune/jpeg encoder dependencies and custom BMP pixel codec have been removed. `png`
remains a developer dependency for independent fixture generation. Zerocopy provides
a 54-byte little-endian BMP header view; dimensions, offsets, declared palettes and
supported masks still require semantic admission before `image` decodes pixels.

Tests preserve hidden RGB/alpha and the existing high-byte 16-to-8 conversion policy.
JPEG output explicitly composites alpha over white. ICC/EXIF loss is reported, and
CLI import loss codes can be acknowledged separately from export loss codes. These
are capability/correctness choices; BF03/BF25 comparative performance remains pending.

## Streaming native encoder

PCND `write_to` emits canonical stored-ZIP bytes through a backpressured `Write`
sink. Pixel scratch retains one 4 MiB chunk and one raster block; document metadata,
sorted block coordinates and the central directory are accounted separately. Directory
scratch has an 8 MiB provisional cap. This remains the existing non-ZIP64 v1 subset.
The in-memory convenience writer uses the same encoder. Partial-write, failing-sink,
round-trip and conformance tests preserve canonical bytes. SAVE-09 is partial: process-wide memory admission and measured native backpressure remain pending.

The desktop's explicit native save and CLI new/apply/import now stream directly to
same-filesystem staging files. A byte-quota sink hashes accepted writes; bounded
64 KiB reads verify staged and replaced contents without loading a second archive.
Encoder errors, ignored failed writes, staging corruption and pre-replace barriers
cannot publish partial files. Receipt IDs are stable, and coordinators can stamp a
save sequence and binding generation. Injected phase failures distinguish known
replacement from confirmed durability. Temporary-link cleanup follows the successful
create boundary. Ambiguous replace errors retain staging and prohibit automatic retry.
Ordering is supplied by the native coordinator below. Existing symlinks resolve to their
captured target; new paths resolve their parent aliases. Dangling symlinks are rejected.
Atomic replacement uses the selected namespace, so replacing a hardlink does not rewrite
other hardlinks. Receipts retain the destination identity captured before encoding.
Content fingerprints before and after encoding detect external changes during encoding;
a changed destination is left untouched by this attempt. This is a best-effort check,
with a race between verification and replacement. Open-to-save/queued-request baseline
fingerprints, cooperative locking and Windows/network qualification remain pending.
Alias and concurrent-change tests ran on the local macOS filesystem (FILE-07 partial).

## GPU compositor and native dialogs

The desktop shares eframe's wgpu device with an ordered compute compositor. WGSL uses
the scalar policy's exact integer source-over and layer-opacity rounding. A required-GPU
test passed on the development machine's Apple M1 Pro Metal adapter: layer order,
visibility, opacity, negative/off-canvas offsets, tile boundaries, workgroup tails and
transparent source samples produced byte-identical results to the CPU compositor.
Other adapters/backends are not qualified by that result.

The initial compositor materializes clipped layer samples, dispatches layers in order,
then reads the composite back for the existing egui texture path. Device waits occur on an admitted native worker;
there is no performance or zero-copy claim. Device buffer/dispatch limits are checked
before allocating, and unsupported documents fall back to CPU rendering. GPU and dialog
features remain optional for headless consumers.

Open, Save As and Export use parented native `rfd` dialogs. Their completion is polled
by the application, and cancellation returns without file operations. Save reuses the
bound native document path. Save As updates the binding only after the save is confirmed;
a failed save preserves the previous binding and dirty state. Automated tests cover both
binding outcomes. Interactive native-dialog behavior still needs manual checks on each OS.
A short native desktop run with GPU/native services initialized and temporary settings
opened and exited cleanly on macOS.

## Recovery journal

[Recovery store v1](RECOVERY_FORMAT.md) owns the private on-disk encoding and limits.
Immutable full snapshots become durable before checksummed commit segments; two
alternating heads retain a previous valid generation. Recovery replays only a backed,
contiguous tail. Native parser admission and snapshot identities are verified before
an orphan is offered. Valid legacy checkpoints are retained as an input format.

Protocol interruption, corrupt payload, torn/missing tail, live-lock protection,
interrupted discard and GC tests pass on the local macOS filesystem (SAVE-07/08).
Durable discard markers prevent cleanup interruptions from reviving old data. GC
failure does not negate an already durable checkpoint. Disk/record limits are
provisional; power-loss, platform, process-wide admission and performance qualification
remain pending. The desktop streams typed snapshots into this store, reports scan/cleanup failures,
and shows the last confirmed checkpoint generation/time and freshness in the status
tooltip. Recovery handoff durably protects the restored document before discarding its
original orphan; a failed handoff keeps the old copy. Tests cover an immediate second
crash and an unavailable new store.

Native periodic checkpoints and recovery cleanup share the admitted save FIFO. Jobs
hold immutable snapshots and owner-scoped publication tokens. Atomic invalidation does
not wait for disk work; cleanup precedes the next episode. Stale callbacks cannot
reinstate cleared checkpoint status. One checkpoint per current owner is admitted at
a time. Queue exhaustion refuses document replacement before changing its recovery
fence or editor. Tests cover queued work, edits during checkpoints, save/cleanup races,
and clean close after undo. Native startup now submits recovery discovery and owner
initialization after the window/services are created. Results are checked against the
current editor and startup configuration; unused owners are retired on the I/O queue.
An editor replacement during exclusive discovery retries initialization after capacity
is available. Startup configuration can supersede pending requests; changing an already
active store is explicitly refused until durable migration is implemented. Recovery
requires discard confirmation before replacing current unsaved work.

Native restore reads and first protective checkpoints now use the same I/O service.
A claim verifies the offered input digest/IDs and holds its orphan lock through
preparation and handoff. One bounded restore intent waits for capacity without another
click. Publication checks the session, StateId/epoch, binding generation, configuration,
tool drafts and layer edit buffers. Superseded data is disposed on the worker. A failed
new recovery store retains the original claim; a confirmed checkpoint or explicit
native save can finish the handoff. Exports do not release this native protection.
Tests cover deferred publication, cancellation, ABA, deleted/changed sources, newer
Save As choices and unavailable storage. Explicit orphan discard is also queued.

A session records the acknowledged saved state as an optional UUID. Absence is distinct
from every admitted StateId, including the all-zero UUID. A regression test restores
such a snapshot through the native I/O lifecycle and verifies that it remains dirty,
receives its own checkpoint, and retires the old source only after protection.

## Ordered native saves

The desktop enables Tokio 1.53.3 (`rt`, `sync`) only at the native I/O consumer.
One current-thread coordinator admits at most four jobs and one blocking I/O worker;
a global FIFO serializes accepted saves, periodic checkpoints and recovery cleanup,
including save path aliases. Other destinations
also wait in this initial baseline. A provisional 1 GiB charge covers independently
retained snapshot payloads and encoder estimates; it is not a measured RSS bound.
Completions remain charged until consumed. Accepted user saves drain on shutdown;
superseded recovery work is explicitly fenced.

The app captures immutable document state and adopts confirmed completion only for the
current session and requested binding generation. Saving A while editing B records A;
B remains dirty and undoing to A becomes clean (SAVE-04). Stale Save As completions
cannot retarget a later choice (SAVE-02/FILE-04). Same-destination FIFO tests cover
SAVE-01. Repeated layout passes cannot submit duplicate saves. Close requests drain
accepted writes, and failures keep the window open for a new explicit decision.
The deterministic headless UI harness retains its synchronous adapter; native queue
integration has separate tests. Open-to-save conflict fingerprints,
restart-persistent receipt reconciliation and BF07 performance qualification remain pending.

The platform save outcome now retains private attempt evidence for bounded local-Unix
reinspection (FILE-02/03). `Outcome::reconcile` revises the same operation ID without
encoding or replacing again. Inspection distinguishes staged/original/third-party,
missing and unreadable content. Matching hashes alone never acknowledge a syscall
without positive replacement evidence. Explicit durability confirmation syncs the
opened file and parent, then checks the descriptor, namespace, content and revision;
changes across those barriers remain unconfirmed. Receipt source/sequence/binding
stamps remain caller-owned. Failure injection covers both barriers, late replacement,
late edits/deletion and alias changes. This is not a portable exclusion mechanism or
filesystem qualification; other platform identity adapters and restart-persistent
uncertain-attempt retention remain pending.

The native desktop retains up to 16 incomplete save attempts, including known
pre-replacement failures and worker failures, with the exact immutable document each
request captured. Their conservative snapshot charges share the 1 GiB I/O admission
budget; save admission leaves 1 MiB available for control work. This is an estimate,
not allocator/RSS qualification. A later successful save does not hide earlier failed
attempts or release their snapshots. The Incomplete saves panel queues inspection and
explicit persistence confirmation on the same FIFO. Revised receipts retain the
original operation ID, save sequence and binding generation. Only the current session
and binding generation may adopt a confirmed result, and an older save sequence cannot
regress a newer acknowledged state or replace the latest receipt. Adopted paths use
the originally resolved namespace rather than re-resolving a potentially changed alias.

Captured snapshots can be opened as unbound unsaved work, with confirmation before
replacing newer edits or drafts, even after the original file is removed. Explicitly
forgetting an attempt requires confirmation and does not change the destination or
claim durability. Unresolved attempts keep close warnings active; explicitly quitting
warns that these in-memory snapshots will be abandoned. This tranche does not yet
persist attempt details/snapshots across restart. Native coordinator and real egui
multipass widget tests cover these controls; target/filesystem durability qualification
and external-process conflict handling remain separate obligations.

## Native render preparation

The native shell submits immutable document snapshots to one dedicated Rayon worker
through `paintcraft-runtime`. Its queue is bounded to two admitted jobs and a provisional
1 GiB estimate covering retained tile payloads and transient render buffers. Completed
and cancelled results remain charged until consumed. Slotmap keys are owner-scoped by a
separate UUID; neither transient keys nor generations are persisted as document IDs.

Only the latest requested job/revision may update the displayed canvas. Cancellation,
new document installation, edits and undo cannot publish obsolete prepared images.
The CPU renderer checks cancellation at row boundaries; GPU preparation checks between
completed layer submissions, bounding simultaneously retained source buffers. Shader
work already submitted to the GPU is not preempted. Workers do not mutate editor state.
Panics become failed preparations; stale callbacks/results are discarded. Tests cover
reservation lifetime, slot reuse/ownership, cancellation and late render completion.

This is an admitted render-worker baseline, not the complete T13 priority scheduler or
a physical-memory/performance qualification. Background recovery, process-wide service
coordination, GPU device epochs and measured fairness remain pending.

## Classic presentation

`paintcraft-classic` owns the version descriptor, styles and presentation-only controls.
The desktop feeds it a snapshot of tool options and colors; its actions pass through a
tokenized outbox before reaching editor/platform services. Repeated layout passes cannot
repeat New/Open/Save or reset a newly started tool gesture. Shared vector icons/widgets
live in `paintcraft-ui`; no proprietary artwork is bundled.

The initial Classic layout has File/Edit/View menus and quick actions, grouped selection,
tools and shapes, primary/secondary colors, a 28-swatch palette, right-side layers, framed
canvas workspace and status-bar pixel dimensions/zoom. v7/v10 show the Home ribbon family;
v11 uses the modern palette styling. These are working presentation baselines, not pinned
reference reproductions. KeyTips, historical target/eraser rules and reference geometry
remain T16 work. Current-app GPU previews can be exported with the ignored
`export_classic_ui_previews` GUI test; they are not reference capture evidence.

## Implemented

| Area | What | Evidence |
|---|---|---|
| Engine (T01, T04–T07) | Sparse RGBA8 layers (256×256 copy-on-write tiles, exact default pixel, hidden RGB preserved), immutable documents, deterministic UUIDv5 identifiers, atomic staged batches, semantic no-ops, linear recorded undo/redo | unit tests; 21 conformance vectors from the Python oracle |
| Native format (T08, T29) | Canonical PCND v1 writer (byte-identical to the specimen) and bounded reader; unknown version, schema, feature, encoding or group is `capability.unsupported`, never editable | `pcnd.rs` tests, vector digests |
| Batch v2 | v1 plus `raster.stroke_path`, `raster.flood_fill`, `raster.shape`, `raster.text`, `layer.reorder` (see below) | behavior tests, JSON-Schema grammar cases |
| Image formats | PNG, BMP, JPEG import and export with named, acknowledged losses (flatten, hidden RGB, lossy JPEG, discarded alpha) | `image.rs`, `export.rs`, CLI tests |
| Safe save and recovery (T10) | Streaming staging → atomic replace → bounded read-back; ordered native queue and file receipts; autosave checkpoints with per-session locks; recover, discard or ask later | platform `save.rs`, format `recovery.rs`, GUI tests |
| Bounded history (T11) | Memory budget over all retained snapshots (shared tiles counted once); only the oldest undo steps are dropped, the newest edit stays undoable, the user is told | `session.rs`, editor tests |
| CLI (T08, T33, T34) | `new`, `inspect`, `schema batch|batch-v2`, `capabilities`, `apply` (hash-guarded), `import`, `render` | 11 end-to-end tests of the real binary; output validates against the pack's schemas |
| Editor (T09, T14 subset) | Pencil, brush (round/square), eraser, flood fill, line, rectangle, ellipse (outline/filled/both), rectangle select, bitmap text; layers: add, show/hide, move, delete, rename, opacity; undo/redo; one gesture or text object is one transaction | desktop controller tests; headless egui tests |
| Presentation (T15 subset) | First-visit Classic picker (v11 preselected), remembered choice, Interface button, damaged settings kept aside | GUI tests |

Native evidence is limited to repeated runs of the real wgpu window, which opened, drew
frames and exited cleanly.

## What the GUI evidence does and does not show

The `egui_kittest` tests inject real egui input events through the real layout code:
one click, drag or typed text object publishes exactly one transaction however the events
are batched across frames, a frame laid out twice repeats nothing, focus loss and Escape
behave as specified, shortcuts use each key event's own modifiers, the canvas maps one
document pixel to one physical pixel at 1x and 2x display scale, and the displayed
composite equals the engine's render byte for byte. The harness' accessibility check found
unlabeled and ambiguously labelled widgets, now fixed.

Not shown: real mouse, pen or touch hardware, timestamps and coalesced samples, IME, DPI
changes while running, final GPU presentation correctness, window or compositor behavior on platforms
other than the macOS machine used. The native input spike (T02) has not been done:
`eframe` is used by default and is provisional.

## Batch v2 is an extension of the pinned contract

The pack pins batch v1 and its tool set. The editor needs more operations, so v2 adds them
as a **superset**: v1 batches parse and behave unchanged, v1 rejects the new operations,
and nothing in the pinned v1 schemas changed. This is a deliberate extension that the
specification pack does not yet describe; it should be adopted there before anything is
called a stable API.

Evidence is weaker than for v1. v1 has an independent Python oracle and 21 vectors; **v2
has no independent oracle**. What exists: the published JSON Schema (`schema batch-v2`)
cross-checked against the parser on independent validator verdicts
(`conformance/grammar-v2.cases`), and Rust tests whose expected pictures and bytes were
derived by hand from the geometry definitions. Rust-generated golden vectors for v2 would
only prove stability, not correctness.

Definitions (integer-exact, so any implementation can reproduce them):

- **Brush stamps.** Diameter `n`; square is the full `n×n` box, round is the `n×n` ellipse.
  The stamp's top-left is `centre − (n−1)/2`. A stroke's footprint is the union of stamps
  along the pencil path, and every covered pixel blends exactly once.
- **Ellipse.** Pixel `(px,py)` in a `w×h` box is inside when
  `(2px+1−w)²h² + (2py+1−h)²w² ≤ w²h²`. An outline of thickness `t` is the figure minus the
  same figure shrunk by `t` on every side; if nothing remains inside, it is solid.
- **Flood fill.** 4-connected from the seed; a pixel joins when each channel differs from
  the seed pixel by at most the tolerance; the region is computed before painting.
- **Text.** 8×8 bitmap font (`font8x8`), integer scale 1–16, no anti-aliasing. Characters
  without a glyph fail the batch naming the code point; the editor skips and reports them.
  This is a starting point, not a claim of fidelity to any application's text tool.
- **Budgets** (provisional): stroke work (path pixels × brush diameter) 16,777,216 rows per
  batch, flood area 67,108,864 pixels, text up to 1,024 characters.

## Deliberate differences and choices

- **Own JSON and ZIP code.** `serde_json` accepts duplicate keys and cannot give the
  required UTF-16 canonical key order. A small strict parser and a stored-only ZIP
  reader/writer are used so every accepted input and every output byte is controlled. The
  ZIP reader rejects ZIP64 (at most 65,534 members), compression, encryption and data
  descriptors.
- **Integers only.** JSON numbers with a fraction or exponent are `input.schema`, even
  when integral (`1.0`). The Python oracle's validator accepts those.
- **UUID matching.** Layer ids in a batch are compared as UUID values (upper case matches).
- **Admission limits** (provisional): 268,435,456 staged pixels per batch, a 64M-pixel
  render, a 268M-pixel decode budget, a 1 GiB default history budget. None is measured.
- **Loss codes.** `flatten`, `hidden-rgb`, `lossy-jpeg`, `alpha-discarded`. JPEG export
  composites over white. EXIF orientation and colour profiles are not interpreted on JPEG
  import; PNG ICC profiles and 16-bit depth are reported and not kept.
- **`inspect` output.** `paintcraft.inspect/v1` is this implementation's shape; the pack
  names its contents but no schema.
- **Identifiers.** Only the deterministic allocator exists; identical `new` parameters give
  identical DocumentIds. Recovery sessions use random ids and do not rely on DocumentId.
- **Eraser.** It clears to transparent (replace with `#00000000`), also on the opaque
  Background layer. Classic Paint erases to the background colour; that is not reproduced.
- **Classic variants.** v7, v10 and v11 share initial ribbon/control primitives and version styling;
  they are not captured reproductions. No fidelity claim of any kind is made.
- **Foreground file work.** Native periodic checkpoints and cleanup use the I/O worker.
  Open/import/export still have foreground paths. Native recovery discovery,
  initialization, restore, handoff and cleanup use the admitted I/O queue. The deterministic harness uses synchronous checkpoint services.
- **Windows.** Durability confirmation syncs the destination directory, which Windows does
  not allow by opening a directory, so a save there would report "not confirmed" (exit 6)
  until a platform adapter exists. CI covers Linux and macOS only.

## Scale observation (not a benchmark)

One run on the development machine: on a 4096×4096 document, a batch with a large
ellipse, a thick translucent stroke, a flood fill and 16× text applied in about 0.45 s at
about 350 MB peak memory (the document is about 64 MB resident, and staging shares tiles by
copy-on-write); exporting that document to PNG took about 0.4 s. No repetition, no
controlled environment, no comparison. Real measurements follow the benchmark protocol.

## Provisional baselines (no benchmark has been run)

Tile edge 256, a `HashMap` tile index (BF16, BF17), whole-snapshot history sharing tiles by
`Arc` and counted by unique tiles on every commit (BF18, T11), a scalar per-pixel
source-over (BF02), a full-document composite and texture upload after every publication
(BF29), `eframe` as the window shell (BF22). Do not describe any of these as measured or
optimal.

## Not started

Native input and presentation spike (T02), reference capture (T03) and therefore any
Paint/Paint.NET behavioral parity, selections beyond one rectangle (no move, transform,
invert or free-form), scalable or system fonts, clipboard, zoom to fit, layer groups,
layer blend modes, other formats (GIF, TIFF, WebP, PCX, `.pdn`, `.paint`), Wasm effects,
Zstd blocks, `paintcraft-web`, and everything in milestones M3
onward.
