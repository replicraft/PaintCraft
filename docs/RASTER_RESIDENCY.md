# Decoded tile allocation ownership

Decoded 256×256 RGBA8 tiles now retain a byte-reservation lease for the lifetime of
their actual Vec payload. Standard `Admission::new` calls share one process-default
realm with a 2 GiB ceiling (`DEFAULT_RESIDENT_TILE_BYTES`), used by native/headless
editing, imports, preview staging and history restoration. The per-batch staged
pixel allowance still applies independently.

The common `SharedBudget` / `BudgetLease` primitive lives in
`paintcraft_core::types::resources`, with compatibility exports in
`paintcraft-runtime`. Worker jobs and decoded tiles use separate realms because
their charges describe different resources. A declared 2 GiB worker charge is not
an additional measured 2 GiB of decoded allocation, and their ceilings cannot be
summed into a process RSS estimate.

## Allocation and ownership

Before allocating a tile, raster checks the staged-pixel allowance and reserves
one full tile's payload bytes, or transfers them from a caller's already admitted
[capacity reservation](EDIT_ADMISSION.md). Allocation is fallible; Vec capacity must exactly
match the charged payload size before the tile is retained. Failed allocation or
initialization drops its candidate and returns the lease. Actual allocator padding,
Arc/control-block metadata and temporary buffers are outside this payload count.
An unexpected excess Vec capacity is rejected before exposing the allocation; that
post-check does not establish a physical allocator/RSS bound during allocation.

Every fresh tile, COW copy and `TileLease::materialize` restoration follows the
same construction path. A COW failure leaves the original tile installed. An
unshared tile with weak version tokens transfers its Vec and lease together,
retiring the old weak identity without allocating another pixel buffer. Pixel
math, finite-edge padding and hidden RGBA remain unchanged.

Snapshots, recorded raw history and explicit `TileLease` readers share the tile's
single charge. A reader can keep it alive after a history entry or document is
removed. Weak compositor/cache/history identities retain no decoded payload or
reservation. The Tile's pixel Vec is destroyed before its lease, so the last
reader's release is the ownership boundary for returning payload capacity.
Returning capacity does not force the native allocator to reduce process RSS.

A resource-limit failure does not trim committed history. Private batch staging
and failed restoration release unpublished tiles, with publication remaining under
the existing session guards. Low-level raster mutators retain their existing
caller-managed staging contract; arbitrary multi-tile direct mutation is not a
transaction API.

## Explicit realms and counters

`Admission::with_residency` supports a caller-supplied realm in addition to its
staged-pixel allowance. `execute_with_admission` runs the same private transaction
implementation under that explicit policy. `residency_usage` reports live reserved
payload/reserved-headroom bytes, their ceiling and a reservation high-water mark.
Explicit reservations include unused admitted headroom until their final owner
drops; surviving tile leases retain only their actual payload charges.

An explicit realm is separate from the process default. A host choosing multiple
realms must coordinate their aggregate policy. This API does not migrate existing
allocations or configure all future Session/history/import admissions for that
host. Standard production callers continue using the shared default realm.

The default 2 GiB ceiling is a provisional engineering guardrail, not an
available-memory-derived startup setting or reference-application limit. Current
history pressure also has a [native owner coordinator](HISTORY_PRESSURE.md) that
prepares preserving storage work across inactive tabs and parked frontend
workspaces. [Budget notifications](BUDGET_NOTIFICATIONS.md) wake native resource
consumers across contexts. Known-demand edit retries can reserve capacity before
restarting. Native multi-window wake qualification, first-attempt whole-request
planning and adaptive all-service pressure remain incomplete. Readers can pin
capacity indefinitely; a waiting edit can be explicitly cancelled.
Published state and undo remain intact.

## Failed-attempt demand

Decoded-budget failures retain a typed allocation diagnostic with the exact realm.
Each Admission lazily creates an allocation gauge shared by its clones and tiles.
The requirement is the greater of its prior peak live bytes and live bytes plus
the denied tile. Dropping unpublished tiles reduces live ownership without erasing
the observed peak. Pixel storage is freed and the gauge retired before capacity
is returned; budget wake callbacks run outside the gauge lock.

The native fresh serial attempt uses this diagnostic to avoid retrying a partial
prefix with insufficient space. It can observe capacity returned before error
delivery. The requirement covers only the observed attempt prefix: later operations
may demand more, and reused or independently cloned scopes are not a general
whole-request planner. Fixed staged caps and native allocator failures carry no
decoded-reclamation readiness. Wire error/v1 fields, equality and pixel policy are
unchanged. These metadata/counter costs have not been benchmark-qualified.

## Executed checks and limits

641 workspace tests pass, with 21 explicit ignores. Strict workspace/all-target
Clippy, formatting, the 800-line limit, release desktop/CLI builds, required Metal
parity/lifetime checks and a 30-frame isolated native-window smoke pass.

Six raster regressions cover reader-pinned exhaustion, COW lifetime accounting,
weak-token ownership transfer, failed materialization, partial edge-tile imports,
racing allocation admission and shared default-realm observation. Two editor
regressions check whole-batch rollback at the second allocation and actual history
compaction/reader release/undo/redo restoration counters with independent pixels.
Small explicit ceilings exercise the same allocation path without allocating
2 GiB merely to provoke exhaustion.

Compressed history, codec workspaces, source/decode buffers, tile indexes,
full-frame outputs, masks, CPU/GPU caches and native/GPU allocations are not covered
by this tile realm. Logical blob authority, adaptive all-service admission, pressure
coordination, metadata bounds and physical/RSS qualification remain incomplete.
Allocation-accounting overhead and sustained native input/interference still need
measurement. No throughput or RSS improvement is claimed by these correctness
checks. [T04/T11/T13 remain partial](ROADMAP_STATUS.md).
