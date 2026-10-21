# CPU compositor: borrowed sparse row spans

## Scope and invariants

The active T12/BF29 slice removes per-layer scratch-row copying from full CPU
composition. Authoritative storage remains sparse COW 256×256 tiles. Committed
document pixels, history, layer order and preview publication rules do not change.
The public full-render API still returns one owned dense RGBA8 canvas buffer;
this first slice did not introduce a regional renderer or cache. The subsequent
[regional compositor](REGIONAL_COMPOSITOR.md) adds those APIs and CPU fallback reuse.

`RasterLayer::row_spans(y, x, pixels)` validates the complete unsigned layer-local
range before returning an allocation-free iterator. Resident segments borrow
tightly packed immutable tile bytes. Missing tiles produce uniform descriptors
with the exact default RGBA, including hidden RGB. Partial edge tiles and empty
ranges are bounded explicitly. The borrow cannot outlive or mutate its source;
editing a cloned snapshot retains the existing COW allocation admission.

The compositor retains the transparent-black starting backdrop, finite canvas
clipping, bottom-to-top order and exact integer layer-opacity/source-over policy.
It skips invisible, zero-opacity and clipped layers as before. For a uniform run,
effective alpha is computed once; zero skips the run, 255 fills it, and other
alphas use the unchanged scalar kernel. Resident samples use the same effective
alpha dispatch, borrowing their source bytes rather than copying a temporary row.
Cancellation remains checked before allocation and on each intersecting layer row.
No partial output is returned after cancellation.

This row-span slice adds no expensive hashes, persistent cache or new resident representation.
Profile transforms, groups, masks and effects are absent from the current flat
document model; this path does not claim correctness for their future policies.
Display overlays remain separate, and staged brush/text previews are composed
from their private document snapshot. GPU selection/residency is unchanged.

## Correctness evidence

- Raster tests cover borrowed/implicit segments, hidden RGB, tile boundaries,
  short final tiles, invalid/overflowing ranges and mutation of a COW clone.
- `paintcraft-render/tests/row_spans.rs` checks every source alpha and opacity
  boundaries against independent per-pixel integer arithmetic, with visibility,
  clipping, offsets and resident pixels. It checks retained tile identities too.
- The v1 conformance corpus retains exact render and canonical PCND digests.
- `paintcraft-formats/tests/editor_workflow.rs` runs a complete layered edit,
  selection/fill, undo/redo, PCND write/reopen and independent PNG decoder check
  using hand-calculated output. This is distinct from isolated kernel tests.
- Existing cancellation/admission tests and GPU parity tests remain applicable.

## Measurement boundaries

The [benchmark workflow](../benchmarks/README.md) compares the frozen 95db5d0
row-copy renderer with production on identical generated fixtures. Both allocate
and drop their final dense output; the contender removes only the scratch row.
Divan includes allocation profiling and explicit result destruction inside its
timed section. Hyperfine includes fixture construction/destruction and process
startup/exit. Zero-render calibration estimates that overhead without subtracting
it from results. Native `time` logs retain process CPU and peak RSS observations.

Fixtures include a 16×16 tiny image, 1024×1024 opaque/sparse single layers,
5/20/100-layer sparse images, five fully occupied mixed-alpha layers, five uniform
translucent layers and 100 completely clipped layers. Sparse layers contain a
7×7 patch (sometimes crossing a tile boundary) and nonzero implicit hidden RGB. The dense fixture
populates every tile and varies alpha across 0/1/64/128/254/255; it is not an
implicit-default-only approximation of a dense image.

Raw evidence is retained under
[`benchmarks/results/cpu-spans-2026-10-19`](../benchmarks/results/cpu-spans-2026-10-19).
Exploratory runs are labelled separately and excluded from selection. Hyperfine
execution metadata stays `unreviewed`; a separate decision records the accepted
scope rather than rewriting raw execution status into a release claim.

## Remaining backend work

Desktop CPU/GPU presentation now requests exact [viewport regions](VIEWPORT_RENDERING.md),
while export APIs retain complete requested output. Regional CPU composition and
bounded output/lower-prefix reuse are covered separately in
[the regional compositor contract](REGIONAL_COMPOSITOR.md). A bounded [CPU box-sampling display](MULTIRES_DISPLAY.md) now handles zoomed-out
regions without full native surfaces. Finer operation damage,
GPU multiresolution/tiled output, GPU output reuse, parallel cache-miss integration,
SIMD comparisons and all-service memory/latency qualification remain. History and
file interoperability have their own partial implementations and remaining gates;
see [the current roadmap ledger](ROADMAP_STATUS.md). Admission ceilings are policy
bounds, not proof that arbitrary allocations fit system RAM.
