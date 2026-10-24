# Keep direct single-layer display explicitly selectable

Measured source: `ea2b154` on Apple M1 Pro / Metal, Rust 1.99.0. The frozen
optimized desktop test executable, complete source/dependency hashes, selected
Hyperfine version and commands are in `benchmarks/results/gpu-single-layer-2026-10-21/manifest.json` (local result). Both
variants run actual preview staging, completed GPU native-resolution rendering and
App texture registration for eight frames. Cold device, pipeline and document
setup and destruction are included. This measures texture readiness, not OS input,
swap/presentation latency, physical GPU allocation or a CPU/GPU crossover.

The contender reads an existing bounded source view directly into a fresh display
texture. The ordered control first creates/completes an intermediate composite,
then converts it into the same independently owned display texture. Layer-opacity
rounding, hidden RGB handling and ecolor premultiplication are exact. Multiple
contributing layers and partial coverage retain the ordered path. See
[the implementation contract](../../../docs/GPU_SINGLE_LAYER.md).

## Complete evidence

Six independent command/scalar preflights and postflights after all three process
sessions compare every required display byte for both variants. The renderer also
passes all 65,536 alpha/opacity combinations and tests source reuse/patching,
streaming, cancellation, epoch rejection and old-frame ownership. Desktop checks
exercise actual egui framebuffer pixels and the admitted native worker.

Criterion retains 20 raw samples per case/variant: 240 raw JSON
sample values across twelve contenders, with estimates and outlier classifications.
Three Hyperfine sessions retain 20 observations per contender and case, with
forward/reverse/forward order and three warmups: 720 comparisons plus 60 calibration
observations. All 780 exit codes are zero. Hyperfine 2.0.0's original schema-v2
measurements include wall/CPU time, process RSS, cycles and instructions. Exact
original exports remain compressed alongside compact equal-valued JSON and hash
sidecars. Calibration means are 10.89/13.62/11.55 ms and are never subtracted.
No slow samples, controls or sessions are excluded.

`benchmarks/results/gpu-single-layer-2026-10-21/summary.json` (local result) derives wall-clock ratios and RSS from the raw exports.
An initial analysis attempt expected the older `times` field; analysis was adapted
to schema-v2 `measurements`/`summary` without changing or rerunning timing data.
Untimed `/usr/bin/time -l` logs also retain process RSS and renderer work counters.

| Case | Criterion ordered / single, ms | Process ordered/single ratio, sessions 1/2/3 | Mean process RSS ordered → single, MiB |
|---|---:|---|---|
| Tiny 32×32, retraced brush | 23.24 / 18.88 | 1.028 / 1.067 / 1.037 | 21.45–21.56 → 21.47–21.55 |
| Dense one layer, 512×256 | 29.18 / 26.76 | 1.059 / 1.124 / 1.051 | 24.55–24.80 → 24.72–24.98 |
| Sparse 4K canvas, 1024×768 viewport | 35.45 / 32.19 | 1.113 / 1.039 / 0.966 | 25.05–25.83 → 26.22–26.56 |
| Opacity-only edits, 512×256 | 26.55 / 24.18 | 1.181 / 1.089 / 1.094 | 20.71–21.05 → 20.69–20.92 |
| Five dense layers, fallback | 43.77 / 56.22 | 1.481 / 0.997 / 1.607 | 24.77–26.52 → 25.05–25.78 |
| Partial one-layer coverage, fallback | 30.03 / 29.04 | 1.053 / 0.909 / 1.122 | 24.85–26.04 → 24.47–25.52 |

A ratio above one means the contender's observed wall-clock mean is lower.
Criterion mean intervals do not overlap for tiny/dense gains or the five-layer
regression; large, opacity and partial intervals overlap. These intervals do not
control for changing machine conditions or establish historical/native latency.

For eligible cases, eight intermediate output allocations and eight compositor
submissions become zero; eight fused display submissions remain. Source uploads
are identical within each pair: 32,768 bytes tiny, 1,441,792 dense, 4,980,736 large,
and 524,288 opacity. The large case avoids 24 MiB cumulative intermediate buffer
requests across eight frames, not a claim that RSS drops by 24 MiB. Fallback
controls retain identical uploads, eight output allocations and respectively
forty/eight compositor submissions, with zero single-layer bypasses.

## Policy and limits

Keep library and native defaults on the ordered path. Hosts can explicitly enable
`with_single_layer_display(true)`; the native experiment uses
`PAINTCRAFT_GPU_SINGLE_LAYER=1`. No heuristic threshold is inferred from six cases.

The removed intermediate work is demonstrated, and tiny/dense statistical samples
favor it on this host. However, the large process case regresses in session three,
RSS does not show a general saving, and fallback controls vary substantially despite
unchanged rendering work. The five-layer process apparent gains disagree with its
statistical regression. These controls prevent attributing all measured changes to
the bypass or promoting a general throughput claim. Retain every result and the
correct explicit implementation; do not enable it globally from this run.

There were no intentionally concurrent task builds/tests/measurements. Background
load, affinity, power and thermal state were not controlled; other hosts/adapters
and long sessions remain unqualified. Disk checks before/after showed headroom,
not a continuously sampled minimum. Source/binary identities remained unchanged
through all postflights. [Validation evidence](../../../docs/validation/gpu-single-layer-2026-10-21/validation.json)
separately records full regressions, release builds, startup checks and development
compile corrections. This does not complete T12/T13/T21/T37 or the full roadmap.
