# Scoped metadata demand and native waiting

Paint admission factories now share a BudgetScope across one preparation attempt.
Each independently owned descriptor/index account contributes live declarations;
failed pending growth contributes to the largest observed demand peak. Denial
therefore includes the attempt's known prefix and pending allocation, rather than
only the last failed vector. Release reduces live, while retaining the observed
peak. Ordinary factory clones own no capacity and accounts release independently;
[reserved factories](RESERVED_PAINT_METADATA.md) retain their unused capacity.

Scope locks are released before shared-budget callbacks. Concurrent pending
requests can conservatively increase the known peak; live observations include
pending declarations during resize. This is observed demand, not a full upfront
batch plan or a minimal physical allocation estimate. A fresh retry creates a new
scope and can discover a larger next requirement. Direct unscoped accounts retain
their existing account-local semantics.

## Retained native work

Feasible PaintMetadata denials retain the captured batch/navigation request at its
original FIFO position, with owner/base ticket, preview and file barriers intact.
Retries keep both original decoded and metadata realms even if the worker service
is replaced. Waiting owns no temporary decoded reservation or prepared tile prefix.
Escape removes the waiting activation and later queued edits; stale bases discard
the request instead of rebasing it. Fixed quota errors and observed requirements
above the hard realm capacity remain terminal atomic failures.

The original implementation measured below used live shared admission. Current
native retries [reserve their known metadata requirement](RESERVED_PAINT_METADATA.md)
before dispatch. A larger discovered requirement can defer the same request again;
a provisional five-second live probe can reprice stale information when the old
requirement cannot be reserved. Failed-attempt cleanup alone cannot spin a known
unmet request. The decoded path keeps its reservation/navigation policies. The
editor still executes the actual batch sequentially.

## Separate wake and reclamation paths

Native windows register distinct metadata, decoded-tile and worker watches. Metadata
known demands and releases wake affected contexts without publishing or executing
edits from callbacks. Render metadata denials use the same current-key guarded
retry mechanism; obsolete document/preview/view keys leave no active demand.

Metadata interest never advertises decoded capacity demand. It cannot bypass a
queued file barrier to start history cleanup. Actual independent decoded pressure
retains its existing storage-only maintenance policy. Successful GPU display now
lets the coordinator update class-specific interest instead of clearing another
consumer's tile-release wait flag.

## Evidence and scope

Thirteen new tests cover scoped prefix/pending peak pricing, cloned factories and
reentrant callbacks; descriptor-plus-index readiness; native FIFO/preview/file
barriers, Escape, stale owners, hard-limit rejection, periodic no-spin probing and
original realms across service replacement; distinct context wakes, render retries
and preservation of resident history under metadata-only pressure. Existing decoded
retry/navigation regressions remain in the validation scope.

[Validation](validation/metadata-waiting-2026-10-22/validation.json) pins source,
commands and compressed logs. [Matched overhead evidence](../benchmarks/results/metadata-waiting-2026-10-22/DECISION.md)
uses frozen pre-scope/current CPU-only CLI and library binaries with identical
required output. Unit pumping deadlines and wake counts do not qualify native
input/display/cancellation latency or cross-window fairness.

Complete batch/source-lineage planning, joint multi-resource reservations, aggregate
geometry/font/flood budgets, physical memory accounting, native pen/IME/device
qualification and the broader feature roadmap remain open. No wire/file policy or
new dependency is introduced.
