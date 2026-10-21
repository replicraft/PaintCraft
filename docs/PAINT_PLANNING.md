# Ordered paint descriptors and operation admission

Raster PaintPlan stores ordered layer-local rectangles/runs and exact 16-sample
replacement coverage. Clipping precedes tile indexing. Each tile's descriptors
keep their original order; independent tiles can be prepared/replayed in any order
because blend and replacement coverage read only the destination pixel. Row replay
uses the existing source-over policy and the same integer coverage formula. Hidden
RGB, finite-edge defaults and selection/origin handling stay unchanged.

The editor compiles fills, pencil paths, smooth and unsmoothed strokes, shape
fill/outline, masks, built-in text and flood regions into this shared representation.
Fill and outline share one operation plan. Flood classification still examines the
original private raster before compiling its paint descriptors. Earlier operations
in a batch are executed sequentially, so later flood/selection/structural commands
see the appropriate private state. This is not an upfront plan for the whole batch.

## Admission and ownership

The plan indexes touched coordinates and counts fresh/missing or shared COW tiles.
Unshared tiles need no new decoded capacity. Weak-only identities use atomic
ownership transfer, as in the existing mutator; a racing/live reader still requires
COW. Planning may retire weak derived identities without changing pixels. Counted
COW is conservative if transient readers disappear or unusual host-created aliases
share one buffer across coordinates; no claim of minimal reservation is made.

Staged-pixel validation occurs while discovering fresh tiles, stopping oversized
sparse operations before indexing their full logical canvas. A scoped child
admission then reserves the operation's known tile count, prepares all tile buffers
and applies its writes. Existing explicit reservations remain strict and are never
silently expanded. The cumulative staged allowance is consumed by actual tile
allocations rather than charged twice by planning.

A per-operation reservation returns unused bytes when that operation ends. Surviving
tiles keep independent leases. The parent batch still tracks live/peak attempt
ownership, so a later denial reports the earlier live prefix plus the pending
operation, not just one denied tile. This preserves safe readiness after whole-batch
rollback. Published document/history changes retain the Session atomic boundary.

Quota denial precedes this operation's tile allocation/pixel writes. Descriptor,
index and allocator failures use fallible construction, but low-level callers still
own private staging/rollback. PaintPlan is not a new standalone transaction API.
Descriptor/index/command metadata is not fully covered by the decoded payload realm;
work limits and early fresh-tile checks do not constitute all-service/RSS admission.

## Verification and measured scope

Raster regressions compare ordered clipped overlap against direct scalar APIs and
an independent coverage formula, test hidden RGBA and weak-reader COW, verify no
quota-failed tile prefix, check sequential footprint readiness, and stop oversized
sparse plans at the staged limit. An editor regression exercises eight painting
variants under the same two-tile quota and verifies successful exact capacity use.
Existing command oracle, antialiasing, selection, preview and history tests retain
the numerical/transactional acceptance checks.

The [measured replay comparison](../benchmarks/results/paint-plan-rows-2026-10-20/DECISION.md)
archives Criterion raw samples, three Hyperfine sessions and process resource data.
Horizontal pencil, brush runs and partial replacement improve locally; diagonal
pencil remains a non-win control. Earlier slower candidates are preserved. These
are same-source synthetic replay comparisons, not native input/display latency or
parent desktop benchmarks. The separate [matched CLI comparison](../benchmarks/results/paint-plan-cli-cpu-2026-10-21/DECISION.md)
checks baseline/current CPU-only apply commands and byte-identical canonical output:
horizontal pencil improves 1.12–1.20x, smooth brush 1.07–1.10x and smooth eraser
1.66–1.73x; diagonal remains a tie/slight regression. Full-batch planning, bounded descriptor/index metadata,
priority/fairness and long-session physical-pressure qualification remain open.
