# Scoped CPU layer blend measurements

Measured on Apple M1 Pro, macOS, Rust 1.99.0, native release/bench opt3 with line-table debug information. Actual Cargo profile records and compiled feature sets are retained. Background user/OS load and thermal conditions were uncontrolled.

This compares the actual public regional renderer, including caller admission, output allocation and destruction. It is not an isolated-kernel, GPU, browser, interactive-latency, frame-rate or RSS measurement.

Both contenders produce the same independently specified pixels. A default opaque background certifies the fixed-channel path; one unchanged resident tile entirely outside the requested ROI forces the general path. That residency and lookup difference remains part of the comparison. Process measurements additionally include fixture construction.

All eight release scenes passed complete-byte comparisons before timing. Each frame uses the existing explicit 64 MiB intermediate / one-billion-work-unit caller workspace. Counters are fresh between completed standalone frames and never reset inside a frame.

## Separate-session results

Each cell gives the range of three session means, without pooling sessions. Ratios are general divided by opaque.

| Measurement | Requested pixels | Mode | Opaque ms | General ms | Ratio |
| --- | --- | --- | ---: | ---: | ---: |
| Criterion | 1000 × 486 | Multiply | 2.214 to 3.181 | 3.949 to 5.864 | 1.724 to 1.843× |
| Criterion | 1000 × 486 | Screen | 2.243 to 3.835 | 3.922 to 4.502 | 1.029 to 1.855× |
| Criterion | 1920 × 1080 | Multiply | 10.279 to 11.897 | 16.862 to 22.089 | 1.612 to 2.025× |
| Criterion | 1920 × 1080 | Screen | 10.038 to 19.326 | 17.426 to 36.182 | 1.575 to 1.872× |
| Process | 1000 × 486 | Multiply | 4.018 to 5.108 | 5.592 to 5.647 | 1.095 to 1.403× |
| Process | 1000 × 486 | Screen | 3.634 to 3.954 | 5.332 to 5.633 | 1.405 to 1.467× |
| Process | 1920 × 1080 | Multiply | 11.741 to 12.000 | 19.105 to 19.192 | 1.594 to 1.627× |
| Process | 1920 × 1080 | Screen | 11.711 to 12.618 | 19.124 to 19.314 | 1.518 to 1.633× |

At 1080p, all three process sessions favor the opaque path: approximately 11.7 to 12.6 ms versus 19.1 to 19.3 ms, or 1.52 to 1.63× by separate-session mean ratios. These observations support keeping the exact opaque specialization in this native CPU baseline; they establish no universal speedup or UI target.

Two comparisons remain unresolved because the descriptive 95% ratio interval includes one: default Screen in Criterion session 1, and default Multiply in process session 1. These are not equivalence proofs. All outliers and all original observations remain; no observation is excluded. Absolute times also vary across sessions, so the ranges must not be presented as one homogeneous timing distribution.

The logical work-price ratio (32 extra opaque units versus 256 general-call units) is an accounting policy, not an eightfold runtime speed claim.

## Reproduction and evidence

- Criterion: 100 Flat samples per contender, one-second warmup and at least two seconds requested measurement; three forward/reverse/forward sessions. All 27 original CSV/sample exports are retained.
- Process: 20 runs and three warmups per contender in each of three serial sessions, using the existing Hyperfine runner.
- Four process calibration pairs per session and the separate Criterion caller-factory calibration are retained separately and never subtracted from completed rendering.
- Ratio intervals use 10,000 deterministic bootstrap resamples of independent contender means within each session. They do not cover thermal/time-order/host effects or multiple comparisons. Sessions remain separate.

The build bound 708 source/config/driver inputs selected from actual Cargo-emitted local packages plus shared assets. All 714 final defining inputs/artifacts matched before/after all 27 phases. The actual compiler-emitted example and benchmark binaries were selected from Cargo JSON, not guessed filenames.

Raw run: `benchmarks/results/layer-blend-cpu-20261102-1/`. Build and eight-case verification: `docs/validation/layer-blend-cpu-benchmark-integration/build-1/`. Descriptive analysis, original bootstrap helper and raw manifest hashes: `docs/validation/layer-blend-cpu-benchmark-analysis/result-1/`. These local evidence directories are intentionally ignored; preserve them when reproducing a qualification.

Source manifest SHA256: `480275adf2c586f25ec23e417879b7caf91d67874803d0448bff1cacb3c421ed`. Numerical expected output comes from the separately frozen Fraction generator in `crates/services/paintcraft-render/tests/layer-blend-benchmark-fixtures/`.

The committed runner is `benchmarks/tools/run_layer_blend.py`; it consumes prebuilt exact Cargo artifacts and a compiled-graph manifest, defaults to plan-only, validates inputs before/after each phase and requires explicit execution. These measurements precede the separately [qualified native GPU composition](../architecture/LAYER_BLEND_FOUNDATION.md#scoped-gpu-composition); they measure CPU rendering only. Frontend and browser blend adoption remain open.
