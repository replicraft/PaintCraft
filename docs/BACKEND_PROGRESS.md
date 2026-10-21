# Backend engineering progress

The [initial audit and slices 1 through 14](backend-progress/initial-through-fourteen.md)
and [slices 15 through 24](backend-progress/fifteen-through-twenty-four.md) retain
completed backend work and its scoped evidence. This file continues from ordered
paint planning onward.

## Twenty-fifth slice: ordered paint plans and operation admission

Painting now compiles ordered layer-local descriptors before allocating its
operation's tiles. Fills, pencil paths, smooth/unsmoothed strokes, shape fill plus
outline, masks, built-in text and flood regions use this shared path. Tile-local
write order and exact source-over/16-sample replacement math stay unchanged.
Scoped admission reserves touched COW/missing capacity before preparing buffers;
weak-only identities can transfer without new headroom. Actual allocations consume
the cumulative staged allowance and survive as independent leases when the
operation's unused reservation returns. A later batch denial includes earlier live
attempt demand. Oversized sparse operations stop at the staged limit rather than
indexing the entire logical canvas.

Replay groups descriptors by tile and uses contiguous row kernels. Targeted scalar
comparisons verify clipped overlap, hidden RGBA and coverage; quota tests verify no
allocated operation prefix, reader/COW behavior, whole-attempt retry demand and
bounded fresh-tile discovery. An editor test covers eight painting variants under
an injected two-tile quota. Existing command oracle/brush/preview/history tests pass.

The [local replay decision](../benchmarks/results/paint-plan-rows-2026-10-20/DECISION.md)
retains raw Criterion samples, three Hyperfine sessions and process resources for
three source snapshots. Earlier slower candidates remain archived. Final process
medians improve 1.41–1.53x for horizontal pencil, 1.14–1.15x for brush runs and
24.71–27.15x for the synthetic partial-replacement fixture. Diagonal pencil is a
non-win control: Criterion is slower and process results vary around a tie. Result
destruction and descriptor construction are included. These are same-source raster
API comparisons, not parent desktop/input latency or across-workload RSS claims.

[The paint-plan contract](PAINT_PLANNING.md) distinguishes operation admission from
whole-batch planning. Sequential data-dependent operations still use private staged
results; whole-batch/effect plans, bounded descriptor/index metadata, logical tile
eviction, all-service physical admission, priorities/fairness and sustained native
latency qualification remain. T11/T13/T14/T37 remain partial or in progress.


Matched CPU-only [baseline/current CLI apply measurements](../benchmarks/results/paint-plan-cli-cpu-2026-10-21/DECISION.md)
retain 480 process observations and byte-identical complete canonical outputs.
Horizontal pencil improves 1.12–1.20x, smooth brush 1.07–1.10x and smooth eraser
1.66–1.73x across three sessions; diagonal is a tie/slight regression. These
include geometry, history and file IO. A mixed-build pilot is archived/excluded;
shared-target artifact reuse was detected and corrected before included timings.
Neither comparison qualifies native input/display tails or general RSS reduction.

710 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Metal checks
and a 30-frame isolated native smoke. No production source changed after these
checks; the CLI-only confirmation rebuilt matching features from frozen commits.


## Twenty-sixth slice: cooperative geometry and paint cancellation

Detached Session execution now passes its cancellation predicate into command
execution, smooth coverage and operation-plan replay. Execution checks between
operations. Coverage checks between segments and every 64 scan/union rows; paint
plans check descriptor/tile indexing, buffer preparation and each write, with
checks every 64 rows inside writes. Existing noncancellable raster entry points
remain wrappers. Pixel equations, coverage ordering and publication stay unchanged.

Cancellation may leave a low-level caller's private raster partially painted;
Session discards it, returns unused reservations and releases private tile leases.
Raster tests cancel at every observed geometry/replay boundary and preserve the
base and zero residual decoded charge. An editor test checks cancellation during
preparation against unchanged owner snapshot, ticket and history. Successful
geometry still matches the independent sample-distance oracle.

713 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required real Metal checks
and a 30-frame isolated native smoke (exit zero, empty log).

These are cooperative work boundaries, not measured response-time guarantees.
Row sorting, individual row kernels, other geometry/command paths and codec/OS calls
remain nonpreemptible. Incremental stroke coverage, scheduling priorities and native
input/cancellation tail measurements remain. Earlier timing decisions stay frozen
to their recorded revisions and do not qualify this callback overhead. T13/T14 stay
partial; this does not complete the performance roadmap.


## Twenty-seventh slice: incremental sample-union preview geometry

The desktop render worker now retains one disposable smooth-path geometry entry
for CPU and GPU preview staging. Exact prefix/style/clip matches scan only appended
capsules, normalize retained sample intervals and resolve full coverage against
current private source pixels. It never blends a suffix onto already blended
preview pixels. Changed prefixes/policies rebuild, cancellation/errors retire partial
unions, poison clears derived state, and committed rendering releases the entry.
The provisional retained metadata charge is 8 MiB; transient work and physical/RSS
bounds remain separate. Single-point-to-segment transitions retain the original
scan-row limit. Compound previews can churn this single entry.

