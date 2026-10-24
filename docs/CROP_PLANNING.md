# Destructive canvas Crop

`canvas.crop` is an additive `paintcraft.batch/v2` operation with one `rect`
(`x`, `y`, `width`, `height`). The rectangle must be wholly inside the current
canvas; dimensions follow the existing numeric policy. V1 rejects this operation.
The shared host helper clips a rectangular selection to the canvas before issuing
it. Empty/inactive selections keep the action disabled.

Crop discards content outside that rectangle from every layer, including hidden
layers and off-canvas content. It translates retained content to the new canvas
origin, preserves layer IDs, ordering, names, visibility and opacity, and clears
the selection. Fully retained layer rasters remain shared. Partially retained
rasters preserve straight RGBA, including RGB under zero alpha, and their default
pixel. A wholly excluded layer retains its identity with an empty transparent
1 × 1 raster at the origin. Expansion cannot recover discarded samples. One
recorded transaction restores the exact document and selection on Undo/Redo.
Cropping an already bounded whole canvas without a selection is a semantic no-op.

## Preparation and admission

Crop uses the normal complete-batch compiler. Preparation pins the logical source
at that operation, including earlier paint, duplication, floods and crops in the
same batch. It scans only candidate non-default source tiles to construct a
metadata-charged map of destination tile coordinates. It creates no decoded pixel
buffers or per-pixel constant-color command descriptors. A huge uniform raster
can therefore be cropped without walking its implicit background pixels.

Mapped logical snapshots make the cropped result available to later operations
before replay. The residency simulator accounts for each fresh destination tile
and the full private batch peak. The engine reserves that peak before allocating
any private decoded tiles. Replay reads the pinned source and copies exact RGBA
into the admitted destination cells; no sequential fallback bypasses compilation.
Cancellation or denial discards private state without publishing history.

A provisional ceiling allows 16,777,216 source sample reads per batch across all
cropped layers; dense larger crops report `ResourceLimit`. Replay may read more
samples because destination tiles cross source tile boundaries. Metadata and
lineage retain their existing bounded accounts. These are logical admission
limits, not measured physical RSS or latency guarantees. Large dense transforms,
rotate/flip/skew and broader document structures remain separate work.

## Host behavior and validation

Classic's ribbon and Edit menu, desktop tool actions, web and CLI use the same
operation. Desktop processing runs through the existing editing worker. Classic
and web remember the dimensions after an authored crop/resize actually publishes;
Undo/Redo preserve that last-authored startup preference. This also covers the
native asynchronous path rather than relying on a dialog's immediate return.

Source checks:

- [Raster plans](../crates/foundation/paintcraft-raster/tests/crop_plans.rs):
  unaligned tile boundaries, raw hidden RGB, nested crops and expansion padding,
  huge uniform sources, cancellation and allocation denial.
- [Engine](../crates/editing/paintcraft-editor/tests/crop.rs): independent
  world-coordinate sample oracle, mixed origins/hidden layers, ordered mixed
  batches, exact history, no-op and pre-allocation peak denial.
- [Native GUI](../apps/paintcraft/tests/gui/crop.rs),
  [native worker](../apps/paintcraft/src/app/editing/tests.rs) and
  [web](../apps/paintcraft-web/src/crop_tests.rs): real selection/action routing,
  publication, raw samples, Undo and authored-size preferences.
- [CLI](../apps/paintcraft-cli/tests/cli/crop.rs): batch, project round trip and
  PNG output with explicit hidden-RGB-loss acknowledgement.
- [Grammar cases](../conformance/grammar-v2.cases): independently checked JSON
  Schema verdicts, compared against the Rust parser.

On 2026-10-24 an isolated headless Brave instance drew a stroke, selected a
121 × 81 region, cropped it, downloaded PCND and undid the crop. The CLI reopened
the download at exactly 121 × 81; Undo restored the sampled original pixels
byte-for-byte, with no JavaScript exceptions. This is scoped browser behavior
validation, not Windows pixel fidelity, device qualification or the T22 matrix.
Generated captures and logs remain in ignored validation storage.

Qualification includes 257 passing native library tests (8 opt-in tests ignored),
216 passing GUI tests (45 opt-in tests ignored), 47 web tests, the four new raster
plan cases, six engine cases and CLI/grammar checks. All 77 grammar fixtures agree
with an independent JSON Schema validator and the Rust parser. Strict workspace
all-target Clippy, wasm Clippy, formatting, source-length checks and the final
release web bundle pass. Ignored GPU/device tests were not counted as passing.
