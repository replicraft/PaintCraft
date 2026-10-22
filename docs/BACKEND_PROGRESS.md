# Backend engineering progress

The [initial audit and slices 1 through 14](backend-progress/initial-through-fourteen.md),
[slices 15 through 24](backend-progress/fifteen-through-twenty-four.md) and
[slices 25 through 34](backend-progress/twenty-five-through-thirty-four.md) retain
completed backend work and its scoped evidence. This file continues from regional
GPU source working sets onward.

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

## Forty-second slice: cancellable bounded flood classification

Flood fill now checks the worker token during bitmap initialization, frontier
processing and wide-row expansion/marking/neighbor scans, including single-span
regions. Bitmap, retained frontier and output-span capacities share a configurable
64 MiB per-call baseline with fallible growth. The old public region API delegates
to the checked path. Signed clips are area-validated in wide arithmetic and neighbor
steps cannot overflow at signed endpoints. The editor propagates raster admission's
scratch policy and compiles already-clipped local spans directly, removing the
infallible duplicate document-coordinate conversion vector. No dependency,
command schema, numerical policy or publication path changes. See
[the flood preparation contract](FLOOD_PREPARATION.md).

An independent BFS checks all 512 binary 3×3 images over every seed, two clips and
four tolerances: 36,864 cases, including hidden RGB/alpha. Tests cancel every observed
checkpoint in a wide one-row fill, cancel large bitmap initialization, bound
bitmap/frontier/output growth and reject overflowing clip areas. Editor regressions
observe earlier staged walls, preserve negative origins/selections, propagate fixed
scratch limits through clones and discard a private prefix on classification
cancellation or quota failure. Fixed scratch errors carry no decoded denial and
cannot manufacture a history-pressure retry.

815 workspace tests pass (26 explicit ignores) across 69 suites, with strict
workspace/all-target Clippy and release desktop/CLI builds. All 31 required Metal integration tests, 23 renderer unit tests and 224 required-GPU
desktop tests pass. Default/direct-single-layer 30-frame native startup checks exit
zero with empty logs. The [validation record](validation/flood-preparation-2026-10-21/validation.json)
pins source, commands, compressed logs and release hashes for this correctness milestone. The 1,024-step
check interval and 64 MiB allowance are safety baselines, not measured optima or
wall-clock response guarantees. Returned spans and later plan construction can
still overlap under separate allowances. Aggregate batch/service leases, allocator/
transient/RSS limits, other command kernels, whole-batch planning, native cancellation/
display tails, GPU recovery and broader feature work remain. No throughput, RSS or
native latency improvement is claimed. T04/T13/T14/T37 remain partial/in progress;
the full roadmap stays active.

## Forty-third slice: streamed shapes and exact large ellipses

Rectangle/ellipse preparation now streams clipped spans directly into the admitted
paint plan, checking cancellation before each row and propagating sink failures.
Fill and outline retain their shared plan and original ordering; no intermediate
geometry Vec is needed in the editor. Checked endpoint differences build the
inclusive box safely. Existing collection APIs remain for callers that own their
allocation policy. Line/unsmoothed brush unions and font geometry retain their
other paths. See [the shape preparation contract](SHAPE_PREPARATION.md).

Large legal ellipses previously overflowed intermediate products even for tiny
clipped canvases. Ordinary rows now use checked u128 products; wider rows compare
exact u256 high/low products and use at most 63 bisections. No floating quotient,
approximate pixels, new dependency or public big-integer type is introduced.
Outline insets and empty local pieces avoid signed endpoint overflow. Existing
default-v1 centre/inset semantics remain unchanged.

Independent tests cover 2,880 small shape/clip/thickness combinations, exact large
centres and independently located edges, schoolbook u32 product oracles, signed
endpoints, row cancellation and immediate sink failure. Editor tests commit an
ellipse at the admitted coordinate limit, undo/redo exact recorded pixels and
cancel rectangle/ellipse preparation after a private prefix without owner/history
changes. 822 workspace tests pass (26 explicit ignores) across 71 suites; strict
workspace/all-target Clippy and release desktop/CLI builds pass. Targeted optimized geometry/history tests also pass, along with all 31 required
Metal integration tests, 23 renderer unit tests and 224 required-GPU desktop tests.
Default/direct-single-layer 30-frame native startup checks exit zero with empty logs.
The [validation record](validation/shape-preparation-2026-10-21/validation.json)
pins source, commands, compressed logs and release hashes for this correctness milestone. A test-oracle index-type compile diagnostic is retained;
its usize annotation is corrected in the validated source.

The wider numeric path and callbacks are correct baselines, without a throughput,
RSS or native latency claim. Historical timings remain scoped to their frozen
source. Other geometry kernels, whole-batch planning, aggregate budgets, native
cancellation/display tails, GPU recovery and broader features remain. T04/T13/T14/T37
remain partial/in progress; the full roadmap stays active.


## Forty-fourth slice: bounded cancellable stroke preparation

