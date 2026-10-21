# Decoded capacity reservations and accepted-edit retries

Native edits that fail a feasible decoded-tile allocation retain their captured
validated batch at the front of their owner's editing queue. They do not publish
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

## Retry and publication

The first native edit still allocates incrementally. Worker-side stroke/text plan
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

## Limits and evidence

This covers decoded failures during successfully submitted native batch edits.
Initial worker-submission races, history navigation/restoration and other service
failures do not yet have the complete retained retry lifecycle. Full first-attempt
request planning, global scheduling fairness and starvation policy remain open.
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
