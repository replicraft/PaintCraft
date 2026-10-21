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
