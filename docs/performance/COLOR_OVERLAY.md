# Color Overlay measurements

Recorded on 2026-11-11 on an Apple M1 Pro, macOS arm64, using the normal optimized build and unchanged source/artifacts. These are normal-workstation observations, not controlled hardware or interactive-latency qualification.

## Decision

Keep the correct prepared service as a provisional CPU implementation. Fourteen of 21 matched cases show a consistent practical benefit; seven are mixed or inconclusive. Four cases carry potential-regression flags and five reverse direction between sessions. There is no overall winner and no blanket prepared-faster qualification. Tiny-row performance and the mixed Screen boundary remain open.

A future row-method inlining experiment is source-only and unadopted. Assembly inspection found that the constructor was already inlined in this artifact; constructor call overhead is not an established explanation for tiny-row results.

## Method

The confirmatory policy was frozen before this series: symmetric 5% practical boundaries, three forward/reverse/forward sessions, and casewise 95% independent bootstrap ratio intervals from 100,000 replicates. Each case must meet its direction and magnitude criteria in all three sessions. These are approximate casewise intervals, with no familywise or aggregate significance claim. All samples, mixed outcomes and flags are retained.

The earlier exploratory series had no practical threshold declared before timing. That setup error is preserved, and none of its samples enter this decision.

Both real row APIs receive the same immutable input, timed copy, private output, bounded 64-pixel partitions and logical callback counts. Prepared formation occurs inside timing for every bounded row. This compares the original matched wrapper with the prepared wrapper; it does not establish a minimum-cost scalar implementation. Full pixel/hash and callback checks precede registration.

Criterion 0.8.2 uses 30 samples, 1 second warmup and 2 seconds measurement for each kernel and renderer case. The separate factory/TLS calibration retains 100 samples, 3 seconds warmup and 5 seconds measurement. No calibration is subtracted. Hyperfine 2.0.0 uses 20 completed runs and three warmups, direct process execution and one requested render worker. All outputs are independently checked.

## Complete matched case table

Ratio is prepared mean latency / original scalar mean latency. Values below 1 favor prepared. Brackets contain the frozen casewise 95% ratio interval. Modes 0/1/2 mean Normal/Multiply/Screen. Opacity is a byte value. Displayed ratios are rounded; classification uses unrounded data.

| Case | Forward 1 | Reverse 2 | Forward 3 | Outcome | Flags |
| --- | --- | --- | --- | --- | --- |
| mode0-n0-opacity0 | 0.933 [0.825, 1.056] | 0.905 [0.835, 0.961] | 1.007 [0.997, 1.018] | inconclusive-or-mixed | direction reversal |
| mode0-n1-opacity255 | 0.929 [0.791, 1.098] | 1.117 [0.942, 1.250] | 1.307 [1.092, 1.640] | inconclusive-or-mixed | potential regression, direction reversal |
| mode0-n63-opacity1 | 0.286 [0.266, 0.317] | 0.279 [0.259, 0.312] | 0.108 [0.088, 0.135] | consistent-practical-benefit | none |
| mode0-n64-opacity127 | 0.275 [0.256, 0.301] | 0.260 [0.254, 0.268] | 0.311 [0.272, 0.357] | consistent-practical-benefit | none |
| mode0-n65-opacity128 | 0.420 [0.372, 0.475] | 0.312 [0.288, 0.341] | 0.270 [0.265, 0.278] | consistent-practical-benefit | none |
| mode0-n4096-opacity254 | 0.225 [0.218, 0.232] | 0.243 [0.225, 0.260] | 0.259 [0.219, 0.307] | consistent-practical-benefit | none |
| mode0-n486000-opacity128 | 0.404 [0.303, 0.530] | 0.228 [0.213, 0.243] | 0.244 [0.239, 0.250] | consistent-practical-benefit | none |
| mode1-n0-opacity0 | 0.991 [0.848, 1.107] | 1.145 [1.132, 1.158] | 4.579 [3.292, 6.079] | inconclusive-or-mixed | potential regression, direction reversal |
| mode1-n1-opacity255 | 0.974 [0.865, 1.048] | 0.994 [0.945, 1.037] | 0.930 [0.583, 1.519] | inconclusive-or-mixed | none |
| mode1-n63-opacity1 | 0.215 [0.203, 0.230] | 0.183 [0.164, 0.196] | 0.166 [0.106, 0.264] | consistent-practical-benefit | none |
| mode1-n64-opacity127 | 0.211 [0.194, 0.236] | 0.184 [0.171, 0.193] | 0.267 [0.186, 0.387] | consistent-practical-benefit | none |
| mode1-n65-opacity128 | 0.226 [0.201, 0.252] | 0.204 [0.185, 0.216] | 0.123 [0.081, 0.187] | consistent-practical-benefit | none |
| mode1-n4096-opacity254 | 0.091 [0.075, 0.111] | 0.183 [0.178, 0.189] | 0.184 [0.136, 0.247] | consistent-practical-benefit | none |
| mode1-n486000-opacity128 | 0.201 [0.194, 0.208] | 0.201 [0.177, 0.233] | 0.230 [0.176, 0.300] | consistent-practical-benefit | none |
| mode2-n0-opacity0 | 2.582 [1.901, 3.417] | 1.075 [0.966, 1.153] | 1.504 [1.133, 2.014] | inconclusive-or-mixed | potential regression |
| mode2-n1-opacity255 | 0.665 [0.447, 1.029] | 0.989 [0.867, 1.095] | 1.429 [1.060, 1.913] | inconclusive-or-mixed | potential regression, direction reversal |
| mode2-n63-opacity1 | 0.107 [0.085, 0.135] | 0.194 [0.168, 0.222] | 0.392 [0.296, 0.513] | consistent-practical-benefit | none |
| mode2-n64-opacity127 | 0.351 [0.255, 0.470] | 0.212 [0.173, 0.261] | 0.186 [0.134, 0.254] | consistent-practical-benefit | none |
| mode2-n65-opacity128 | 1.427 [0.721, 2.411] | 0.248 [0.203, 0.316] | 0.134 [0.095, 0.191] | inconclusive-or-mixed | direction reversal |
| mode2-n4096-opacity254 | 0.154 [0.086, 0.265] | 0.184 [0.160, 0.214] | 0.209 [0.163, 0.277] | consistent-practical-benefit | none |
| mode2-n486000-opacity128 | 0.106 [0.072, 0.159] | 0.218 [0.176, 0.270] | 0.190 [0.143, 0.257] | consistent-practical-benefit | none |

