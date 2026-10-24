# Combined logical paint: one index, bounded retained program

Use the combined index for continued logical-program development instead of
chaining independent per-operation projections. In the 64-plan fixture it removes
the large chain lookup penalty while retaining exact pixels, fewer metadata
capacities and consuming failure semantics. Keep ordinary sequential editor replay
until complete batch/lineage integration, shared leases and contextual no-op proofs
are implemented and qualified. This record does not activate a native batch path.

## Identity and required work

Measured source `207227b` includes production `c00140c`. The manifest pins source,
toolchain, fixtures and a frozen optimized executable. Three variants in that same
binary exercise real library paths: materialized paint then flood, chained logical
projections then flood, and one combined logical program then flood. Descriptor
construction, append/index work, classification and destruction are timed.

The 8x8 tiny, 512x64 wall, 128x64/64-plan overlap and 512x64 default/no-op fixtures
are unchanged in required pixels from the preceding experiment. Processes repeat
complete attempts 10,000/100/100/100 times respectively. Inputs include hidden RGB
and source-over alpha; the no-op control replaces the implicit default. Every
prior-paint pixel and canonical flood span matches before timing and after every
session. Correctness/resource output is byte-identical across those checks.

No task builds/tests intentionally overlapped timing. This is one uncontrolled
macOS host without affinity, power or thermal controls. Native editor events,
whole-batch/history/file workflows and CPU/GPU dispatch are outside the workload.

## Raw results

Criterion retains 20 samples for each of 12 case/variant combinations (240 total).
Means and bootstrap intervals below are milliseconds per complete attempt.

| Case | Materialized mean [interval] | Chained mean [interval] | Combined mean [interval] |
|---|---:|---:|---:|
| tiny | 0.01251 [0.01229, 0.01274] | 0.00497 [0.00473, 0.00532] | 0.00474 [0.00472, 0.00476] |
| wall | 1.86686 [1.83021, 1.90811] | 1.90845 [1.78321, 2.14653] | 2.01767 [1.80990, 2.41806] |
| overlap | 0.99943 [0.95032, 1.07882] | 9.85018 [9.74485, 9.96887] | 0.64336 [0.60254, 0.71044] |
| default-control | 0.50878 [0.50206, 0.51818] | 2.36796 [2.27652, 2.51607] | 2.25461 [2.22419, 2.28243] |

Hyperfine retains 20 runs and three warmups in three forward/reverse/forward
sessions: 720 workload observations plus 60 same-executable calibration observations,
all exit zero. Original schema-v2 exports are compressed unchanged; compact exports
preserve identical parsed values. All slow samples/controls remain; no calibration
subtraction or best-session selection is used. Process times include setup/teardown.
Ratios above one favor the combined program.

| Case | Materialized/chained/combined means ms, sessions 1–3 | Materialized/combined range |
|---|---|---:|
| tiny | 124.31/51.81/51.10, 122.89/49.31/49.65, 123.88/49.38/61.47 | 2.015–2.475x |
| wall | 183.96/194.11/181.81, 190.31/194.45/182.88, 208.76/185.21/185.39 | 1.012–1.126x |
| overlap | 95.73/965.11/69.22, 95.07/976.06/63.60, 98.18/971.64/62.35 | 1.383–1.575x |
| default-control | 52.16/227.84/229.69, 51.39/234.30/228.71, 53.17/230.42/232.11 | 0.225–0.229x |

Overlap improves 13.94–15.58x over the chained process and 1.38–1.57x over actual
materialization; statistical results agree on this fixture. It retains the exact
per-operation numerical order rather than associating alpha operations. Tiny
improves 2.01–2.48x over materialization, but its one-plan chained/combined control
varies between sessions, including a slower third combined session. Do not
attribute all one-plan variation to append, which this case never calls.

Wall disagrees across tiers: its combined statistical mean regresses while the
process means favor combined by a variable 1–13%. This is not a selected general
wall gain. The default/no-op case still costs roughly 4.4x as much as ordinary
materialization. Source-independent logical descriptors cannot yet exploit the
raster's contextual default proof. No universal crossover, size threshold or
whole-editor speedup is established by these four cases.

## Resource evidence

The 64-plan case retains 3,192 declared combined metadata bytes instead of 19,968
across chained projections. Both projected variants acquire no decoded lease;
materialization reaches 262,144 declared decoded bytes. Tiny/wall projection payload
is 312 bytes and default-control is 344; materialization's decoded default control
is zero. Projection-payload zero for materialization does not imply its transient
paint index allocates nothing. These counters describe distinct resources.

Combined append's capacity policy includes all retained descriptors/map/vectors
and the incoming descriptor buffer during transfer, validated by independent quota
regressions. Reported retained payload excludes that transient input, the trusted
fixture's not-yet-transferred plan list, allocator/hash controls and flood scratch.
It is not process-wide or physical RSS accounting. `summary.json`/`analysis.json`
retain session-specific measured median RSS: overlap uses less locally, while
no-op RSS remains mixed/slightly higher. No global memory ceiling or reclamation
claim follows from short runs or logical counters.

## Remaining work

Continue from this bounded combined primitive to ownership across layers/programs,
structural lineage, aggregate geometry/font/flood scratch leases, contextual
uniform/no-op proofs, decoded peak planning and guarded whole-batch replay/error
attribution. Preserve original numerical order and these negative controls in
integrated comparisons. Native input/display/cancellation tails, physical pressure
and release compatibility remain unqualified.

See [the program contract](../../../docs/PAINT_PROGRAM.md) and
[source-pinned validation](../../../docs/validation/paint-program-2026-10-22/validation.json).