Pencil and smooth-stroke accounting now traverse borrowed segments/fixed-point
endpoints without copied path vectors. Vertex scans and invisible Bresenham work
observe cancellation. Round/square unsmoothed strokes and shape lines use a checked
once-per-stroke union whose row-map, run-vector and merge-heap payload capacities
share a provisional 64 MiB per-operation allowance. Chunk sorting/merging checks the
worker token, rather than sorting an entire large row without yielding. Exact
coverage, joint suppression and canonical output remain unchanged. Fixed scratch
exhaustion carries no decoded denial and cannot manufacture a history retry. See
[the stroke preparation contract](STROKE_PREPARATION.md).

Seven new tests cover independent pixel coverage, disordered multi-chunk union,
cancellation at every observed merge checkpoint, shared scratch exhaustion,
invisible path cancellation, wide/fixed-point path accounting, retraced half-alpha
coverage and owner/history rollback. Earlier cancellation exposed a native cache
cleanup dependency; staging now clears acquired disposable geometry on any error.
The existing regression passes. Development diagnostics are retained.

The [matched CPU-only comparison](../benchmarks/results/stroke-preparation-2026-10-21/DECISION.md)
retains 160 raw Criterion samples and 720 Hyperfine observations over three ordered
sessions, all exit zero. Baseline/candidate feature graphs are identical without
wgpu. Exact PCND archive/member output passes preflight and every session postflight.
The checked union is a required correctness baseline, **not a throughput win**:
square/round CLI batches take roughly 10–14% longer, with retrace overhead around
10–40% across sessions. Tiny/pencil cases are approximately tied; smooth/control
variation and RSS drift are retained. The policy/check intervals are provisional.
No native input/display/cancellation-tail or physical memory claim follows.

829 workspace tests pass (26 explicit ignores) across 73 suites. Strict all-target
Clippy, release desktop/CLI builds, 23 renderer unit tests, 31 required Metal
integration tests and 224 required-GPU desktop tests pass. Default/single-layer
30-frame native startup checks exit zero with empty logs. The subsequent test-only
raster module split preserves production/test bodies and passes raster unit tests;
all repository text files remain below 800 lines except Cargo's generated lockfile.
The [validation record](validation/stroke-preparation-2026-10-21/validation.json)
pins final and measured source identities, commands, hashes and compressed logs.

Reducing checked-union overhead, whole-batch/effect planning, aggregate service
budgets, other geometry/font kernels, physical pressure, asynchronous GPU services,
native device reconstruction/latency and broader feature work remain. T04/T13/T14/
T21/T37 remain partial/in progress; no roadmap gate is promoted.


## Forty-fifth slice: streamed text and cancellable resolved masks

Built-in bitmap text now streams clipped glyph spans into the existing bounded
paint plan. The visitor checks characters, glyph rows and emissions, including
newlines/offscreen glyphs; i128 origin/cell arithmetic narrows only clipped spans.
The admitted whole-pixel font, scale, character limits and emission order remain
unchanged. Legacy collectors remain for callers with their own allocation policy.
Resolved grayscale mask validation now has a checked counterpart; editor pixel
accounting and clipping also check bounded chunks, including offscreen spans.
No dependency, font choice, wire format or publication policy changes. See
[the text preparation contract](TEXT_PREPARATION.md).

Six new tests compare independent glyph-bit pixels and legacy span order over
scales/clips/origins, cancel every observed visitor/validator checkpoint, reject
unsupported offscreen glyphs, handle signed origins and stop on the first sink
failure. Editor tests cancel text and late mask traversals after private prefixes;
fixed metadata exhaustion and unsupported glyphs retain owner ticket/history and
release decoded leases. This completes another preparation prerequisite for
compound planning, not whole-batch reservation itself.

The frontend installed-font Cosmic Text/Swash path remains distinct: editable
objects, shaping/font-loading cancellation, IME, aggregate font/input memory and
native antialiasing/reference fidelity are not completed here. Callback intervals
and streamed geometry are correctness baselines, without measured throughput,
RSS or native input/display/cancellation-tail claims. Earlier timings remain scoped
to their frozen sources. Whole-batch/effect planning, aggregate resource budgets,
scheduling, asynchronous GPU services, device reconstruction and broader features
remain; T04/T13/T14/T37 stay partial/in progress.


835 workspace tests pass (26 explicit ignores) across 75 suites. Strict all-target
Clippy, optimized raster/editor text regressions, release desktop/CLI builds,
23 renderer unit tests, 31 required Metal integration tests and 224 required-GPU
desktop tests pass. Default/single-layer 30-frame native startup checks exit zero
with empty logs. The [validation record](validation/text-preparation-2026-10-21/validation.json)
pins committed source, commands, exact compressed logs and release hashes.


## Forty-sixth slice: logical paint reads before decoded replay

