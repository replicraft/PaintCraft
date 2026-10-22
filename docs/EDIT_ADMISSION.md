# Decoded capacity reservations and ordered retries

Native edits and history navigation that fail a feasible decoded-tile allocation
retain their captured request at the front of their owner's editing queue. They do not publish
partial pixels, drop history or allow a save boundary to pass them. Their preview,
painting queue accounting and dirty/pending status remain active. Escape cancels
the waiting edit and later editing activations while preserving file barriers.

## Reservation ownership

`SharedBudget::reserve_capacity(bytes)` admits a bounded `BudgetReservation` up
front. A reservation transfers bytes into independent `BudgetLease` values without
competing for shared capacity again. Its clones share one allowance; requests
beyond that allowance cannot borrow other free bytes. A failed transfer reports
known live-plus-request demand in the original realm.

While a reservation owner lives, a resource's last lease recycles its bytes into
that reservation. It does not announce globally free capacity. Dropping the last
reservation owner returns unused capacity immediately. Surviving resource leases
then return only their own bytes on final release. Weak links from leases do not
pin the reservation or its unused capacity. Final-owner/reader races preserve the
shared count, and release callbacks run outside reservation/counter locks.

Reserved headroom counts against `BudgetUsage.reserved` even before vectors are
allocated. These are admission counters, not process RSS or exclusively live Vec
bytes. The same decoded realm covers both reserved capacity and surviving tiles;
transfer does not add another charge. Zero-byte reservations own no capacity.

`Admission::with_reserved_residency` supplies a raster attempt with that reservation.
Temporary tiles can recycle capacity, while the separate cumulative staged-pixel
limit continues to apply. Tile leases follow COW/read/history ownership as before.
Unused capacity returns when the attempt ends; retained tiles remain charged.

## Initial worker admission

The native worker readiness check is advisory. If another pool takes shared
capacity before submission, the runtime returns the original task intact. The
controller retains its captured batch/tool plan/navigation preparation, description
and target-alias handling, without moving later actions or file barriers ahead.
No job handle or worker byte charge exists until actual admission succeeds.

Each controller has at most one such captured initial task. It remains pending/
dirty and participates in preview/painting limits. Escape or controller destruction
drops it without executing it. A changed owner/base cancels it instead of capturing
a replacement base; a changed resource policy still fails normal publication guards.
Own history maintenance waits while that captured preparation is retained. After
submission, normal worker cancellation, result ownership and publication apply.

This retains existing resource references, not a complete frozen font/profile
snapshot. Captured private metadata and decoded readers may stay pinned while the
task waits; their all-service admission and scheduling fairness remain open.

## Retry and publication

Native batch execution now [plans painting operations](PAINT_PLANNING.md) before
materializing their touched tiles; the complete batch still executes sequentially. Worker-side stroke/text plan
construction produces its batch once. If execution reports a feasible decoded
denial, the result retains that batch and the captured owner/base ticket; it does
not retain a private document/history fork. Tool plans and font masks are not
rebuilt from changed UI state when retrying.

When sufficient capacity and the shared editing worker are available, the owner
atomically reserves the known footprint, captures a fresh private preparation and
resubmits the retained batch. A racing tile or worker admission keeps the command
waiting and releases unused capacity. A later allocation can reveal a larger
footprint, in which case the updated demand waits again. The known requirement is
an observed prefix, not a full prediction of every later operation.

A changed owner/base cancels the retained command rather than rebasing it. Fresh
preparation after storage-only maintenance uses its new storage representation;
normal cancellation, resource-policy and publication guards still apply. Success
publishes once and unblocks the next queued action/file boundary. Impossible
footprints and fixed staged/work limits remain terminal errors. Native allocator,
codec, spill and other unclassified failures do not become decoded-capacity waits.

Waiting edits publish known demands through their App's existing bounded budget
watch, including inactive tabs and parked workspace ownership. The maximum demand
across renders and editors guides preserving maintenance in subscribed contexts.
A waiting owner may maintain its own history even when a later file boundary is
queued; that boundary still captures only after the edit succeeds or is cancelled.

## Recorded history restoration

`History::{undo,redo,seek}_with_admission` and matching detached `Preparation`
methods accept the caller's decoded realm/reservation. The history's configured
cumulative restore-pixel budget still constrains explicit admission. Hot snapshots,
weakly reusable decoded documents and tile versions can be reused without allocating
new bytes; a fully resident target can succeed with zero fresh capacity.

