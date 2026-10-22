# Combined logical paint programs

`PaintProjection::append_checked` consumes another PaintPlan into one ordered
logical program. The shared incremental index records each appended descriptor
once at each touched tile. A read calls the preceding source once, then evaluates
that tile's descriptors in original order. There is no per-operation projection
chain, descriptor-order regrouping or repeated alpha-composition shortcut.

The receiving projection's metadata policy bounds its combined descriptor capacity,
usable tile-map payload and every write-index capacity. While appending, the
incoming descriptor vector's entire capacity is also charged until its IntoIter
allocation drops. A child's larger/smaller per-plan allowance neither widens nor
shrinks the receiving program's policy. The append transfer uses fallible growth,
checks returned capacities and yields during descriptor/tile work. Fixed policy
failure has no retryable decoded allocation denial.

Append consumes both arguments. Any quota, allocation or cancellation failure
therefore drops the whole private program, including a partially updated index;
no reusable half-appended result escapes. Source pixels are never requested or
owned by append. Caller-owned inputs before transfer, other programs, source
chains, allocator/hash controls, transient reallocations and process RSS remain
outside this one program's payload policy. This is aggregate retained-plan
accounting within one program, not a shared batch/service lease.

The captured layer-local bounds constrain indexing and reads. As with the original
projection, recovered descriptors retain their original areas; the owning planner
must preserve captured source identity/clipping and handle structural changes.
`into_plan` drops the read index and recovers one ordered plan for ordinary admitted
replay. That replay can reserve the complete program's touched tiles before any
decoded allocation, without rebuilding geometry. Replay indexing still follows the
existing source-aware missing/default and private/COW policy.

## Executed acceptance

Five new tests verify 64 ordered mixed source-over/partial-replacement operations
across tile boundaries, exact recovered replay and one source read per pixel.
Tight limits reject aggregate growth and oversized incoming capacity even for
fully offscreen descriptors. Every observed append checkpoint can cancel the
consumed private attempt. A two-operation program fails a two-tile replay quota
before allocating the first tile. An independent BFS checks combined logical
flood after earlier writes over all 512 binary 3x3 images, nine seeds and four
tolerances: 18,432 cases.

[Validation](validation/paint-program-2026-10-22/validation.json) pins source,
commands and exact logs. The [three-way preparation comparison](../benchmarks/results/paint-program-2026-10-22/DECISION.md)
retains materialized, chained and combined paths with required pixel/region
preflights, raw statistical/process samples, calibration and no-op controls.
It measures current real library preparation, not native editor or file workflows.

The editor still executes sequentially. Complete batch integration needs bounded
ownership across layers/programs, duplication/deletion/expansion lineage, contextual
uniform/no-op proofs, aggregate geometry/font/flood scratch, decoded peak planning,
source guards and appropriate operation-error attribution before publication.
Per-program bounds and work-count checks do not establish physical ceilings or
native input/display/cancellation tails. No dependency or wire/file policy changes.


Paint descriptors and indices now have [shared ownership-based admission](SHARED_PAINT_METADATA.md)
in addition to their fixed per-plan/program policy. Plan clones share charged
immutable storage; mutation copies under admission. Metadata denials carry their
own class and never enter decoded-history reclamation. Full-attempt metadata
waiting and aggregate geometry/service/physical budgets remain open.
