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
one full tile's payload bytes. Allocation is fallible; Vec capacity must exactly
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
payload bytes, their ceiling and a reservation high-water mark.

An explicit realm is separate from the process default. A host choosing multiple
realms must coordinate their aggregate policy. This API does not migrate existing
allocations or configure all future Session/history/import admissions for that
host. Standard production callers continue using the shared default realm.

The default 2 GiB ceiling is a provisional engineering guardrail, not an
available-memory-derived startup setting or reference-application limit. Current
history pressure responds to per-session retention budgets; it does not yet
coordinate compaction/spill across tabs when this shared tile realm fills. A new
edit can therefore receive `resource.limit` while an inactive session still owns
reclaimable history. Published state and undo remain intact.

## Executed checks and limits

633 workspace tests pass, with 21 explicit ignores. Strict workspace/all-target
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