## Completed CPU jobs

These are absolute whole-job means in milliseconds, including original admission/factory creation, TLS installation, real rendering, output destruction and retirement. Each job uses the original 8 MiB secondary workspace, 268,435,456 work limit and prior work debt 17; source fixture storage is separate and untimed. No scalar whole-job contender was measured. These values do not imply a kernel-to-application speedup.

| Workload | Forward 1 ms | Reverse 2 ms | Forward 3 ms |
| --- | --- | --- | --- |
| mode0-1000x486-uniform-style | 9.864 | 3.702 | 2.287 |
| mode0-257x129-mixed-filter-mask-style-ownmask-opacity-multiply | 4.095 | 1.485 | 4.152 |
| mode1-1000x486-uniform-style | 8.595 | 2.031 | 2.267 |
| mode1-257x129-mixed-filter-mask-style-ownmask-opacity-multiply | 4.778 | 1.277 | 1.312 |
| mode2-1000x486-uniform-style | 6.135 | 2.010 | 2.292 |
| mode2-257x129-mixed-filter-mask-style-ownmask-opacity-multiply | 2.588 | 1.255 | 1.596 |

Raw distributions are broad on this workstation. The source report retains every sample, standard deviation and descriptive percentile; a whole-job mean is not an event-level p95/p99 or scheduling promise.

## Completed CLI exports

Absolute PCND read/render/PNG encode/write process means, in milliseconds. Cases produce different outputs and are not speedup pairs.

| Workload | Forward 1 ms | Reverse 2 ms | Forward 3 ms |
| --- | --- | --- | --- |
| compound-startup-boundary | 15.001 | 16.365 | 33.244 |
| default-mode0 | 17.125 | 23.810 | 37.521 |
| default-mode1 | 19.850 | 18.371 | 27.078 |
| default-mode2 | 18.529 | 16.849 | 67.807 |

## Separate calibrations

| Session | Factory/TLS ns | CLI version process ms |
| --- | --- | --- |
| 1-forward | 1338.3 | 3.775 |
| 2-reverse | 1142.1 | 3.486 |
| 3-forward | 1504.8 | 10.819 |

These are separate calibration paths, not exact component attribution. They are never subtracted from row, renderer or process timings.

## Evidence and limits

The benchmark source is committed as `a4adcfac`; the same file bytes were present at optimized build time. The optimized build passed both untimed benchmark orders and four independently decoded PNG preflights. Source and artifact guards remained unchanged through all six confirmatory runs and analysis. An independent review reproduced all 63 raw ratios/classifications and one complete 100,000-replicate interval.

The original analyzer assumed Hyperfine 1 JSON and failed verification before resampling. Its failed attempt is retained. The corrected reader strictly verifies schema 2, the wall-clock primary metric, seconds, 20 completed measurements and zero exit codes, preserving all other raw metrics. See the [official Hyperfine 2 format](https://github.com/sharkdp/hyperfine/releases/tag/v2.0.0).

Criterion raw archives are authenticated by producer receipts. Hyperfine raw exports have a separate Root post-completion readback binding; this is explicitly not a producer-time attestation. The earlier overwritten source-review receipt is also retained as an explicit evidence gap; the current independent review records are separate.

Local ignored evidence is under `docs/validation/color-overlay-composition/`, including `confirmatory-analysis-full-attempt-2/analysis.json`, its complete report, the six `confirmatory-v1-*` run folders and `confirmatory-hyperfine-raw-readback.json`. Full compiler-product records, actual local source archive and all raw metric records remain there.

| Identity | SHA-256 |
| --- | --- |
| Decision policy | `01c4439b84a9f14ad97de7e465726c17233a5400636a92990da4fdf1621da6cf` |
| Optimized build receipt | `154166752f49a2c263700aaacb1cf8e776d41f7c243291fa0fee2ace9b389280` |
| Criterion artifact | `178b021009ed25eb3aa23db7911dfa70621e244b168892b74cbf15cb6804624d` |
| CLI artifact | `90dfef4c29bfb4e106a79771b309c572e63314ab3dd0518bc55edad365d6b49f` |
| Analysis JSON | `bcf5b722065386f97fcd3557d9bcbdc7be08b65ae8496818991c50404a73deab` |
| Independent output review | `ff3e52faad0dd4755d2f0bd05e76300fb0ff5e35aae8b99f83eb03b587c748a5` |

This qualifies only these casewise observations on this recorded host. It does not qualify physical memory, other CPU/ISA/compiler combinations, GPU style shaders, browser style editing or interactive latency. Hyperfine memory/counter observations remain raw evidence and do not establish memory-budget compliance. Fresh production-browser qualification and the full roadmap remain open.
