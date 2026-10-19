# Implementation notes

What has been adopted from the PaintCraft specification pack (kept outside version
control under `docs/temp/`), where this implementation deliberately differs, and what
is still provisional. "Verified" means covered by tests in this repository; it never
means native-hardware, GPU, platform-compatibility or original-application evidence.

## Implemented

| Area | What | Evidence |
|---|---|---|
| Engine (T01, T04–T07) | Sparse RGBA8 layers (256×256 copy-on-write tiles, exact default pixel, hidden RGB preserved), immutable documents, deterministic UUIDv5 identifiers, atomic staged batches, semantic no-ops, linear recorded undo/redo | unit tests; 21 conformance vectors from the Python oracle |
| Native format (T08, T29) | Canonical PCND v1 writer (byte-identical to the specimen) and bounded reader; unknown version, schema, feature, encoding or group is `capability.unsupported`, never editable | `pcnd.rs` tests, vector digests |
| Batch v2 | v1 plus `raster.stroke_path`, `raster.flood_fill`, `raster.shape`, `raster.text`, `layer.reorder` (see below) | behavior tests, JSON-Schema grammar cases |
| Image formats | PNG, BMP, JPEG import and export with named, acknowledged losses (flatten, hidden RGB, lossy JPEG, discarded alpha) | `image.rs`, `export.rs`, CLI tests |
| Safe save and recovery (T10) | Stage → atomic replace → read-back; a file-operation receipt; autosave checkpoints with per-session locks; recover, discard or ask later; a clean save or exit leaves nothing behind | `save.rs`, `recovery.rs`, GUI tests |
| Bounded history (T11) | Memory budget over all retained snapshots (shared tiles counted once); only the oldest undo steps are dropped, the newest edit stays undoable, the user is told | `session.rs`, editor tests |
| CLI (T08, T33, T34) | `new`, `inspect`, `schema batch|batch-v2`, `capabilities`, `apply` (hash-guarded), `import`, `render` | 11 end-to-end tests of the real binary; output validates against the pack's schemas |
| Editor (T09, T14 subset) | Pencil, brush (round/square), eraser, flood fill, line, rectangle, ellipse (outline/filled/both), rectangle select, bitmap text; layers: add, show/hide, move, delete, rename, opacity; undo/redo; one gesture or text object is one transaction | `paintcraft-ui` tests; 28 headless egui tests |
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
changes while running, GPU output correctness, window or compositor behavior on platforms
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
- **Classic variants.** v7, v10 and v11 are presentation presets (rounding, panel colour,
  accent), not captured reproductions. No fidelity claim of any kind is made.
- **Checkpoints on the UI thread.** Autosave writes on the UI thread when idle; fine at
  these sizes, to be moved to a background worker with large documents.
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
source-over (BF02), a full-document re-render and texture upload after every publication
(BF29), `eframe` as the window shell (BF22). Do not describe any of these as measured or
optimal.

## Not started

Native input and presentation spike (T02), reference capture (T03) and therefore any
Paint/Paint.NET behavioral parity, selections beyond one rectangle (no move, transform,
invert or free-form), scalable or system fonts, clipboard, zoom to fit, layer groups,
layer blend modes, other formats (GIF, TIFF, WebP, PCX, `.pdn`, `.paint`), Wasm effects,
Zstd blocks, a background save worker, `paintcraft-web`, and everything in milestones M3
onward.
