# Reserved paint metadata admission

Native metadata retries atomically reserve their known attempt requirement in the
original metadata realm before submitting work. Descriptors and projection/replay
indices transfer bytes from that capacity rather than competing for global
headroom again. Decoded tile admission remains a separate realm and policy.

## Ownership

`BudgetReservation::scope` transfers a reservation into a clonable account factory.
The factory retains unused capacity. Accounts and their siblings hold weak links
to it: they never retain the unused part merely because a plan or projection
survives preparation. Account growth takes reserved bytes atomically; shrink/drop
recycles them while the factory survives. Once the last factory or other reservation owner drops, unused bytes
return globally and each surviving account keeps only its own declaration.
Further growth of surviving accounts uses ordinary shared admission.

While a reservation survives, growth beyond its available capacity fails even
when unrelated global headroom exists. Existing charges and live scope declarations
remain unchanged on failure; the observed demand peak includes the rejected
pending growth. Lease and account consumers share the same reservation counter.
No scope/reservation lock is held across global budget callbacks, and storage must
still be freed before its owning account shrinks or drops.

The raster admission's `with_reserved_paint_metadata` carries the factory through
every plan it constructs and each replay index. Ordinary admissions retain the
previous shared-demand behavior. Per-plan metadata and cumulative staged-pixel
limits remain independently enforced; neither limit silently expands.

## Native retry lifecycle

The native coordinator reserves the waiting demand before dispatch, retaining the
captured request, owner/base ticket, FIFO position, preview and file barriers.
Worker-admission failure drops the captured admission and its unused reservation;
the waiting request survives for another dispatch. Cancellation, stale bases and
fixed/impossible limits keep their existing behavior. Metadata demand does not
trigger decoded-history reclamation.

The known demand can be incomplete. If preparation discovers a larger requirement,
it drops the old private attempt/reservation and defers the same captured request
with the larger peak. A provisional five-second periodic probe may run ordinary
live admission when its old requirement cannot be reserved, allowing repricing.
This fallback has no reservation guarantee; cleanup cannot trigger an eager loop.
There is no combined atomic reservation across tile and metadata realms, no
cross-window fairness guarantee and no complete upfront batch plan.

## Executed evidence and remaining scope

Core tests verify strict bounds, sibling/lease competition, exact transfer and
recycling, independent release after factory teardown, reentrant callbacks,
concurrent account contention and concurrent final factory/account destruction.
Raster tests cover exact-capacity descriptor/index replay under fully occupied
global capacity, insufficient reservations before tile allocation, retained
projection recovery and cancellation recycling. Native tests run an actual
captured request while a competitor occupies all unreserved bytes and verify
larger-demand deferral preserves identity and publishes once after retry.

[Source-pinned validation](validation/reserved-paint-metadata-2026-10-22/validation.json)
records 883 passing workspace tests, strict all-target Clippy, 46 optimized core
and 29 optimized raster tests, required Metal/desktop tests, release builds and
isolated native startup checks. The sandbox adapter failure and corrected test
notification harness remain archived.

These are ownership and admission correctness checks. The prior
[normal-path scope comparison](../docs/benchmarks/metadata-waiting-2026-10-22/DECISION.md)
is frozen to its recorded source and does not qualify this change's overhead or
native latency. Whole-batch/source-lineage planning, contextual no-op proofs,
aggregate geometry/font/flood/service budgets, physical memory accounting,
scheduling fairness and native latency qualification remain open.
