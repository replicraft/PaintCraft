# Shared ownership of paint metadata

Paint descriptor and replay/projection index allocations now use the existing
SharedBudget authority. A provisional process-default realm admits 256 MiB of
usable payload, independently of the existing 64 MiB per-plan/program policy and
decoded tiles. Admission clones share the metadata realm; SDK callers can supply
`Admission::with_paint_metadata_budget` or `PaintPlan::with_metadata_budget`.
Every current editor painting family already uses that factory.

## Ownership and admission

`BudgetAccount` is a unique, movable resizable declaration. Growth reserves a
positive delta atomically; rejected growth preserves the old charge. Shrink/drop
releases after storage is freed, outside the budget counter lock. Multiple storage
owners compete in the same realm, with the existing watches/activity notifications.
The resource class PaintMetadata keeps these denials distinct from DecodedTiles.
No new resource manager, dependency or wire error code is introduced.

Descriptor Vec capacity, usable tile-map payload and every write-index Vec capacity
are charged through their storage owners. Index guards survive replay/cancellation/
unwind, and projection guards survive result transfer. Recovering a PaintPlan drops
the index charge while preserving its descriptors. Values drop before their
accounts notify release, including failed private construction.

Vec growth reserves the requested payload before allocation. Hash-map growth
reserves a minimum entry payload, then validates/admits actual returned usable
capacity. If a returned capacity exceeds either policy, the failed private buffer
is freed before its charge is released. Allocator/hash controls, Arc/control headers,
transient reallocations and other services are excluded. This bounds retained
payload declarations, not a physical RSS ceiling or an interception of allocators.

## Clones and transfer

PaintPlan clone shares immutable descriptor storage and one charge. Mutation makes
a separately admitted fallible copy before changing either value. Copies preserve
existing capacity instead of requesting amortized spare capacity that can needlessly
exhaust a tight aggregate policy. Checked append yields during copy chunks. The
ordinary rect builder retains its existing caller-owned cancellation contract.

Combined append retains the incoming storage owner through transfer, charging
its full descriptor capacity under the receiver's fixed local policy. Shared
storage is charged once in its actual realm. Incoming buffers from another realm
remain charged there until their final owner drops; destination growth is charged
to its own realm. Source snapshots and failed copy/append attempts retain correct
independent lifetimes. No partially appended program is returned on failure.

## Failure and native behavior

Fixed per-plan/program quota failures still have no allocation denial. Shared
capacity failures identify PaintMetadata and the original realm. Their per-account
known demand is advisory; complete-attempt metadata demand/reservation and native
waiting/repricing are not implemented by this resource class alone. The native
decoded-history retry accepts only DecodedTiles and cannot reclaim history for a
metadata failure. Failures are reported atomically with owner/history preserved.

Thirteen new tests cover resize/contention/reentrant release order, shared plan
clones and COW failure, projection construction/recovery/cancellation, cross-realm
append, exact capacity copies, admission clones, editor private-prefix rollback
under metadata interference and native decoded-retry isolation. The clamped-capacity
regression is native 64-bit specific; other profile qualification remains separate.
[Validation](validation/shared-paint-metadata-2026-10-22/validation.json) pins source,
commands and compressed logs.

The [matched overhead comparison](../benchmarks/results/shared-paint-metadata-2026-10-22/DECISION.md)
retains frozen CPU-only CLI and real-library baselines, source/binary/feature/input
identity, exact canonical output and raw samples. Shared accounting is a correctness
baseline; its overhead and negative controls are reported rather than described
as a throughput win. The default allowance is provisional, not a measured optimum.

Complete batch/source-lineage planning, metadata waiting/fairness, aggregate
geometry/font/flood scratch, allocator/physical budgets, native latency and broader
roadmap work remain. The editor still executes batches sequentially.


[Scoped metadata demand and native waiting](METADATA_WAITING.md) now retain
metadata-blocked edits with their original realms, FIFO position and file barriers.
Distinct metadata watches do not request decoded history reclamation. The known
attempt peak supports repricing; upfront batch planning and atomic metadata
reservations remain separate work.
