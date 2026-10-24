# Backend engineering progress

The [initial audit and slices 1 through 14](backend-progress/initial-through-fourteen.md),
[slices 15 through 24](backend-progress/fifteen-through-twenty-four.md),
[slices 25 through 34](backend-progress/twenty-five-through-thirty-four.md), and
[slices 35 through 44](backend-progress/thirty-five-through-forty-four.md) retain
completed backend work and its scoped evidence. This file continues from streamed
text preparation onward.

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


The [real-library preparation comparison](../docs/benchmarks/paint-projection-2026-10-21/DECISION.md)
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

The [three-way real-library comparison](../docs/benchmarks/paint-program-2026-10-22/DECISION.md)
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


## Forty-eighth slice: shared owned paint metadata admission

Paint descriptors and indices now use the existing SharedBudget authority in a
provisional 256 MiB process-default realm, separate from decoded tiles and fixed
per-plan/program policies. A unique resizable BudgetAccount admits growth atomically
and releases after storage drops; index guards survive replay/projection ownership.
PaintPlan clone shares immutable descriptors and one charge. Mutation makes a
fallible separately admitted copy, preserving exact capacity rather than requesting
unnecessary amortized spare capacity. Custom admission realms propagate through
all existing editor paint factories. See [the shared metadata contract](SHARED_PAINT_METADATA.md).

Thirteen new tests cover account resize/contention/reentrant release order, shared
clones and COW denial, index construction/recovery/cancellation, cross-realm append,
clamped-capacity copies, cloned admissions, private-prefix editor rollback under
metadata interference and native retry isolation. A fixed local quota carries no
allocation denial; shared pressure identifies PaintMetadata and cannot trigger
decoded-history reclamation. Per-account demand remains advisory, without complete
attempt pricing or native metadata waiting/repricing. Minimum growth is admitted
before allocation and actual usable capacity validated; rejected private growth
frees storage before releasing its declaration. Allocator/hash/Arc controls and
transient reallocations remain outside retained payload accounting.

859 workspace tests pass (26 explicit ignores) across 79 suites. Strict all-target
Clippy, 37 optimized core tests, 25 optimized raster ownership/program/projection/
replay tests, the optimized editor interference regression, release desktop/CLI
builds, 23 renderer unit tests, 31 required Metal integration tests and 225
required-GPU desktop tests pass. Default/single-layer 30-frame native startup checks
exit zero with empty logs. The [validation record](validation/shared-paint-metadata-2026-10-22/validation.json)
pins final source, commands, exact compressed logs and release hashes. The specific
clamped-capacity regression is native 64-bit; other target qualification is separate.

The [matched overhead comparison](../docs/benchmarks/shared-paint-metadata-2026-10-22/DECISION.md)
retains 480 statistical samples and 1,200 original Hyperfine observations with
identical CPU-only CLI feature graphs and canonical outputs. Most CLI cases are
near tied/mixed, but original eraser session 2 retains 48.9% mean overhead, a broad
median slowdown and a 115.28 ms charged maximum. Three separately labeled focused
follow-ups add 240 observations, with +4.75%, -3.87% and -0.32% mean changes. The
large slowdown did not repeat; its cause remains unproven and no original sample
is removed. Combined tiny has 8–22% process overhead; other apparent gains and tier
disagreements do not establish a general speedup. All 1,440 process observations,
calibration, per-session RSS and negative controls remain. Shared accounting is a
correctness baseline, not a throughput optimization or measured capacity optimum.

The editor remains sequential. Full batch/source-lineage planning, metadata
waiting/fairness, aggregate geometry/font/flood scratch, allocator/physical pressure,
native input/display/cancellation tails and broader feature work remain. No roadmap
gate is promoted by declared ownership or isolated measurements.


## Forty-ninth slice: scoped metadata demand and native waiting

A paint preparation attempt now shares live-plus-pending metadata demand across
its independently owned descriptor/index accounts. A failed growth reports the
known attempt peak, and clones of the factory retain no capacity. Scope locks
are released before budget callbacks; releasing one owner does not pin the others.
This is observed demand rather than an upfront full-batch or physical-memory plan.

