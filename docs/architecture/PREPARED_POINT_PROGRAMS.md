# Prepared point programs

Status: admitted Levels ownership and row evaluation are connected to frozen
document jobs, CPU cache and GPU entry points, catalog persistence/automation,
and Studio previews. The [complete Levels integration](LEVELS_FILTER_FOUNDATION.md)
tracks current qualification; browser export checks remain open. No global cache
is introduced, and the existing seven filters retain their numerical policies.

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

## Frozen job reuse

`PreparedLevelsJob::new` eagerly prepares each distinct complete typed policy
before returning an immutable job. `quote_levels_job` reports unique programs,
pointer-index bytes, ordered peak payload and total compile allowance. Both
input and unique counts are bounded to 256 for this initial host API; this is
not a document-model limit. Equality includes all five parameters, even when
different policies produce identical constant tables.

The index's requested capacity is admitted before allocation and actual capacity
is reconciled before compilation. Each unique table retains its existing
256-byte account in the same supplied scope and cumulative work counter.
Allocator and Arc bookkeeping retain the exclusions above. Bounded index
comparisons are separate from the LUT compile-work metric.

Lookup only borrows an existing program; it cannot compile, allocate or spend
work. Callers may share an Arc of the complete job across tile workers. No
admission factory or unused reservation capacity is retained by the job.
Cancellation, failure or unwind during construction returns no partial job;
successful ownership releases index and tables with its last owner. Separate
jobs prepare independently, with no implicit process cache.

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

The subsequent eager-job checkpoint passes all 17 prepared tests, including
eight new controls for complete-key deduplication, pure lookup, aggregate denial,
cancellation at every check, panic cleanup, last-owner release, reserved-capacity
retirement and mixed spatial work. Native all-target GPU compilation Clippy and
actual Web-consumer Wasm Clippy pass with warnings denied. These are compilation
and native unit checks, not new GPU, browser UI or frontend execution evidence.
The receipt is `docs/validation/levels-planning/job-final-freeze.json`, SHA-256
`6d7d26f9b3f082a2fd7fa3c1bbb54953c8aebc627b43cffc528feee19ac14c81`.

Hosts construct the eager job from their frozen parameter inventory and retain
it across all tiles. The standalone job API alone does not connect a frontend;
the document collector and host integration now supply that binding. Unique
policies are canonically ordered before quoting and preparation so layer-map
iteration cannot change the admission peak. No implicit cache is supplied.
Any later cross-job cache needs bounded ownership, eviction, cancellation and
retained-charge tests. Document/history semantics, the catalog boundary and host
admission are implemented locally. Preview source4 and export6 negotiate their
own prepared-work contracts; older worker profiles remain closed. Their separate
execution evidence and the [Levels integration gates](LEVELS_FILTER_FOUNDATION.md#prepared-execution-and-remaining-integration)
remain authoritative.
