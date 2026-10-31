# Prepared point programs

Status: standalone admitted Levels ownership and row evaluation, without a new
Descriptor, document format, command, UI option or global cache. This foundation
prepares the renderer for the [complete Levels policy](LEVELS_FILTER_FOUNDATION.md).
The existing seven editable filters retain their current paths.

## Admission and lifetime

`paintcraft_render::prepared::PreparedLevels::new` takes validated parameters,
the caller's existing `WorkspaceAdmission` and cancellation check. It uses the
same cumulative work counter as spatial regions; it creates no process realm,
fallback allowance or per-row compiler. Conservative compile work remains spent
after a cancelled or failed attempt, matching spatial preparation.

`quote_levels` separates peak payload bytes, transient kernel scratch, retained
table bytes and compile work. On the qualified native target these are 6,952
peak bytes: 6,696 scratch plus a 256-byte table. Both declarations are acquired
before constructing scratch/table values. Scratch ends before its account is
released. The completed table and its retained account move together only after
the final successful check. Error or unwind returns no owner and releases both
accounts.

The admission's byte ceiling applies to one local preparation/region. Aggregate
simultaneous payloads are constrained by the caller's shared realm or reserved
job scope. A caller requiring a tight aggregate job ceiling must supply that
bounded scope; a large realm is not made smaller by a local plan ceiling.
Payload accounting excludes caller input/output, Rust owner/Arc bookkeeping,
allocator overhead and physical RSS. It is not allocator interception.

The [qualified backend selector](LEVELS_BACKEND_SELECTION.md) preserves the u32
reference path and chooses u64 for sampled large quoted workloads. Scratch is
constructed only in the selected helper. Raw compile reports are tagged by word
width; the shared job counter reserves a conservative abstract maximum rather
than treating those different raw operations as equal-cost units.

The owner is non-Clone, immutable and Send/Sync. Callers may share it through
their own Arc without duplicating the table's payload charge. After a reserved
job factory retires, unused reservation capacity is released while the surviving
owner retains only its table charge. Its last owner releases those bytes.
The program may be reused in a new explicitly admitted job without recompilation.
It is derived data; owning it grants no document-adoption, bake or privacy consent.

## Row execution

`evaluate_row` borrows immutable source and optional byte coverage, plus disjoint
caller-owned private output. It validates all lengths before callbacks/writes,
reserves two bounded logical work units per pixel in the same job counter, and
checks cancellation before each 64-pixel chunk and at completion. There is no
LUT construction or heap allocation on the successful row path apart from any
behavior supplied by the caller's cancellation function.

Every RGB byte, including hidden RGB at alpha zero, uses the already prepared
table. Missing coverage is 255; zero coverage preserves the incoming tuple.
Other coverage blends the complete rounded candidate exactly once through the
existing point-mask policy. Alpha stays exact. A geometry or admission failure
before execution leaves output intact. Cancellation during a row may leave
private partial output, which the host must discard; it is not an atomic image
publication API.

## Qualification and remaining integration

Controls exercise independent literal RGB/alpha/mask expectations, repeated row
reuse without new compile work, shared-reader lifetime, preflight/parent denial,
cancellation at every preparation checkpoint, panic cleanup, aggregate charges,
reserved-job retirement and partial-row cancellation. A mixed spatial/prepared
control checks the same cumulative counter in both execution orders. Qualification
receipts retain their exact source/command scope; this standalone API is not
proof that an existing frontend uses prepared Levels.

The earlier eight-test checkpoint passed, including mixed spatial/prepared work in
both orders. Native all-target Render Clippy and the affected Render/Formats/
preview-Worker Wasm library checks pass. The initial seven controls were also
qualified with the closed-worker changes; the added mixed control and final
source are covered by `docs/validation/levels-planning/prepared-final-tests.log`,
`prepared-native-strict.log` and `prepared-wasm-strict.log` in that directory.

The adaptive backend checkpoint passes all nine prepared-program tests, adding
wide-backend selection, tagged accounting and cancellation cleanup. Its exact
source and command receipt is
`docs/validation/levels-optimization/refactor/functional-freeze.json`; see
[backend selection](LEVELS_BACKEND_SELECTION.md) for actual-artifact measurements.

Before exposing the filter, the frozen job's point program must prepare once per
distinct complete parameter/profile set and retain that owner across its tiles.
The key includes all five validated parameters and the numerical profile, not
only resulting table bytes. This API intentionally supplies no implicit cache.
Any later cross-job cache needs bounded ownership, eviction, cancellation and
retained-charge tests. Then connect document/history semantics, the catalog
boundary, host job admission and all native/browser/CLI appearance consumers.
Old worker profiles must stay closed until their own prepared-work contracts are
implemented. The [Levels integration gates](LEVELS_FILTER_FOUNDATION.md#prepared-execution-and-remaining-integration)
remain authoritative.