Feasible native metadata failures now retain the original captured edit/navigation
request, FIFO position, preview, base ticket and file barriers. Fresh retries keep
both original resource realms across service replacement. Escape and stale bases
retire the request; impossible/fixed limits remain terminal atomic failures. There
is no metadata reservation guarantee: another consumer may consume headroom and
defer the same request again. A provisional periodic probe can reprice demand,
while failed-attempt cleanup alone cannot spin a known unmet request.

Metadata and decoded watchers now update independently. Metadata-only demand wakes
edit/render consumers without advertising decoded pressure or bypassing a file
barrier to reclaim resident history. Rendering retries preserve current view and
preview identity. The coordinator owns release-interest updates, so successful
GPU presentation cannot clear another consumer's wait. See the
[contract](METADATA_WAITING.md) and
[source-pinned validation](validation/metadata-waiting-2026-10-22/validation.json).

872 workspace tests pass (26 explicit ignores), including thirteen new regressions;
40 optimized core and 26 optimized raster tests pass. Required Metal validation
passes 23 renderer unit and 31 integration tests, and required-GPU desktop validation
passes 234 tests (eight explicit ignores). Strict workspace/all-target Clippy,
release desktop/CLI builds and isolated default/single-layer 30-frame native
startup smokes pass. Initial visibility errors and an unused-mut warning remain
archived separately from final successful validation.

The [matched overhead comparison](../docs/benchmarks/metadata-waiting-2026-10-22/DECISION.md)
retains 480 statistical samples and 1,200 original process observations, exact
canonical/library outputs and unchanged CPU-only dependency graphs. Projected
tiny is about 7% slower statistically; tiny process medians are slower in all
sessions. Mixed CLI controls, a slow original eraser baseline and long library
baseline/candidate observations remain. Scope ownership/waiting is a correctness
adoption, not a general speedup or native event-latency claim.

Whole-batch/source-lineage planning, atomic metadata reservations, aggregate
geometry/font/flood/service budgets, physical memory accounting, cross-window
fairness and sustained native input/display/cancellation tails remain open.
T04/T11/T13/T14 remain partial; no wire/file policy or dependency changes.


## Fiftieth slice: reserved metadata retry admission

A BudgetReservation can now transfer its capacity into a resizable BudgetScope.
Descriptor/index accounts atomically consume that capacity and recycle freed
bytes while reservation owners survive. Accounts hold weak reservation links;
a retained plan/projection does not pin unused attempt capacity after its factories
and other reservation owners drop. Surviving storage keeps only its own charge,
and subsequent growth uses ordinary shared admission. Lease and account consumers
share one capacity counter. No lock crosses a global callback.

Native metadata retries now reserve known demand before worker dispatch. Existing
FIFO/preview/file barriers and original realms survive. Another consumer cannot
take reserved capacity; a larger discovered demand releases the old attempt and
re-defers the same captured request. A bounded periodic live probe can still
reprice stale demand when the old requirement cannot be reserved. Fixed/impossible
limits remain terminal. See [the ownership contract](RESERVED_PAINT_METADATA.md).

Six core regressions cover strict bounds, transfer/recycling, sibling and lease
competition, callback reentrancy, concurrent contention and final owner teardown.
Three raster regressions cover fully occupied global headroom, insufficient
reservation before decoded allocation, projection retention/recovery and cancelled
factory reuse. Two native regressions execute captured preparation under competing
admission and preserve request/base identity through a larger-demand retry.

883 workspace tests pass (26 explicit ignores) across 80 suites. Strict all-target
Clippy, 46 optimized core tests, 29 optimized raster tests, 23 renderer unit and
31 required Metal integration tests, 236 required-GPU desktop tests (eight explicit
ignores), release desktop/CLI builds and isolated default/single-layer 30-frame
startup checks pass. [Source-pinned validation](validation/reserved-paint-metadata-2026-10-22/validation.json)
retains exact logs/hashes. The initial sandbox run exposed no Metal adapter; its
eleven GPU-dependent failures remain alongside the successful native rerun. A
development callback test's missing release-interest/rearm setup is corrected
and its original failed log preserved.

This is declared ownership correctness. Earlier scope timing remains pinned to
its measured source and does not qualify this change's overhead or native latency.
Whole-batch/source-lineage planning and contextual no-op proofs, joint tile/metadata
reservations, aggregate geometry/font/flood/service budgets, physical accounting,
scheduling fairness and native input/display/cancellation tails remain open.
No roadmap gate, file/wire policy or rendering default changes.


