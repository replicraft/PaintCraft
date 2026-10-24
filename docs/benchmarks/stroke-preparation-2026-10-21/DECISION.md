# Bounded stroke preparation: correctness baseline with measured overhead

The checked union remains the editor baseline because bounded scratch growth and
cooperative cancellation are required. This comparison does **not** establish a
brush speed improvement. Square/round preparation regresses, and the retrace case
has a larger variable penalty. Optimize this checked implementation in subsequent
source-pinned comparisons rather than restoring the unbounded legacy path.

## Inputs and identity

Candidate source `5e7f1f2` is compared with frozen CPU-only CLI baseline `9f50410`.
Both CLI feature trees are byte-identical and contain no wgpu. The earlier combined
desktop/CLI snapshot recorded in the manifest was untimed and is excluded.
`manifest.json` and `baseline-manifest.json` pin sources, executables, toolchain,
fixtures and commands. Neither source was edited during timing; no task builds or
tests intentionally overlapped measurement. This is one uncontrolled macOS/Metal
host, without affinity, power or thermal controls.

Each process case reads and hash-checks the same 1000-square PCND input, prepares
100 atomic operations, publishes recorded history, encodes canonical PCND and
atomically replaces a separate output. Pencil is size one; square is size 16;
round/retrace are size 32; retrace uses five vertices with four full-row traversals. Smooth-round is a
control. Input includes opaque pixels and hidden transparent RGB. Exact archive
and every ZIP member equality pass before timing and after every session; output
hashes are stable across all checks. Cases do not time native pointer events.

## Measurements

Criterion retains 20 raw samples per variant for four union cases (160 total).
Independent required coverage agrees before timing. Borrowed fixture setup is
excluded; union construction, output collection and destruction are included.
Legacy sorts all rows; checked output requires only disjoint x-ordered row runs.

| Kernel | Legacy mean ms | Checked mean ms | Checked time change |
|---|---:|---:|---:|
| tiny | 0.000372 | 0.000375 | +0.9% |
| square | 0.148631 | 0.169835 | +14.3% |
| round | 0.474783 | 0.554648 | +16.8% |
| retrace | 2.539611 | 3.334559 | +31.3% |

Hyperfine retains three sessions in forward/reverse/forward order, 20 runs and
three warmups per variant: 600 workload observations plus 120 `--version`
calibration observations, all exit zero. Original schema-v2 exports are compressed
without alteration; compact JSON preserves identical parsed values. No calibration
subtraction, outlier deletion or best-session selection is used.

| Process | Checked mean-time change across sessions | Baseline/checked means ms, sessions 1–3 |
|---|---:|---|
| pencil | -4.2% to +0.2% | 20.73/20.47, 21.26/20.37, 20.48/20.52 |
| square | +10.0% to +11.9% | 49.87/55.81, 51.27/56.87, 50.15/55.18 |
| round | +10.5% to +13.7% | 124.32/137.32, 129.33/147.07, 124.11/137.97 |
| retrace | +9.7% to +39.8% | 547.68/765.68, 583.98/674.31, 647.49/710.27 |
| smooth-control | -23.5% to -0.7% | 42.99/32.90, 31.62/31.02, 31.22/31.02 |
| calibration | -2.2% to +12.3% | 1.29/1.45, 1.11/1.14, 1.15/1.12 |

Tiny kernel intervals overlap. The square/round/retrace kernel regressions persist
in the CLI comparisons. Pencil is approximately tied. Smooth-control and calibration
vary between sessions; their apparent first-session gain is not a selected win.
Retrace distributions and RSS also drift, so the penalty range is retained rather
than reduced to one preferred number. `summary.json` includes each mean/median,
Criterion interval and session-specific median RSS; original samples are retained.
No general RSS reduction, CPU/GPU crossover or native latency gain is established.

## Policy and remaining scope

The 64 MiB per-operation scratch allowance and 1,024-record sort/check chunks are
provisional safety policies, not benchmark-selected optima. They cover declared
row-map/vector/heap capacity, excluding allocator/hash controls, transient growth,
input geometry, later PaintPlan and aggregate batch/service memory. Cancellation
checks bound work counts, not wall-clock tails. Exact once-per-stroke coverage,
private-prefix rollback and owner/history identity remain required.

Next work includes reducing checked-union overhead, whole-batch planning, aggregate
admission, other geometry/font kernels and instrumented native input/display/
cancellation qualification. See [the contract](../../../docs/STROKE_PREPARATION.md)
and its final validation record.