The worker also passes its cancellation predicate through preview command execution;
previously it called the compatibility noncancellable entry point. Session commits
remain independent of this derived geometry and retain atomic publication/history.
See [the coverage contract](INCREMENTAL_COVERAGE.md).

720 workspace tests pass (22 explicit ignores, including the new measurement driver),
strict workspace/all-target Clippy, formatting/file limits, release desktop/CLI builds,
three required real Metal checks and a 30-frame isolated native smoke. New tests
cover exact growing prefixes/retraces, subpixel/clipped/translated policy changes,
retention disablement, cancellation at each extension boundary, budget equivalence,
no old-segment rescanning, current selection/origin/opacity, cache retirement and poison.

[The desktop replay decision](../benchmarks/results/preview-geometry-2026-10-21/DECISION.md)
retains 120 Criterion samples, 360 comparison process runs, 60 calibration runs,
full-frame pre/postflight checks, frozen source/binary identities and process RSS.
Across three sessions, growing previews improve 1.63–1.71x and retraces 1.62–1.75x;
tiny process cases are 1–3% slower and stay non-win controls despite their noisy
statistical improvement. Cold setup/teardown and complete CPU frames are included;
empty process medians are 11.33–12.49 ms and are not subtracted. This is application
method replay, not OS input/display or GPU transfer latency or a parent-source claim.

Command/prefix validation, whole-union resolution, raster replay, compound-prefix
reuse, finer dirty regions, GPU residency, scheduling priority and measured native
input/display tails remain. T13/T14/T37 remain partial or in progress. Earlier
benchmarks stay tied to their frozen source; no broad performance completion claim.


## Twenty-eighth slice: regional dependency identities and cached output patches

Raster tiles retain sixteen process-local 64-pixel region stamps and their initial
uniform RGBA. COW preserves untouched identities; pixel/rectangle/planned writes
retire intersecting regions, while raw import/materialization retires every region.
Installed leases retain actual initial colors independently of destination defaults.
Monotonic revision exhaustion disables cache eligibility rather than wrapping.
Tokens pin neither pixels nor weak control blocks; resident Tile structs now enter
the conservative raster metadata weight. Pixel/state/wire semantics stay unchanged.

Matching ordered layer geometry/policies can reuse a complete output tile and
privately recompose only changed, nonoverlapping 64-pixel output cells. Translated
source damage is conservatively unioned. Structural/global changes keep full/prefix
routing, preview partitions stay separate and cancelled patches install no partial
image/key. A local pixel can now recompose 4096 rather than 65536 pixels within the
same 256-pixel output tile. See [the invalidation contract](REGIONAL_INVALIDATION.md).

