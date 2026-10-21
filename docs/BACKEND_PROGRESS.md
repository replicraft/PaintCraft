# Backend engineering progress

The [initial audit and slices 1 through 14](backend-progress/initial-through-fourteen.md)
retain the compositor, history, native preparation and early benchmark evidence.
This file continues the implementation ledger from shared admission onward.

## Fifteenth slice: shared in-flight worker admission

Native edit/render pools now use one process-local declared-byte authority while
retaining local queue/byte caps. Runtime submission reserves atomically, and shared
lease guards keep a running worker or unread result charged after executor drop.
The controller holds observable contention in its edit queue; rendering retries a
feasible contended request instead of caching an error. Failed admission never
publishes a partial transaction.

625 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits and release desktop/CLI builds. Runtime tests cover
heterogeneous and racing pools, overflow, local rejection, cancelled/panicking workers,
executor drop and output destruction ordering. The desktop contention regression
retains a completed stroke through worker completion and unread-result admission,
then checks one publication and exact displayed pixels. Required Metal lifetime/
parity checks and a 30-frame isolated native smoke pass.

[The admission contract](WORKER_ADMISSION.md) explicitly limits the 3 GiB ceiling
to declared in-flight charges. This does not establish global decoded/native/GPU
allocation bounds, thread priorities, fairness, reclamation or latency. No new
dependency or performance improvement is claimed. Persistent physical ownership,
all-service admission and native pressure/interference qualification remain;
T13 stays partial.


## Sixteenth slice: ownership-based decoded tile residency

Every decoded tile now owns a shared byte lease until its Vec payload and final
reader are released. Standard native/headless admissions use one default 2 GiB
payload realm, covering fresh tiles, COW, imports, previews and cold restoration.
Weak cache/history tokens do not retain capacity. The byte primitive moves to
core types for actual raster/runtime consumers; runtime exports remain compatible.
Private execution also accepts an explicit admission realm. Pixel and wire policy
remain unchanged.

633 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits and release desktop/CLI builds. New regressions
cover reader-pinned capacity, failed COW, unshared weak-token transfer, decode and
partial-import rollback, racing allocations, shared defaults and atomic failure at
the second batch operation. A real history compaction/reader/undo/redo case checks
exact live payload counters and independent pixels. Metal parity/lifetime tests and
a 30-frame isolated native smoke pass.

[The residency contract](RASTER_RESIDENCY.md) distinguishes payload ownership from
worker declarations and process RSS. The fixed ceiling is a provisional guardrail;
metadata, codec/encoded/output/GPU buffers, adaptive admission and cross-tab pressure
handling remain. No allocation-throughput or RSS improvement is inferred from the
correctness checks. T04/T11/T13 remain partial.


## Seventeenth slice: guarded owner pressure maintenance

Detached preparation can now pack/refine/spill a bounded preserving history step
and return a typed storage-maintenance result. Owner publication changes no content
revision, cursor, redo or saved acknowledgement, but retires older private storage
preparations. Required spill barriers and cancellation complete before publication.
Native pressure coordination polls inactive tabs and parked frontend workspaces
first; clean maintenance stays separate from edits and yields to queued foreground
work. Resource-failed render keys retry when tile residency drops.

641 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Metal checks and
an isolated 30-frame native smoke. New regressions cover delayed ownership release,
identity/save preservation, all observed cancellation boundaries, failed durability,
staleness, clean/no-progress behavior, inactive/parked ownership, foreground yielding
and unchanged-key render retry. Pressure telemetry is scaled/injected in coordinator
tests; there is no native 2 GiB exhaustion or event/frame latency qualification.

[The pressure contract](HISTORY_PRESSURE.md) records provisional 64 MiB headroom,
64-item work batches and retry timing. Request-sized admission, content retry,
cross-window signaling, pinned current/read data, all-service physical budgets and
native pressure/interference/tail-latency evidence remain. T04/T11/T13 stay partial;
no throughput or RSS improvement is inferred from these correctness checks.


## Eighteenth slice: bounded budget wake subscriptions

Byte budgets now offer weak, bounded, coalesced wake subscriptions. Pressure entry,
feasible denial and relevant releases notify consumers outside the counter mutex;
callback/capture destruction also occurs after unlocking. Unregistration, callback
panics and concurrent release events retain counter ownership guarantees. Native
contexts subscribe to shared tile and worker realms so another owner's pressure
or returned capacity can trigger a fresh local poll.

650 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits, release desktop/CLI builds, three required Metal
checks and a 30-frame isolated native startup/shutdown smoke. Six core regressions
cover coalescing, denial, initial pressure, weak/bounded fanout, callback reentrancy/
panics and concurrent unregister/release. Three independent-context regressions
check wake routing without edits, unread-result retention and above-threshold
release interest. These use real counters/egui callbacks, not multiple OS windows.

[The notification contract](BUDGET_NOTIFICATIONS.md) records the 64-registration
bound, wake-only callback rule and remaining qualification. Request-sized admission,
content retry, native multi-window/suspension behavior, all-service physical budgets,
fairness and measured callback/input/frame latency remain. No throughput or RSS
gain follows from these correctness checks. T04/T11/T13 stay partial.