## Fifty-first slice: source-bound logical paint lineage

PaintSnapshot captures an owned sparse input and its decoded leases; PaintOverlay
appends logical writes and freezes immutable branch prefixes without decoded
replay. Forks retain exact source pixels across sibling edits and external COW.
Expansion reveals the implicit default, including hidden RGB, without making old
out-of-bounds descriptors visible. The bound API exposes neither a replacement
source callback nor a detached replay plan. Arbitrary-source projections retain
their existing behavior. See [the lineage contract](PAINT_LINEAGE.md).

Captured missing/default proofs omit contextual no-ops and allow million-square
uniform views without a tile-grid walk or reader scratch. Other readers prepare
one bounded reference vector and evaluate ancestry iteratively. Source nodes,
source-map payload, programs/indices and reader scratch carry shared/reserved
metadata accounts. Input decoded buffers remain ordinary pinned reader leases.
Deep teardown consumes ancestors iteratively, including concurrent release; it
does not rely on recursive Arc destruction.

Ten regressions cover source/branch identity, clipping/defaults after expansion,
independent coverage/source-over pixels, external COW, huge no-op proofs, metadata
denials/reservations, reentrant source release, cancellation and 4,096-node reads
and teardown on a small stack. An independent BFS checks 18,432 flood cases.
The first broad checks passed before review strengthened concurrent teardown;
final validation is repeated against the resulting source. Development visibility
errors and test type/borrow diagnostics remain separate from final validation.

893 workspace tests pass (26 explicit ignores) across 81 suites. Strict all-target
Clippy, 39 optimized raster tests, 23 renderer unit and 31 required Metal integration
tests, 236 required-GPU desktop tests (eight explicit ignores), release desktop/CLI
builds and isolated default/single-layer 30-frame startup checks pass.
[Validation](validation/paint-lineage-2026-10-22/validation.json) pins final source,
commands, exact logs and release hashes.

The [matched three-way comparison](../docs/benchmarks/paint-lineage-2026-10-22/DECISION.md)
retains 240 statistical samples, 780 original process observations and exact pixel/
region postflights. Bound default/no-op process preparation is 3.71–3.91x faster
than combined but 13.5–21.1% slower than materialized. Bound tiny is 11.5–16.3%
slower than combined; wall retains roughly 16% statistical overhead and two slow
process sessions. Noisy wall tails, overlap tier disagreement, calibration and
RSS remain. Source/reader ownership adds declared payload in painted cases;
physical budgets, branching-chain/native latency and editor dispatch are not qualified.

This remains a preparation primitive. The editor still replays sequentially;
complete batch compilation needs structural integration, contextual tile-ownership
and decoded peak/cumulative simulation, source guards and operation attribution.
Joint resource reservations, aggregate geometry/font/flood/service budgets, physical
accounting, scheduling fairness, native latency and the broader roadmap remain open.
No dependency, wire/file policy or rendering default changes.


## Fifty-second slice: complete built-in batch preparation

The editor compiles all current built-in commands against a structural shadow
document before allocating decoded output. Ordered plans carry resolved layer
identity and original operation attribution. Flood reads source-bound logical
paint, including frozen duplicate branches and pre-expansion bounds. Write-only
batches omit logical source views. See [the batch contract](BATCH_PLANNING.md).

A cancellable sparse tile-ownership model tracks input COW, private reuse, shared
duplicates, expansion and deletion. It separately prices live/peak output versions
and cumulative allocation work. The editor releases source/model storage, reserves
the complete known new decoded peak once and replays original plans/structural
operations from the captured base. Decoded quota denial therefore allocates no
private pixel prefix. Ordinary replay allocation/metadata failures and caller
interference still roll back private work; recorded history and publication remain
owner guarded. This is not a joint metadata or physical-memory reservation.

Source-pinned differential checks cover every painting family and 64 mixed
structural sequences. New tests exercise model ownership, fallible source capture,
zero-prefix quota denial, recycled peak versus cumulative limits, every observed
cancellation checkpoint and write-only logical-view elision. Native retry tests
retain distinct metadata/decoded policies and real replay interference checks.