727 workspace tests pass (22 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required real Metal checks
and a 30-frame isolated native smoke. Tests cover default/COW and installed identities,
imports/restoration, saturation/concurrent uniqueness, unpinned decoded capacity,
unaligned origins/31–1024 grids, overlaps, cancellation and undo/redo. An initial
workspace/example/bench build exhausted disk before timing; generated debug objects
and incremental caches were reclaimed, two-job builds passed, and failed build logs
are retained separately from timing evidence.

[Current compositor measurements](../benchmarks/results/regional-patches-2026-10-21/DECISION.md)
retain 400 Criterion samples and 1440 Hyperfine observations, including cold/zero-edit
controls. Keep 256-pixel output tiles: 64 increases lookup overhead; 128/256 are close
or mixed in dense/viewport cases. Current tile256 versus full composition ratios are
1.09–1.20x for sparse5, 4.37–4.64x for dense5 and 1.09–1.49x for viewport100. Single
sparse-layer caching regresses; global-bottom results reverse sign and drift sharply
in session 3. All samples remain and calibration is never subtracted. These are
current configuration comparisons, not an isolated old/new patch speedup claim.

[Desktop consumer confirmation](../benchmarks/results/regional-patches-app-2026-10-21/DECISION.md)
adds 420 process observations over actual preview batches, checked staging and CPU
composition. Dense5 improves 1.54–1.89x; the single-layer bypass is a tie. Sparse5
improves in two sessions and regresses in the third. Cached sparse/dense runs have
higher RSS, so no general memory gain is claimed. Full-frame/source pre/postflights
and source/executable identities accompany both records. No OS input/display or GPU
latency is inferred; desktop output tiles and existing single-layer policy stay put.

Exact operation rectangles, effect halos/groups/profiles, parallel cache misses,
multiresolution, GPU texture validity/residency, globally dirty cost routing,
all-service physical admission, scheduling and native latency/interference remain.
T12/T21/T37 stay partial/in progress; the full roadmap remains active.


## Twenty-ninth slice: bounded resident GPU source views

The integer GPU compositor now retains clipped dense layer-source buffers keyed by
document/layer identity, geometry/defaults and 64-pixel region versions. Unchanged
sources upload nothing; changed source bands coalesce into contiguous full-width
strips copied in the same encoder as ordered composition. Empty transparent implicit
layers skip upload/dispatch. Default retained bounds are 64 MiB of buffer sizes,
4 MiB of metadata charge and 64 views. Current-frame sources are protected; overflow
streams instead of cycling out the entire working set. These are provisional guards,
not sparse atlas or physical GPU/RSS admission. Keys pin no decoded host tiles.

Source identities advance only after completed layer work and epoch/cancellation
checks. Unsubmitted encoders discard incomplete copies; errors/cancellation retire
all source entries and poisoned locks recover cold. Native declaration adds 76 MiB
for buffer capacity and old/new/strip metadata; its UI query is immutable and cannot
wait on the GPU mutex. Host-backed document/history/save semantics stay unchanged.
See [the source contract](GPU_SOURCES.md).

735 workspace tests pass (22 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, nine required real Metal checks
and an isolated 30-frame native smoke. Required tests cover exact parity, clipping,
hidden RGB, unchanged/partial uploads, scope separation, overflow/streaming, decoded
lease release, every observed cancellation boundary, poisoning and device recovery.
A test-only follow-up additionally exercises cross-frame eviction after a source
becomes invisible; production and benchmark driver remain frozen for timing evidence.

[Actual desktop GPU preview measurements](../benchmarks/results/gpu-sources-2026-10-21/DECISION.md)
retain 120 Criterion samples and 420 Hyperfine observations with full-frame/source
pre/postflights and frozen source/shader/executable hashes. Across 32-frame sequences,
source staging falls from 16 to 8.25 MiB (one layer), 80 to 10.25 MiB (dense5), and
1040 to 48 MiB (dense65). Complete-process ratios are only 1.02–1.05x, 1.05–1.19x
and 1.03–1.56x, respectively, with substantial host/GPU variation. A flagged first
timed 2.474-second run is retained. Calibration is not subtracted; adapter/pipeline
construction, raster staging, all dispatch completion, readback and teardown stay
included. RSS maxima do not improve universally, and no bus-traffic or native
latency conclusion follows from source-byte counters.

GPU composition still allocates/computes/reads back full output; sparse GPU working
tiles, atlas/output residency, asynchronous/region transfers, CPU/GPU dispatch
thresholds, physical all-service admission and native device/window recovery and
input/display tails remain. T12/T13/T21/T37 remain partial or in progress.


## Thirtieth slice: completed GPU display surfaces and native registration

Native windows now register a completed GPU texture directly instead of reading
straight pixels back, constructing a CPU ColorImage and re-uploading every frame.
The new conversion shader matches selected egui/ecolor 0.37 integer premultiplication
exactly in Rgba8Unorm. Host document/history/save semantics and straight render/export
APIs stay independent. Readback copying is fallible and unmaps through a scope guard.
Explicit texture readback removes padded rows and returns display-premultiplied bytes.

Accepted frames own texture/device/epoch; the UI validates job/document/preview keys,
canvas size and target epoch before registration. Rejected results register nothing.
Replacement, CPU fallback, document/tab reset, backend replacement and drop free old
IDs. Workers never lock egui's renderer. Headless harnesses remain CPU-backed;
App::rendered() is explicitly empty for direct native surfaces. See [the display
contract](GPU_DISPLAY.md). Output pooling and region/mipmap presentation remain open.

740 workspace tests pass (23 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits and release desktop/CLI builds. Eleven required renderer GPU
tests and three real native registration/draw tests pass on Metal, including every
color/alpha pair, odd clipped extents, cancellation, device/stale epoch rejection,
CPU fallback and actual egui native/managed pixel equality. A 30-frame release window
smoke exits zero with an empty log. A parallel suite exposed an existing test's
synchronous-drop assumption: abandoned maintenance may still own a tile until actual
completion. The assertion now waits for observed release; no production lifecycle
changed. This correction has its own commit.

[Actual texture-readiness evidence](../benchmarks/results/direct-display-2026-10-21/DECISION.md)
retains 120 Criterion samples and 420 Hyperfine observations. Both complete GPU work;
the control also completes CPU conversion/backend upload. Across sessions, sparse1
ratios are 1.05–1.13x, dense5 is mixed (0.99–1.25x), and dense65 is effectively a tie
(1.00–1.03x). Large host/GPU variability and statistical/process discrepancies stay
visible; cold device/renderer/font/fixture setup and teardown remain included and
calibration is not subtracted. Process RSS is lower in these fixtures but does not
qualify physical GPU budgets. Pixel/source pre/postflights and frozen source/shader/
executable identities accompany all runs. The failed zero-size font-atlas preflight
collected no timings and remains archived with its prototype source.

No OS swap/input/frame-tail, Windows reference, universal throughput or physical
memory claim follows. CPU/GPU dispatch thresholds, output pooling, viewport and
multiresolution surfaces, sparse GPU working tiles, asynchronous services, native
device recreation and all-service admission remain. T12/T13/T21/T37 stay partial;
the full roadmap remains active.

## Thirty-first slice: exact native viewport composition and presentation

Classic and Studio now request only the visible integral document region, using
floor/ceil coverage at fractional DPI/zoom. Rendering starts after canvas layout,
so the first display does not request a full-canvas output. CPU composition uses
existing exact regional APIs/cache; GPU ordered composition keeps world-space
source-cache identities and rebases output indices. Full render/export APIs remain
complete. GPU textures carry their own document bounds. See
[viewport rendering](VIEWPORT_RENDERING.md).

Frame keys now include the region. Old preview progress cannot cross viewport
changes, and accepted textures remain positioned at their own document bounds
while replacement work runs. Native origin/dimension/epoch checks precede
registration; CPU fallback uses the same region. Worker output admission follows
requested area, without claiming full-preview or physical GPU memory bounds.
CPU upload checks backend texture limits and can recover after a smaller request.
A test exposed the former oversized-texture assertion; failed validation is retained.

747 workspace tests pass (24 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits and release desktop/CLI builds. Thirteen required renderer
Metal tests pass, including clipped offset/opacity/order parity, warm cache reuse,
odd strides, invalid/cancelled requests, a small region of a full-render-rejected
16384-square canvas and subsequent full exports. Native registration/draw tests
pass (3), including exact actual egui framebuffer parity for cropped/full images.
Two desktop regional tests run on required Metal. GUI tests exercise huge-canvas
regional buffers across DPI/zoom/multipass changes without editing the document;
a separate CPU test verifies safe backend-limit failure/recovery. A 30-frame release
native window smoke exits zero with an empty log.

[Measured completed texture readiness](../benchmarks/results/native-viewport-2026-10-21/DECISION.md)
retains 120 Criterion samples and 420 Hyperfine observations. The fixed 128×96 view
has process baseline/contender ratios 1.06–1.07x and the panning view 1.07–1.11x;
the whole-canvas control is mixed/tied (0.98–1.07x). Cropped output pixel counts are
10.67x lower; source upload counts also fall, while cold setup and synchronous work
limit timing gains. All controls, slow observations, memory measurements, calibration,
raw exports and development failures remain visible. Source and executable hashes
match the frozen reproduction commit. This is texture readiness on one Metal host,
not OS input/display latency, cross-platform conformance or physical memory admission.

Multiresolution/tiled display, reusable outputs, sparse GPU working tiles, asynchronous
transfers, full incremental operation staging, device recreation, priority/fairness
and native latency/interference remain. Zoomed-out oversized source regions still
need multiresolution output. T12/T13/T21/T37 remain partial/in progress.

## Thirty-second slice: completed private GPU compositor buffer retention

An explicit per-renderer output pool can retain one completed private compositor
buffer under a configurable byte cap. A fresh buffer uses wgpu zero initialization;
reuse clears exactly the requested logical output before blending, including empty
or fully clipped compositions. Only observed readback/conversion completion permits
recycling. Failure/cancellation/unwind does not recycle private submitted work, and
ordinary errors retire idle caches. Smaller requests may reuse larger capacity;
oversized requests stream. Native admission can read an immutable retention allowance
without GPU locks. See [the ownership contract](GPU_OUTPUT_POOL.md).

Display textures remain separately owned. Their public views can be cloned, so frame
Drop alone cannot prove they are safe to overwrite. This pool does not recycle them.
Metal tests keep older textures live across scratch reuse/idle eviction, validate
exact transparency, sizes, disabled/over-cap behavior, cancellation checkpoints,
retry and retired epochs. Actual egui framebuffer tests draw an old registered view
after later composites reuse scratch; CPU pixels remain exact. No document/history
references or authoritative edits are retained in the pool.

[The completed native experiment](../benchmarks/results/output-pool-2026-10-21/DECISION.md)
retains 120 Criterion samples and 420 process observations for small, 4096×2160 and
alternating-size views. Logical compositor buffer creations fall from 16 to one per
sequence, but small-view process ratios are 0.88–0.97x (pooled slower), 4K is mixed
at 0.75–1.07x, and alternating views show 1.04–1.22x process ratios while Criterion
regresses. Large host variation and slow observations remain unexcluded. Alternating
views end small but retain 33.75 MiB; peak RSS is not proof of physical reclamation.
The default is therefore fresh allocation (zero retention). Hosts can opt into a
nonzero cap; no universal performance gain or dispatch threshold is claimed.

750 workspace tests pass (24 explicit ignores), strict workspace/all-target Clippy,
format/file limits and release desktop/CLI builds. Sixteen required Metal renderer
tests and three real native registration/draw tests pass. The measured candidate
and final zero-default policy have separate source/validation records; a final
30-frame release window smoke exits zero with no log output. Native GUI stress,
physical budgets/pressure eviction, texture reuse ownership, multiresolution/tiled
output, asynchronous transfers and device recovery remain. T12/T13/T21/T37 remain
partial/in progress; this experiment does not complete the roadmap.

## Thirty-third slice: bounded composite-before-filter CPU display

Zoomed-out Classic/Studio canvases now use exact ordered native composition followed
by alpha-weighted encoded-sRGB box sampling. A validated request aligns source bounds
to a global power-of-two grid, normalizes partial canvas-edge boxes over actual pixels
and keeps their full texel footprint clipped to the canvas. Native-resolution display,
exports, sampled-composite tools and authoritative hidden RGB remain unchanged. See
[the scoped display contract](MULTIRES_DISPLAY.md); historical Paint sampling is not
qualified by this provisional filter.

Conservative raster metadata proofs avoid scanning uniform source pixels, including
COW/installed initial colors that differ from the layer default. Unknown regions split
into at most 256-square native blocks and bounded accumulators. Coarse output keys
include level and ordered source dependencies; committed/preview partitions remain
separate and large keys bypass before huge virtual-grid enumeration. A million-square
implicit canvas produces a 1024-square display; changing one black 256-square tile
visits at most 65,536 native pixels with independently verified weighted output.

Native frame freshness includes level, worker output declaration uses coarse dimensions
and an added 15 MiB cache/key/reduction allowance, and the painter preserves partial-edge
footprints. Scaled frames currently use the CPU oracle/managed upload while leaving the
GPU available for level zero. This is not GPU scaled composition or all-service physical
admission. Native GPU multiresolution, tiled presentation and hierarchical refinement
remain open.

762 workspace tests pass (25 explicit ignores), strict workspace/all-target Clippy,
format/file limits and release desktop/CLI builds. Sixteen required Metal renderer
tests and three native registration/draw tests remain green. An admitted native worker
handles reduced million-square output without disabling its GPU. GUI tests fit/restore
large documents without source/history changes, force layout passes and verify actual
partial-edge mesh footprints/clipping. Independent tests cover offset/alpha/order,
compose-before-filter counterexamples, level/preview isolation, cancellation, poisoned
cache fallback and unaligned dense recursive splits. The latter test-only follow-up
is recorded separately from the frozen measured source. A 30-frame default native
window smoke exits zero with no log; giant live OS input/display tails remain unqualified.

[Completed native managed-upload evidence](../benchmarks/results/multires-display-2026-10-21/DECISION.md)
retains 180 Criterion samples and 600 Hyperfine observations, including the existing
cached native-resolution compositor before filtering. Sparse 4K process ratios are
3.12–3.20x; tiny controls are tied (1.00–1.02x), and dense input is mixed/tied against
that established cache (0.99–1.09x). Coarse caching improves dense input over uncached
reduction by 1.29–1.36x, while adding approximately 0–6% to the large uncached case.
The 4K process RSS medians fall from roughly 77 MiB to 43–45 MiB, without promoting
physical GPU or universal memory claims. Raw samples, calibration, slow controls,
source/executable hashes, development failures and scope limits remain retained.

Optimal level/layout/dispatch policies, parallel cache misses, dense giant data,
GPU scaled pipelines, asynchronous services, device recreation, global physical
budgets, full incremental staging, native pressure/interference/tails and reference
parity remain. T12/T13/T21/T37 stay partial/in progress; the roadmap remains active.

## Thirty-fourth slice: exact bounded GPU scaled display and measured dispatch

GPU display now composes bounded native blocks before exact box reduction using
portable pairs of u32 words for wide sums and division. Metadata-proven uniform
composites avoid native expansion; unknown blocks preserve ordered layer rounding.
Both 32- and 64-sample output tiles produce identical pixels. Textures retain
independent ownership through later scratch reuse; cancellation, epoch and storage
limits cannot publish partial frames. Native acceptance checks sampling level,
bounds, dimensions and identity. Ordinary scaled-resource failures use uploadable
CPU pixels while retaining a working native-resolution GPU. See
[the implementation contract](GPU_SCALED_DISPLAY.md).

[Completed native evidence](../benchmarks/results/gpu-scaled-display-2026-10-21/DECISION.md)
retains 240 statistical samples and 780 process observations. GPU64 process ratios
(CPU/GPU) are 0.43–0.86 for tiny, 0.29–0.36 for dense multilayer, 0.95–1.03 for sparse
4K brush and 1.20–1.36 for uniform 4K opacity edits. Allocation reductions do not
remove repeated native submissions or changed-view source retirement. All slow
controls, calibration, memory data and source/binary identities remain. The CPU
scaled path stays automatic; `PAINTCRAFT_GPU_SCALED=1` enables explicit native
experimentation with 64-sample tiles. Native-resolution GPU display stays enabled.
No general crossover, historical sampling parity or OS latency claim follows.

769 workspace tests pass (25 explicit ignores), all 220 desktop library tests pass
with required real GPU availability, and all 20 renderer Metal tests pass. Oracles
cover both tile edges, 164 analytic trillion-pixel composites including odd canvas
dimensions, dense wide carries, cancellation and old-frame ownership. Actual egui
framebuffer tests cover native/scaled output and partial edges; native workers test
both default/experimental giant views and constrained-device CPU fallback. Two
development commands omitted the GPU feature and executed zero tests; they are
retained and excluded from validation. The corrected invocations execute real tests.
Strict workspace/all-target Clippy, formatting/file limits and release desktop/CLI
builds pass. Isolated 30-frame release windows exit zero with empty logs under both
default and experimental settings; these startup smokes do not qualify scaled OS
interaction latency. Final-policy source identities and validation remain separate
from the frozen measured executable.

Single-layer source bypass, sparse GPU working sets/output caches, asynchronous
services, native device reconstruction, global physical budgets, full incremental
staging, pressure/interference/tails and reference parity remain. T12/T13/T21/T37
remain partial/in progress; this baseline does not complete the roadmap.

## Thirty-fifth slice: bounded regional GPU source working sets

The source cache can retain multiple exact clipped views of a compatible layer
under its existing 64 MiB/4 MiB/64-entry limits. Views patch changed rows independently;
layer-scope changes retire incompatible sources. Request protection now spans the
whole scaled display request, so over-cap working sets keep early sources and stream
overflow rather than resetting eviction protection at each internal native block.
No source view aliases authoritative pixels or an independently owned display frame.
No dependency, atlas, asynchronous service or physical budget is added. See
[source ownership and bounds](GPU_SOURCES.md).

[The native comparison](../benchmarks/results/gpu-region-sources-2026-10-21/DECISION.md)
retains 320 raw statistical samples and 1,020 process observations across both
tile edges. In the dense five-layer edge-64 sequence, source staging falls from
20 MiB to 3.375 MiB and logical source creations from 320 to 40, retaining 2.5 MiB
instead of 0.3125 MiB. Edge 32 exceeds the entry cap and preserves its first 64
views while streaming overflow. Native composition and completion waits are unchanged.

Timing is mixed and statistical samples mostly tie. Dense edge-64 process ratios
are 0.971/1.063/1.405 across sessions, but controls with identical source work also
swing substantially. Sparse edge-64 shows a 1.733 ratio in one session despite
identical one-view source work. All calibration, slow observations, raw exports,
source/binary identities and RSS remain retained. The original single-view policy
stays default; callers explicitly opt into regional retention through the renderer
builder. No general throughput improvement or desktop dispatch threshold is claimed.

772 workspace tests pass (25 explicit ignores), 23 required Metal renderer tests
and 220 required-GPU desktop library tests pass, with strict workspace/all-target
Clippy, format/file limits and release desktop/CLI builds. New tests verify a full
40-view hit set, one-row-band patch, opacity/order reuse, moved-layer invalidation,
three-entry overflow, cancellation checkpoints and old-frame ownership. Native
benchmark oracles execute actual registration with both policies and independently
check every required pixel. Isolated 30-frame release startup smokes cover the
existing default/experimental render settings; they do not enable regional retention
or qualify native input/display latency.

Per-layer submission/completion is the next GPU measurement target. Sparse
GPU atlases, bounded dispatch/transfer services, device reconstruction, physical
budgets, fairness, native tails and broader feature/reference work remain open.
T12/T13/T21/T37 remain partial/in progress; the full roadmap stays active.

## Thirty-sixth slice: bounded ordered GPU submissions

Consecutive layer dispatches can now share an encoder under 16-layer and normally
1 MiB source windows. One larger source runs alone; pending keys flush at 1 MiB
plus at most one admitted key. Separate compute passes preserve exact original
layer order/opacity rounding before reduction. Cached sources are protected during
preparation, with new identities published only after successful completion and
fresh cancellation/device checks. Unsubmitted work can be dropped; old display
frames stay independently owned. The native worker adds an immutable 4 MiB pending
allowance without GPU locks. This is not physical driver/RSS admission. See
[the batching contract](GPU_BATCHING.md); no new dependency is introduced.

[Completed native evidence](../benchmarks/results/gpu-submission-batches-2026-10-21-rerun/DECISION.md)
retains 480 statistical samples and 1,500 process observations across single-layer,
5/20/100-layer, sparse 4K and uniform controls under both source policies. Compositor
submissions fall from 320 to 64 (five layers), 1,280 to 128 (twenty) and 6,400 to
448 (one hundred), with identical source work inside each policy pair. Single-view
twenty-layer process ratios are 1.80–2.37 and hundred-layer 2.03–2.21; regional
ratios are 1.52–1.77 and 2.17–2.40. Five-layer timing is mixed, with regressions
retained. Twenty-layer RSS rises from roughly 36–37 to 52–54 MiB single-view, and
hundred-layer from 77–78 to 96–97 MiB. No global throughput, physical-memory,
CPU/GPU crossover or OS event/display-tail claim follows.

Serial single-view rendering stays default. Native experiments can explicitly set
`PAINTCRAFT_GPU_BATCHING=1` or `=regions`; zoomed-out GPU display still independently
requires `PAINTCRAFT_GPU_SCALED=1`. Exact native/readback/scaled output shares the
same compositor. The measured implementation passes 777 workspace tests (25 explicit
ignores), all 27 required Metal renderer tests and strict all-target Clippy. Final
native-option integration separately passes 221 required-GPU desktop tests,
strict workspace/all-target Clippy, release desktop/CLI builds and isolated
30-frame startup smokes for default/batching/regional-batching settings. Pixel
oracles cover layer/byte bounds, solitary oversized sources, cache overflow,
cancellation checkpoints, epoch retirement and actual native framebuffer output.

The first attempt's terminal ENOSPC export failure remains archived with 480 raw
statistical samples and 300 raw process observations; the missing fourth variant
is not invented from its printed summary. Only generated dev-profile artifacts
were reclaimed, preserving frozen release/source hashes. The complete retry records
approximately 15.59 GiB minimum sampled free space and retains all raw samples,
slow controls, calibration, memory and interrupted evidence separately.

Asynchronous transfers/completion, sparse GPU atlases, native device reconstruction,
physical budgets, scheduling fairness, native pressure/tails and broader tool,
format, frontend and reference work remain. T12/T13/T21/T37 remain partial/in progress.

## Thirty-seventh slice: completion failure quarantine

All GPU completion waits now share a failure boundary. Poll timeout/invalid-index
errors and readback callback/mapping failures retire the shared device epoch before
returning an error. A successful wait followed by cancellation still leaves the
device usable. Render entry-point errors clear source/pool retention; independently
retained frame failures invalidate logical use immediately, with renderer-held
storage cleared on its next rejected request or Drop. Old frames and later calls
cannot use that lifetime. No numerical policy, command wire data or dependency
changes. See [the completion contract](GPU_COMPLETION.md).

A real Metal test passes a second device's invalid submission index to the shared
wait boundary, verifying actual poll failure without depending on a loss callback.
It checks old-frame rejection, source/private-pool cleanup on the next request,
unchanged CPU pixels and fresh-device reconstruction. Pure policy tests cover
timeout and callback timeout/disconnection without claiming a physically hung GPU.
A native-renderer test retires an epoch after registration, verifies unchanged
editor publication identity and exact CPU output, then completes managed upload on
the still-working presentation device. Existing real-GPU cancellation/retry tests
remain green.

780 workspace tests pass (25 explicit ignores), the 27 required Metal integration
tests plus 11 renderer unit tests pass, and all 222 required-GPU desktop library
tests pass. Strict workspace/all-target Clippy, format/file limits, release desktop
and CLI builds and default/experimental 30-frame native startup smokes pass.
No performance experiment is warranted for this error-path correctness fix, and
no throughput gain is claimed.

A failed wait does not prove submitted work stopped. Quarantine prevents further
PaintCraft submissions; it does not transfer an ended worker charge into a physical
outstanding-driver lease, repair a hung driver or recreate the native presentation
device. Those accounting/recovery limits are now explicit in the lifetime owner.
Physical budgets, asynchronous services, native recovery/tails and broader features
remain. T12/T13/T37 stay partial/in progress; the full roadmap remains active.

## Thirty-eighth slice: admitted ownership through unobserved GPU work

Runtime now pairs pool-local and shared byte leases behind one explicit resource
group. Cancellation tokens observe it weakly, so ordinary clones do not pin completed
jobs. `resource_lease()` lets actual external work retain both counters beyond task
completion/result consumption or executor Drop. A pool with no pending handles can
still report reserved bytes, and cannot exceed its local limit through premature
result consumption. Original retained rejection and overflow diagnostics remain.

Native render workers attach that group to synchronous PaintCraft GPU calls on
their worker thread. Every submission captures scope ownership before dispatch.
Successful waits clear the extra reference without a callback. Failure or unwind
instead transfers ownership to queue completion observation. Nested scopes retain
all enclosing charges and restore on error/panic, with a 16-scope bound. Ending
the queue context releases callback ownership without a permanent cycle. Returned
completed frames do not pin a task charge. See [the ownership contract](GPU_JOB_ADMISSION.md).
No dependency, wire-policy change or throughput improvement is introduced.

788 workspace tests pass (25 explicit ignores), with all 27 required Metal
integration tests and 16 renderer unit tests, 222 required-GPU desktop tests,
strict workspace/all-target Clippy, format/file limits and release desktop/CLI
builds. Default/regional-batching 30-frame native startup smokes exit zero with empty
logs. Real Metal tests cover failed invalid-index waits, unwind after submission,
nested charges, explicit completion observation and queue teardown. Runtime tests
verify both limits after result consumption, executor Drop/cancellation and cheap
token release. A development run caught a changed overflow diagnostic; it was
restored before the passing full validation, and the failed log remains retained.

This closes declared native rendering ownership after an unobserved wait, rather
than claiming the device stopped at timeout. Callbacks still need polling or context
teardown; scopes neither propagate to spawned tasks nor intercept raw wgpu calls.
Physical driver allocation/reclamation, global all-service budgets, asynchronous
completion progress, scheduling fairness, native device recovery/tails and broader
features remain. T12/T13/T37 stay partial/in progress; the full roadmap is active.

## Thirty-ninth slice: completion progress independent of repaints

Native windows now own a bounded completion observer independently of their active
compositor. Successful waits retain the direct-release path. Unobserved submissions
wake the worker, which requests nonblocking polling outside its status lock and
sleeps between attempts; idle observers do not poll. Compositor retirement and CPU
fallback keep the native observer alive. Final-owner Drop signals shutdown without
joining the UI thread. Callback links are weak, avoiding context/observer cycles.
See [the completion-progress contract](GPU_COMPLETION_PROGRESS.md).

A new mismatch test exposed equal wgpu queue IDs across independent instances.
Submissions now carry a private renderer identity shared with returned frames;
only a matching observer tracks them. Nested scopes search outward for a match.
This conservatively requires separate observers for separately built renderers.
Real Metal tests cover failed invalid-index waits with nested charges, unwind,
zero idle/successful polling, mismatched instances, worker shutdown and pending
callback teardown. The failure test retires and drops the compositor while keeping
a queue handle alive, excluding teardown as a substitute for completion observation.
Native admitted-worker tests use the observer and native fallback preserves it.

795 workspace tests pass (25 explicit ignores), with all 27 required Metal
integration tests, 23 renderer unit tests and 222 required-GPU desktop tests.
Strict workspace/all-target Clippy, format/file-length checks and release desktop/CLI
builds pass. Default/regional-batching 30-frame native startup checks exit zero
with empty logs; these do not qualify OS input/display tails. The
[validation record](validation/gpu-completion-progress-2026-10-21/validation.json)
pins source, commands, compressed logs and release executable hashes.

This is declared admission progress, not asynchronous rendering, a physical-memory
ceiling, protection against a stalled opaque driver call or device reconstruction.
Observer startup/poll failure preserves callback ownership and requires external
observation or teardown. The 16-worker bound is a service policy, not a measured
optimum. No throughput gain or new rendering default is claimed. Physical budgets,
priority/fairness, asynchronous transfers, native recovery/tails and broader feature
work remain. T12/T13/T37 stay partial/in progress; the full roadmap remains active.

## Fortieth slice: direct native single-layer display

An explicit renderer/native option now converts a fully covering single raster
source directly into a new native display texture. It uses the existing bounded
source cache and exact default-v1 opacity/encoded-sRGB premultiplication, avoiding
the intermediate compositor buffer and its completion boundary. Multiple potential
contributors or partial coverage fall back to the original ordered compositor.
Returned textures remain independently owned; patches to later source views cannot
change old frames. Straight exports and scaled display paths are unchanged. No
new dependency, writable document alias or reduction in conservative admission is
introduced. See [the direct-display contract](GPU_SINGLE_LAYER.md).

Required Metal tests exhaust all 65,536 alpha/opacity pairs against direct integer
expectations, then cover sparse hidden RGB, offsets/odd regions, source hits/patches,
streaming, cancellation checkpoints, epoch rejection and old-frame ownership. The
actual egui framebuffer and admitted native worker match independent CPU output.
801 workspace tests pass (26 explicit ignores), including all 31 required Metal
integration tests, 23 renderer unit tests and 224 required-GPU desktop tests.
Strict workspace/all-target Clippy, formatting/file limits, release desktop/CLI and
optimized benchmark builds pass. Three isolated 30-frame native startup checks
cover default, direct single-layer and combined regional-batching options, with
zero exits and empty logs. [Source-pinned validation](validation/gpu-single-layer-2026-10-21/validation.json)
retains commands/logs and corrected development compile diagnostics.

[The native comparison](../benchmarks/results/gpu-single-layer-2026-10-21/DECISION.md)
retains 240 raw statistical samples and 780 process observations over six cases,
including independent per-pixel pre/postflights, reverse order, calibration, RSS
and fallback controls. Eligible frames remove eight intermediate allocations and
eight compositor submissions, with identical source uploads. Tiny/dense statistical
means favor the contender; large-view process ratios include a 0.966 regression.
Fallback controls vary substantially despite unchanged work, including disagreement
between the five-layer statistical regression and process apparent gains. No slow
observation is excluded. Hyperfine 2.0 exports remain byte-preserved; summary
analysis uses their actual schema-v2 measurements instead of older `times` fields.

Library/native defaults remain ordered. `PAINTCRAFT_GPU_SINGLE_LAYER=1` enables the
tested exact implementation as an explicit experiment, without claiming a general
throughput/RSS win or an inferred size threshold. Sparse GPU working sets, CPU shared
surface bypass, asynchronous services, physical budgets, native device recovery,
input/display tails and broader feature work remain. T12/T13/T21/T37 remain partial
or in progress; the full roadmap stays active.

## Forty-first slice: bounded paint-plan metadata

Paint plans now bound descriptor capacities, usable tile-index payload and per-tile
write-index capacities independently of decoded tiles. The default is a provisional
64 MiB per plan, configurable by SDK callers through raster admission or explicit
plan construction. All editor painting families use the admission's plan factory;
replay enforces the smaller plan/admission allowance. Repeated coordinates reuse
buckets, compatible adjacent writes still coalesce, and default/no-op missing tiles
do not require speculative table growth. No new dependency or numerical/wire
policy is introduced. See [the metadata contract](PAINT_METADATA.md).

Metadata exhaustion returns a fixed resource error without decoded denial context,
so freeing history cannot trigger an inappropriate retry. Index failures precede
that operation's decoded materialization/replay. A later operation can still fail
after an earlier private prefix; Session discards the whole attempt and preserves
owner pixels, ticket and history. This closes a prerequisite for retaining compound
plans rather than claiming whole-batch preflight. Aggregate batch/service admission,
input/geometry/font storage, hash controls and allocator/transient/RSS overhead remain
outside the per-plan payload cap. The 64 MiB policy is not a benchmark-selected optimum,
and no throughput or memory-reclamation gain is claimed. Prior timing records stay
frozen to their original source; new guard overhead remains unqualified.

Six new tests cover descriptor limits/coalescing, repeated overlapping writes with
one tile, table/vector exhaustion before any decoded prefix, tight-boundary reuse,
all eight painting families and late-ellipse rollback after a successful private
fill. 807 workspace tests pass (26 explicit ignores) across 67 suites; strict
workspace/all-target Clippy and release desktop/CLI builds pass. All 31 required Metal integration tests, 23 renderer unit tests and 224 required-GPU
desktop tests pass. Default/direct-single-layer 30-frame native startup checks exit
zero with empty logs. The [validation record](validation/paint-metadata-2026-10-21/validation.json)
pins source, commands, compressed logs and release hashes for this correctness milestone.
Whole-batch/effect planning, coordinated physical budgets, scheduling, asynchronous
GPU work, native recovery/latency and broader feature work remain. T04/T11/T14/T37
remain partial/in progress; the full roadmap stays active.