## Nineteenth slice: footprint-aware decoded allocation retries

Resource errors now retain internal typed realm/demand context without changing
error/v1 bytes, display or legacy equality. Raster admissions track live and peak
ownership for their attempt, so partial rollback cannot suggest that a single free
tile is enough to rerun a larger known prefix. Native render retries require a
feasible decoded demand and enough currently free capacity in that exact realm;
reclamation before error delivery is observed too. Fixed work/staged limits and
impossible footprints do not create release-driven retry loops.

657 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Metal checks and
a 30-frame isolated native smoke. Regressions cover live/peak demand, rollback,
actual atomic batch retry after external release, legacy wire errors and native
same-key rendering. These are correctness checks, not timing/RSS evidence.

The known requirement describes an already observed serial prefix, not future
operations or an upfront reservation. Request-sized admission, content retry,
demand-aware pressure coordination, logical blob authority, all-service budgets
and native latency/interference qualification remain. T04/T11/T13 stay partial.


## Twentieth slice: shared known-demand pressure coordination

A budget watcher can publish one feasible known allocation demand for its exact
realm. Demand is weakly owned and bounded by the existing 64 registrations; it
reserves no bytes. Pressure observations take the maximum live demand rather than
summing concurrent attempts. Explicit changes wake consumers, and entry/release
notifications include this larger threshold. Unchanged interest stays silent.
Dropped watchers clear demand even if a collected callback retains their observer.
Counter/observer snapshots and callback destruction retain outside-lock guarantees.

Native render failures publish their decoded demand. Every subscribed context can
then reclaim its own eligible recorded history above the baseline free-space
threshold. Success, a changed document/preview key or shutdown removes interest;
foreign allocation realms cannot trigger that budget's maintenance. Existing
owner/publication/foreground-yield guards remain unchanged.

662 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Apple M1 Pro
Metal checks and an isolated 30-frame native smoke. Three core regressions cover
maximum/exact-realm demand, withdrawal/unregister, concurrent ownership and wakes
above static headroom. Two desktop regressions cover independent-context history
maintenance and stale/foreign interest. Counters are scaled/injected; history work,
publication and reader release are real. These are not native exhaustion or latency
measurements, and the additional observer scans have not been benchmark-qualified.

Known-prefix demand remains advisory. Future operations may need more; racing
consumers still reserve normally. Request-sized pre-reservation, automatic retry
of accepted content edits, current-document blob eviction, all-service physical
budgets, fairness and native pressure/interference qualification remain. T04/T11/T13
stay partial; no throughput, RSS or tail-latency improvement is claimed.


## Twenty-first slice: reserved decoded attempts and accepted-edit retries

Shared capacity can now be reserved upfront and transferred into individual leases.
A live reservation recycles temporary resource releases; its final owner returns
unused capacity, while surviving tiles remain independently charged. Raster attempts
use this allowance without borrowing extra free bytes, and retain cumulative staged
limits. Detached edit preparation accepts an explicit admission while keeping all
history/cancellation/publication guards. Cheap owner/base tickets avoid collecting
history metadata for retry checks.

Successfully submitted native edits retain their validated batches after feasible
decoded failures. They keep their preview, queue accounting, dirty state and FIFO
position ahead of later file boundaries. Readiness atomically reserves the known
footprint before fresh preparation; worker-admission interference retains the command
and releases unused capacity. Escape cancels waiting/later editing activations;
a changed owner/base refuses rebasing. Permanent staged/work limits and impossible
requirements remain terminal. Waiting owners can reclaim eligible own history even
with a later file boundary queued. Render and tab/workspace edit demands share one
bounded observer per context.

679 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Apple M1 Pro
Metal checks and an isolated 30-frame native smoke. Four core tests cover transfer,
recycling, clone/final-reader lifetimes, release wakes and concurrent reservation
teardown. Two raster and four editor regressions verify exact counters, scratch
reuse, rollback, staged policy, independent pixels/receipts, cancellation and stale
publication. Six controller regressions cover accepted command ordering, preview,
Escape, permanent limits, own-history cleanup and shared-worker interference. A
cross-context integration uses one actual four-tile realm and verifies that eligible
history reclamation enables the other editor's accepted two-tile stroke without
changing the history owner's ticket/entries. See [the admission contract](EDIT_ADMISSION.md).

The first attempt still allocates incrementally; a retry reserves only its learned
prefix demand, which may grow. Initial worker-submission races, navigation and other
services do not yet have the full retained retry lifecycle. Command/preview vectors,
metadata, codec/output/GPU resources and physical/RSS admission remain outside this
reservation. Current readers can pin capacity indefinitely; fairness, native memory
pressure and queue-inclusive input latency remain unqualified. No throughput or RSS
gain is claimed. T04/T11/T13 remain partial.


## Twenty-second slice: lossless initial worker-admission rejection

The runtime offers retained submission: admission failure returns the original task
and wake closures without executing either or inserting/charging a job. Ordinary
submission keeps its existing error-only interface. Successful work keeps the same
cancellation, targeted completion and unread-result reservation lifecycle.