908 workspace tests pass (26 explicit ignores, 83 suites). Strict all-target
Clippy, 122 optimized raster tests, 118 optimized editor tests, 54 required-Metal
renderer tests, 236 required-GPU desktop tests (eight ignores), release builds
and default/single-layer isolated 30-frame startup checks pass.
[Validation](validation/batch-preparation-2026-10-22/validation.json) pins production
source and logs. A later benchmark-only semantic verifier change reran Clippy and
benchmark compilation. Original development failures and earlier validation remain.

The [completed comparison](../docs/benchmarks/batch-preparation-2026-10-22/DECISION.md)
retains 280 statistical samples and 1,860 original process observations. Exact
library/CLI semantics, complete canonical archives, decoded peaks and matched
CPU-only feature graphs pass. Diagonal library execution is about 1.56x slower
statistically and 56.2–68.1% slower in processes; mixed lineage is about 3.38x
slower statistically and 154.6–227.3% slower in processes. Diagonal metadata peak
rises from 49,432 to 4,111,632 bytes, with increased process RSS. Small/no-op/dense
controls, overlapping eraser intervals, startup drift, CPU costs and tails remain.
No performance improvement is claimed. Retained geometry, redundant preparation
and source-reading costs require further optimization without relaxing guarantees.

Complete preparation now applies to the current built-in grammar, replacing the
previous sequential-preparation scope. Joint service/geometry/font/history/GPU
budgets, physical accounting, scheduling fairness, native latency, future command
families and the broader roadmap remain open. No dependency, wire/file policy or
rendering default changes.


## Fifty-third slice: shared immutable batch descriptors

The compiler now shares exact consecutive descriptor payloads while retaining
every original operation, blend, source update and replay record. Comparison
checks coverage/color/policy/order in cancellable chunks and uses the same
admission realm. Larger previous capacities are not imported.
[The contract](PAINT_PLAN_SHARING.md) records the limits and resource evidence.

The original release fixtures keep exact semantic hashes and decoded peaks.
Repeated-diagonal declared metadata peak drops from 4,111,632 to 97,552 bytes;
smooth brush/eraser drop from 265,488 to 14,608 bytes. Mixed lineage and default
controls are unchanged. These counters do not establish RSS or timing improvements.

[Validation](validation/paint-plan-sharing-2026-10-22/validation.json) retains 911
workspace passes (26 ignores, 84 suites), strict all-target Clippy, 124 optimized
raster and 119 optimized editor tests, and release benchmark/CPU CLI builds.
A final test-only duplicate/flood addition then passes in the nine-case batch suite
in debug/release and strict Clippy; production code is unchanged.
Before/shared repeated and varied-miss preflights match canonical/library output.
The [matched follow-up](../docs/benchmarks/plan-sharing-2026-10-22/DECISION.md)
retains 480 statistical samples and 3,120 original process observations. Diagonal
library RSS and wall time improve, but mixed sessions, large slow control tails
and startup drift preclude a general speed claim. Mixed-source optimization,
aggregate/physical admission and native latency remain open. No dependency,
wire/file policy or rendering default changes.


## Fifty-fourth slice: conservative projected-write coverage

Logical programs now skip tile lookup/descriptor scans outside a clipped write
envelope while preserving preceding-source calls/errors, layer bounds and
cancellation. Pixel-oracle and long-list/fast-miss tests pass.
[The measured decision](../docs/benchmarks/projection-coverage-2026-10-22/DECISION.md)
retains 480 statistical samples and 3,120 original process observations with exact
all-checkpoint canonical/library output. Mixed flood/duplicate library execution
improves 2.65x statistically and 53.4–69.7% in processes; actual CPU CLI improves
10.1–11.0%. Declared metadata rises by 520 bytes; decoded peak is unchanged.
Unrelated controls retain large slow tails and opposing sessions. No general
throughput, physical-memory or native latency claim follows.

914 workspace tests pass (26 ignores, 85 suites), along with strict all-target
Clippy, 126 optimized raster and 120 optimized editor tests, and release CPU CLI/
benchmark builds. [Validation](validation/projection-coverage-2026-10-22/validation.json)
pins commands and original logs. The next active work returns to Classic UI;
remaining performance scope stays recorded without starting another experiment.