A bounded owned PaintProjection now exposes exact planned pixels without source
raster ownership or decoded tile allocation. Ordered descriptor/tile indexing is
shared with ordinary admitted replay; the latter retains its default/missing
proofs, private/weak COW normalization and staged limits. Projections keep all writes
because their source is supplied later. Reads preserve hidden RGB, straight-alpha
rounding and partial replacement coverage, observe cancellation through long
noncovering lists and return no partial pixel on source failure. Recovering the
original descriptors drops the read index without rebuilding geometry. The owning
planner must preserve captured bounds/source identity when replaying them. See
[the projection contract](PAINT_PROJECTION.md).

The existing flood classifier now accepts a fallible immutable pixel source.
Raster callers still use the same getter; logical sources can observe earlier
planned writes. Existing scratch/seed/clip/tolerance checks and scan work policies
remain. Six new tests cover unchanged decoded usage/weak versions, ordered chained
coverage, recovered admitted replay, fixed index quotas, bounds, every observed
construction/read cancellation checkpoint and source errors. An independent BFS
checks all 512 binary 3x3 inputs, nine seeds and four tolerances after two planned
writes: 18,432 cases.

841 workspace tests pass (26 explicit ignores) across 76 suites. Strict all-target
Clippy, 16 optimized projection/replay/flood tests, release desktop/CLI builds,
23 renderer unit tests, 31 required Metal integration tests and 224 required-GPU
desktop tests pass. Default/single-layer 30-frame startup checks exit zero with
empty logs. The [validation record](validation/paint-projection-2026-10-21/validation.json)
pins production/final benchmark source, exact compressed logs and release hashes.
An initial wrapper-extraction compile error and an incorrectly named optimized
test target are corrected and their exact diagnostics retained.

The editor still executes batches sequentially. Complete retained-program ownership,
layer duplication/deletion/expansion lineage, aggregate metadata/scratch leases,
decoded peak planning and upfront batch reservation remain. Per-plan payload and
work-count bounds are not physical RSS or native cancellation/input/display-tail
guarantees. No new dependency or wire/file policy is introduced; no roadmap gate
is promoted by this prerequisite.


The [real-library preparation comparison](../benchmarks/results/paint-projection-2026-10-21/DECISION.md)
retains 160 raw Criterion samples and 540 Hyperfine observations (480 workloads,
60 calibration), all exit zero, with exact pre/postflight pixels and region spans.
Tiny preparation is about 2.5x faster; wall process results are mixed/tied. Chaining
64 independent projections is 9–10x slower and default/no-op projection is over 4x
slower. All slow controls and session-specific RSS remain. Declared decoded charges
fall to zero for the projected path, while retained map/descriptor payload remains;
this is not an aggregate physical-memory claim. Sequential editor replay stays.
The next program implementation must avoid repeated per-pixel source-chain lookups
and preserve context-specific no-op proofs, then qualify integrated batch behavior.


## Forty-seventh slice: one bounded combined logical paint program

Logical projections can now consume and append further ordered PaintPlans using
one incremental tile index. A read reaches its preceding source once and applies
that tile's descriptors in original numerical order. The receiver's fixed metadata
policy counts all retained descriptor/map/write-index capacities plus the incoming
descriptor allocation until its transfer iterator drops. Growth is fallible and
checks actual capacities. Append consumes both inputs, so failure cannot expose a
partially updated program. No decoded pixels are requested or owned by append.
See [the combined-program contract](PAINT_PROGRAM.md).

Five new tests cover 64 ordered source-over/partial-coverage operations, exact
recovered replay and one source read; aggregate and incoming-transfer denials;
cancellation at every observed append checkpoint; zero-prefix two-operation replay
quota failure; and independent BFS over 18,432 small combined image/seed/tolerance
cases. The complete recovered paint plan admits its known tile set before decoded
allocation. This per-program preparation does not front-load an editor batch.

846 workspace tests pass (26 explicit ignores) across 77 suites. Strict all-target
Clippy, 21 optimized program/projection/replay/flood tests, release desktop/CLI
builds, 23 renderer unit tests, 31 required Metal integration tests and 224
required-GPU desktop tests pass. Default/single-layer 30-frame native startup
checks exit zero with empty logs. The [validation record](validation/paint-program-2026-10-22/validation.json)
pins production/final measured source, exact compressed logs and release hashes.

The [three-way real-library comparison](../benchmarks/results/paint-program-2026-10-22/DECISION.md)
retains 240 raw statistical samples and 780 Hyperfine observations, including 60
calibration observations, all exit zero. Exact all-pixel and canonical region
pre/postflights surround three ordered process sessions. For 64 plans, combined is
13.94–15.58x faster than chained and 1.38–1.57x faster than materialized preparation;
retained declared metadata falls from 19,968 to 3,192 bytes. Tiny gains retain
one-plan drift controls. Wall statistical/process disagreement and an approximately
4.4x no-op regression reject a global editor activation. Per-session RSS remains
separate from declared payload; no native latency or physical-memory claim follows.

The editor remains sequential. Cross-layer/program ownership, duplication/deletion/
expansion lineage, contextual uniform proofs, aggregate scratch/service leases,
decoded peak planning and guarded batch replay/error attribution remain. T04/T11/
T13/T21/T37 stay partial/in progress; the full roadmap remains active.