Native initial batches, captured tool plans and history navigation now retain their
task when shared worker capacity disappears between readiness and submission. They
remain pending at the same FIFO position, including previews, alias routing and file
boundaries. No worker charge exists before actual admission. Escape, controller drop
or a changed owner/base discards the unexecuted task; later commands cannot pass it.
Own storage maintenance waits rather than invalidating its captured preparation.
Decoded-failure retry remains the separate reserved-capacity path.

687 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Apple M1 Pro
Metal checks and an isolated 30-frame native smoke.

Three runtime regressions force a shared-capacity race, local rejection/capture
ownership and overflow/impossible requests, then reuse the original closures. Five
controller regressions cover alias/FIFO publication, retained tool previews, Escape/
drop, navigation ordering/cancellation and stale-base refusal. They use real worker
and shared byte admission without allocating the declared GiB charge physically.

Retained preparation can pin decoded/history readers and copies metadata before
submission. That metadata, other resource classes, full frozen resources, first-
attempt whole-request planning, decoded history-navigation retry, scheduling fairness
and native latency/physical-pressure qualification remain incomplete. No throughput,
RSS or tail-latency gain is inferred. T11/T13 stay partial.


## Twenty-third slice: admitted history restoration and ordered navigation retries

Recorded undo/redo/seek restoration accepts explicit decoded admission all the way
through packed snapshots to tile materialization. Configured cumulative restore
limits still apply. Hot/weakly resident versions can reuse existing allocations,
including successful fully resident navigation with zero fresh capacity. Failed
partial decodes release private tiles and retire dead weak aliases without moving
cursor, redo, content or saved acknowledgement.

Native deferred requests now retain batch edits or exact recorded navigation intent.
Feasible decoded failure returns the request without a private history fork; the
owner waits in FIFO order and reserves known capacity before fresh preparation.
Seek tickets, owner/base checks, cancellation, status and target repair remain intact.
Permanent restore/work caps and impossible decoded demands do not register a wait.
Recorded restoration never replays the original editing commands.

A controller test exposed a separate double-poll race: a maintenance completion
could arrive between the coordinator's poll and start, then be consumed implicitly
inside start and immediately replaced by another cleanup despite queued paint.
Start now leaves completion consumption to the explicit coordinator poll, so the
yield decision observes every completed step.

697 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Apple M1 Pro
Metal checks and an isolated 30-frame native smoke.

Four editor regressions cover cold partial-decode rollback and reserved retry,
independent pixels/state/history identity, undo/redo reuse, fixed restore policy,
cancellation during decoding, and cancelled/stale publication. Five native
regressions cover cold seek/save FIFO ordering, Escape, stale bases, cold Undo and
Redo, and permanent limits. A completion-ready test verifies that starting again
cannot consume an unread maintenance result behind the coordinator's yield decision.

First-attempt full-request planning, logical current-document eviction, encoded/
metadata/codec/output/GPU admission, priority/fairness and native latency/physical-
pressure qualification remain incomplete. Known-prefix demand may grow on later
operations. Reader-pinned capacity can require cancellation. T07/T11/T13 remain
partial; no throughput, RSS or tail-latency gain is inferred from these checks.


## Twenty-fourth slice: planned history decode admission and demand repricing

Cold restoration now captures a cancellable unique-blob plan, pins reusable tile
leases and reserves the complete missing-tile set before decoding. A last external
reader cannot invalidate planned reuse; duplicate references share one allocation.
Fixed restore allowances are validated before decoded admission without double
charging staged pixels. Explicit reservations are checked rather than expanded.
Failed full-set admission allocates no missing-tile prefix. Codec/cancellation
failures still roll back private materialization and dead weak aliases.

Native navigation plans on its worker for every attempt instead of reserving an
outdated learned prefix. Waiting requests reprice after decoded budget activity;
release/reallocation to identical counters is detected. Denials, notification
feedback, zero charges and pool-internal recycling do not manufacture activity.
A provisional five-second fallback catches other-realm cache warming. Saturated
activity stamps conservatively request a recheck. Queue/owner/save guards remain.

704 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Apple M1 Pro
Metal checks and an isolated 30-frame native smoke.

Two core regressions verify nontransferring capacity checks and activity semantics,
including identical-counter replacement and saturation. A raster regression checks
full-set admission, staged accounting and strict explicit reservations. Two history
regressions check shared identity/hidden RGBA and reader-drop reuse pinning. An
editor regression denies a cold plan in a fresh realm and verifies no prefix
reservation high-water increase. A native regression warms a target in another realm
and restores it while the original realm remains full, using activity and periodic
rechecks; unchanged-denial polls do not submit new work.

General batch/effect planning, bounded plan/command/index metadata, logical current-
document eviction, all-service physical budgets, scheduling fairness and measured
native pressure/latency remain open. Concurrent warming may make a plan conservative
until its next recheck. Planning metadata can fail before decoded admission; these
are correctness/admission checks, not throughput/RSS claims. T07/T11/T13 stay partial.


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