Cold restoration first builds a fallible, cancellable plan over unique blob
identities. It pins reusable tile leases, counts all missing blobs, validates the
cumulative restore-pixel allowance and reserves the complete missing set before
materializing any tile. Duplicate references share the plan's single lease; this
is identity reuse rather than content deduplication. Concurrent cache warming can
reduce actual decoding, but the plan never relies on an unpinned resident version.
The plan owns its reuse pins only through this attempt.

Cold restoration materializes into that same admission. A later tile failure drops
the private prefix and prunes dead weak aliases. The owner retains its document,
cursor, redo and saved acknowledgement. Native deferred requests keep Undo/Redo or
the exact guarded seek target, without holding a private history fork while waiting.
They publish known demand through the existing observer and use a fresh guarded
preparation after preserving maintenance. Every native navigation attempt plans
and reserves its current missing set on the worker, including the first attempt.
It does not blindly reserve the previous failure's demand. Original success/
empty-status handling and target repair remain attached to the request. No recorded
edit is replayed to restore pixels.

Partial decode/cancellation and rejected publication release their private decoded
leases and unused capacity. A changed base cancels rather than rebases navigation.
Impossible decoded footprints and fixed restore limits remain terminal. Codec,
checksum, source I/O and other unclassified failures retain their truthful errors.

## Limits and evidence

Initial native batch/tool/navigation submission races now retain their tasks.
Decoded-capacity retries cover successfully submitted native batch edits and
recorded undo/redo/seek restoration. Other service failures do not yet have this
complete retained retry lifecycle. First-attempt batch/effect planning, global
scheduling fairness and starvation policy remain open.
Waiting navigation replans when the decoded realm's successful allocation/release
activity changes, even if its old demand cannot fit and counters returned to the
same values. Denial notifications and pool-internal recycling do not change that
activity stamp, preventing immediate self-denial feedback. A provisional five-second
fallback also rechecks caches warmed through another realm. At activity saturation,
change checks remain conservative. Batch edits still use learned-prefix demand.
Current documents/readers can pin all available capacity, so waiting may require
explicit cancellation until logical eviction is implemented.

Reserved decoded bytes do not include retained command/preview vectors, fonts,
metadata, encoded history, codec workspaces, CPU outputs or GPU allocations. Those
resource classes still need coordinated admission. Individual command kernels and
codec/filesystem calls retain their existing cancellation granularity. Native
pressure, sustained input/interference, observer/reservation overhead and allocator/
RSS evidence remain unqualified; no speed or RSS gain follows from these checks.

Core/raster regressions cover transfer/recycling, clones, independent reader
lifetime, shared-capacity wakes, concurrent final release, failed materialization
and strict pool exhaustion. Editor regressions compare exact receipts and pixels,
underestimated reservation rollback, staged policy, cancellation and stale results.
Native tests cover preview/FIFO/save ordering, Escape, stale-base refusal, permanent
limits, own-history cleanup and shared-worker interference. An independent-context
test uses one real four-tile realm: eligible history release enables the other
context's accepted two-tile stroke, without altering the history owner's ticket.

Paint descriptor/replay-index capacity now has a separate
[fixed per-plan metadata allowance](PAINT_METADATA.md). Every painting family
constructs its plan from the admitted policy. Metadata exhaustion carries no
decoded denial and is not a history-pressure retry. This bounds a prerequisite
for compound planning without claiming complete first-attempt batch reservation.

[Flood preparation](FLOOD_PREPARATION.md) now receives the worker cancellation
callback and a separate per-call scratch limit from raster admission. Fixed scratch
errors carry no decoded denial; cancellation or exhaustion after a private prefix
discards that attempt without advancing owner/history state. Native wall-clock
cancellation tails and full-batch resource planning remain unqualified.

[Shape visitors](SHAPE_PREPARATION.md) now feed rectangle/ellipse descriptors
directly into the admitted plan and check cancellation during row preparation. A
failed sink or cancelled outline cannot publish a preceding fill/private batch
prefix. Large clipped ellipses retain exact arithmetic rather than overflowing
before tile admission; aggregate geometry/whole-batch policies remain separate.

[Pencil/stroke preparation](STROKE_PREPARATION.md) now checks invisible path work
and vertex accounting, traverses borrowed segments, and uses a capacity-bounded
cancellable unsmoothed union before compiling paint descriptors. Fixed geometry
limits do not become decoded-capacity retries. Aggregate batch/service admission
and native wall-clock tails remain unqualified.


Built-in text now [streams into admitted plans](TEXT_PREPARATION.md), and resolved
mask validation/accounting/clipping observe cancellation even offscreen. This
removes an editor geometry collector without extending per-plan limits into a
whole-batch, installed-font or physical-memory guarantee.
