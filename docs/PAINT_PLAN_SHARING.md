# Sharing immutable compiled paint descriptors

The batch compiler retains every operation but shares descriptor storage with the
previous retained paint when the exact ordered writes match. This addresses the
retained-geometry cost observed in the [complete batch experiment](../docs/benchmarks/batch-preparation-2026-10-22/DECISION.md).
It does not merge operations or skip blending, tile simulation, source-program
updates, validation, work accounting, cancellation or replay records.

Equality includes local coordinates, RGBA, blend mode and replacement coverage.
It is not equal-output or contextual no-op inference. Comparison uses bounded
1024-descriptor chunks with cancellation checks and no scratch allocation. A shared
identity can return after the initial check. Changing a plan still follows the
ordinary admitted COW path.

All retained candidates in one compiler use the same admission realm and per-plan
policy. Sharing does not import storage from another caller's budget. A previous
plan with larger descriptor capacity is rejected as a reuse candidate. Newly built
geometry and its admission still occur before comparison, so this does not make
geometry construction free or guarantee lower wall time. It also does not search
the complete batch, hash geometry, retain a global cache, or skip a different last
plan in search of a match.

Repeated translucent paint must blend repeatedly. Frozen duplicate ancestors,
logical flood readers and later changes to clipping/selection retain their own
semantics. Exact equality is independent of layer identity because the descriptors
are already resolved local writes; each replay record retains its own layer ID.

## Executed evidence

Two raster tests distinguish color, blend policy, coverage, order and post-clone
mutation, and cancel every observed comparison checkpoint on independently built
4097-descriptor plans. The batch regression fits 100 translucent diagonal operations
within eight times the single-operation metadata peak, retains a different result
from one operation, and matches the frozen executor. Separate tests preserve
selection changes and frozen duplicate/flood branches.

[Validation](validation/paint-plan-sharing-2026-10-22/validation.json) retains 911
passing workspace tests (26 explicit ignores, 84 suites), strict all-target Clippy,
124 optimized raster tests and 119 optimized editor tests, plus release benchmark
and CPU-only CLI builds. After adding the final duplicate/flood regression, the
nine-case batch suite passes in debug and release and strict Clippy passes again;
production code did not change. The complete workspace was not rerun for that
test-only addition. Original logs and source/frozen-artifact identities are retained.

The same release verifier and original fixtures retain exact semantic hashes and
decoded peaks. Declared metadata peaks before/shared are:

| Case | Before bytes | Shared bytes |
| --- | ---: | ---: |
| Horizontal pencil | 23,568 | 7,888 |
| Diagonal pencil | 4,111,632 | 97,552 |
| Smooth brush | 265,488 | 14,608 |
| Smooth eraser | 265,488 | 14,608 |
| Mixed lineage | 36,728 | 36,728 |
| Default control | 1,840 | 1,840 |
| Dense | 24,072 | 8,392 |

These are actual declared-account peaks, not physical memory ceilings or RSS.
The [matched follow-up](../docs/benchmarks/plan-sharing-2026-10-22/DECISION.md)
now retains 480 statistical samples and 3,120 original process observations,
including early color-mismatch controls and startup calibration. Diagonal process
RSS decreases and library wall time improves 2.2–5.1%, but other controls have
large slow tails, mixed sessions and late calibration drift. Sharing is selected
for retained memory, without a general throughput claim. The original mixed-lineage
regression remains unresolved.
Global service budgets, native input/display/cancellation latency and the broader
feature roadmap remain open.
