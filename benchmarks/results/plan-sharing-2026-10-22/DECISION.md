# Exact descriptor sharing: selected for retained memory, mixed timing

Keep exact consecutive descriptor sharing for its measured reduction in retained
metadata and repeated-diagonal RSS. Do not claim a general speedup. Repeated
diagonal library processes improve 2.2–5.1%, but other controls have opposing
sessions, large tails and calibration drift. The original mixed-lineage cost
against the sequential executor remains unresolved.

## Scope and identities

The candidate production source is `40be7e6763f2b56241992841cba96b6ac492936d`.
The measured tooling/evidence checkout is `4026fad`; intervening changes are the
benchmark tool and documentation, without production-code changes. Before
artifacts are the frozen planned library/CPU CLI from the
[complete batch experiment](../batch-preparation-2026-10-22/DECISION.md), not the old
sequential executor. The library driver itself is unchanged between the frozen
binaries. Thus this experiment measures the descriptor-sharing follow-up directly;
it does not erase the original whole-batch-versus-sequential regressions.

[manifest.json](manifest.json) pins all source files, four frozen binaries, fixture
bytes, matched CPU-only feature graph and platform/tool versions. The candidate
build/validation and exact before/shared preflight identities are in
[validation](../../../docs/validation/paint-plan-sharing-2026-10-22/validation.json).
This is one uncontrolled Apple Silicon macOS host without affinity, power or
thermal controls. No intentional concurrent task build/test/timing ran. Unrelated
host activity was not controlled; another project's Cargo process was observed
before timing. No observation is excluded or calibration-subtracted.

Seven original cases retain repeated geometry and blend work. Five varied controls
change actual RGBA descriptor values on every operation, keeping geometry, alpha,
coverage and blend mode. These are early color-mismatch controls, not worst-case
late-prefix equality walks. Mixed lineage and the default no-op control remain in
the original set only. Raw JSON/PCND/PNG/RGBA bytes are archived losslessly, so
serialization and all measured input identities remain recoverable.

Two frozen planned library binaries run 20 statistical samples per case, producing
**480 statistical samples** across twelve cases. Acquisition is outside these
loops; execution and destruction are inside. Library processes include input
acquisition and five repetitions (100 for the default control). Actual CPU CLI
processes include parsing, apply, recorded history and canonical file IO with an
input hash guard. Absolute library/CLI durations have different work scopes.

Three process sessions use forward/reverse/forward ordering, 20 runs and three
warmups: **2,880 workload observations plus 240 startup calibrations**, or **3,120
original schema-v2 observations**. Each of four distinct binaries retains its own
startup calibration. [audit.json](audit.json) records verified counts.

## Correctness and declared resources

All four checkpoints match. For every case, both library modes and both frozen
binaries produce identical document semantics; the driver also checks selection,
aliases and decoded peaks against its frozen sequential executor. The two actual
CLI archives are byte-identical. Independent raw PCND pixel reconstruction gives
the same canonical document semantic SHA as the real library output. All source,
fixture and executable hashes pass postflight. Original and compact correctness
JSON hashes are retained, as are every original Hyperfine export and its compact
value-equivalent view. [compressed-artifacts.json](compressed-artifacts.json)
records uncompressed/archive hashes and lengths for logs, inputs and outputs.

Declared metadata peaks for repeated cases change as follows:

| Case | Before bytes | Shared bytes |
| --- | ---: | ---: |
| Horizontal pencil | 23,568 | 7,888 |
| Diagonal pencil | 4,111,632 | 97,552 |
| Smooth brush | 265,488 | 14,608 |
| Smooth eraser | 265,488 | 14,608 |
| Mixed lineage | 36,728 | 36,728 |
| Default control | 1,840 | 1,840 |
| Dense | 24,072 | 8,392 |

Varied controls retain equal before/shared metadata peaks. Every decoded peak is
unchanged. These accounts exclude allocator/hash overhead and do not establish
physical ceilings. Repeated-diagonal median library RSS changes from 18.96–21.87
MiB to 6.16–6.73 MiB. Its library CPU means change from 55.76–56.51 ms to
53.83–54.25 ms, while retired instruction means rise from about 783 million to
794 million. This is not an instruction-count optimization. Other RSS cases are
mixed; there is no universal process-memory claim.

## Statistical means

Milliseconds per planned library edit; brackets are 95% confidence intervals of
the mean. Point-estimate changes are not promoted to universal improvements.
Raw samples and [analysis-kernels.json](analysis-kernels.json) remain available.

| Dataset / case | Before mean [CI] | Shared mean [CI] | Shared time change |
| --- | ---: | ---: | ---: |
| repeated / horizontal_pencil | 1.4732 [1.4664, 1.4816] | 1.4610 [1.4529, 1.4699] | -0.8% |
| repeated / diagonal_pencil | 11.5400 [11.0411, 12.1292] | 10.6624 [10.4477, 10.9797] | -7.6% |
| repeated / smooth_brush | 13.4658 [13.0430, 14.2149] | 12.9967 [12.9258, 13.0936] | -3.5% |
| repeated / smooth_eraser | 3.8514 [3.7241, 4.0727] | 3.6787 [3.6600, 3.6999] | -4.5% |
| repeated / mixed_lineage | 3.2520 [3.0195, 3.5460] | 2.9093 [2.8872, 2.9358] | -10.5% |
| repeated / default_control | 0.0241 [0.0241, 0.0243] | 0.0240 [0.0239, 0.0241] | -0.6% |
| repeated / dense | 1.4544 [1.4458, 1.4651] | 1.4565 [1.4474, 1.4672] | +0.1% |
| varied / horizontal_pencil | 1.4852 [1.4512, 1.5384] | 1.6899 [1.4829, 2.0312] | +13.8% |
| varied / diagonal_pencil | 10.8303 [10.7713, 10.8953] | 10.8034 [10.7404, 10.8763] | -0.2% |
| varied / smooth_brush | 14.8367 [14.3136, 15.5455] | 14.3264 [14.0349, 14.5713] | -3.4% |
| varied / smooth_eraser | 5.4354 [4.8820, 6.2313] | 5.1732 [4.9067, 5.4077] | -4.8% |
| varied / dense | 1.4877 [1.4687, 1.5099] | 1.4799 [1.4631, 1.5015] | -0.5% |

The repeated horizontal intervals overlap; repeated diagonal improves its mean
in this statistical run. Varied horizontal has a +13.8% point estimate with wide,
overlapping intervals. The mixed-lineage mean improves despite unchanged declared
storage, while its actual CLI/library processes remain mixed. Do not attribute
all statistical changes to descriptor reuse or infer a universal miss penalty.

## Process means by session

Each cell is before / shared milliseconds and shared wall-time change. CPU,
instructions, cycles, RSS, median and full tails remain in the corresponding
`analysis-{dataset}-{library,cli}.json` and original schema-v2 exports.

### Repeated library

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 9.481 / 9.445 (-0.4%) | 9.610 / 11.464 (+19.3%) | 9.780 / 10.096 (+3.2%) |
| diagonal_pencil | 57.683 / 56.037 (-2.9%) | 57.183 / 55.934 (-2.2%) | 58.169 / 55.187 (-5.1%) |
| smooth_brush | 68.706 / 68.136 (-0.8%) | 68.237 / 69.507 (+1.9%) | 68.643 / 67.974 (-1.0%) |
| smooth_eraser | 21.929 / 23.122 (+5.4%) | 21.084 / 20.695 (-1.8%) | 22.441 / 21.422 (-4.5%) |
| mixed_lineage | 16.781 / 17.313 (+3.2%) | 16.058 / 16.435 (+2.3%) | 16.813 / 16.819 (+0.0%) |
| default_control | 4.418 / 4.734 (+7.2%) | 4.306 / 4.246 (-1.4%) | 4.919 / 4.455 (-9.4%) |
| dense | 11.836 / 20.094 (+69.8%) | 13.114 / 11.640 (-11.2%) | 11.603 / 11.590 (-0.1%) |

### Repeated cli

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 24.397 / 23.405 (-4.1%) | 20.420 / 20.261 (-0.8%) | 20.978 / 20.420 (-2.7%) |
| diagonal_pencil | 41.226 / 31.539 (-23.5%) | 29.908 / 29.697 (-0.7%) | 32.070 / 31.411 (-2.1%) |
| smooth_brush | 32.807 / 33.158 (+1.1%) | 33.005 / 32.015 (-3.0%) | 32.812 / 33.080 (+0.8%) |
| smooth_eraser | 22.461 / 24.863 (+10.7%) | 23.317 / 22.523 (-3.4%) | 23.058 / 23.216 (+0.7%) |
| mixed_lineage | 16.673 / 21.037 (+26.2%) | 17.518 / 17.222 (-1.7%) | 17.606 / 17.561 (-0.3%) |
| default_control | 14.316 / 12.856 (-10.2%) | 12.876 / 12.670 (-1.6%) | 13.016 / 12.613 (-3.1%) |
| dense | 41.369 / 49.411 (+19.4%) | 43.712 / 43.465 (-0.6%) | 42.212 / 40.843 (-3.2%) |

### Varied library

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 10.441 / 10.158 (-2.7%) | 10.030 / 17.662 (+76.1%) | 9.798 / 9.781 (-0.2%) |
| diagonal_pencil | 58.199 / 60.357 (+3.7%) | 59.863 / 58.176 (-2.8%) | 57.325 / 57.562 (+0.4%) |
| smooth_brush | 68.379 / 68.032 (-0.5%) | 71.147 / 72.933 (+2.5%) | 68.257 / 67.498 (-1.1%) |
| smooth_eraser | 21.886 / 21.594 (-1.3%) | 21.479 / 21.827 (+1.6%) | 21.191 / 21.149 (-0.2%) |
| dense | 13.195 / 11.554 (-12.4%) | 11.322 / 11.598 (+2.4%) | 11.246 / 11.054 (-1.7%) |

### Varied cli

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 22.359 / 22.769 (+1.8%) | 21.023 / 22.071 (+5.0%) | 21.217 / 21.621 (+1.9%) |
| diagonal_pencil | 30.341 / 30.471 (+0.4%) | 31.473 / 30.308 (-3.7%) | 30.417 / 30.544 (+0.4%) |
| smooth_brush | 33.015 / 32.999 (-0.0%) | 39.161 / 34.084 (-13.0%) | 32.714 / 32.350 (-1.1%) |
| smooth_eraser | 26.552 / 23.342 (-12.1%) | 28.600 / 24.604 (-14.0%) | 23.024 / 23.314 (+1.3%) |
| dense | 41.690 / 41.647 (-0.1%) | 41.830 / 42.033 (+0.5%) | 40.602 / 46.544 (+14.6%) |

Repeated-diagonal library wall time improves in all sessions, but its CLI means
vary from a 23.5% improvement to 0.7–2.1% improvements. The repeated dense library
first session slows 69.8%: shared mean/median/max are 20.094/14.167/62.401 ms,
versus 11.836/11.567/17.609 ms before. Later sessions are 11.2% faster and tied.

Varied-horizontal library session two slows 76.1%, with shared mean/median/max
17.662/11.038/48.887 ms versus 10.030/10.098/10.518 ms before; other sessions
are 2.7% faster and tied. Its CPU means also rise in that slow session. The varied
horizontal CLI is 1.8–5.0% slower across sessions. These slow cases are retained,
not dismissed as noise. The varied dense CLI's final session is 14.6% slower.
Mixed-lineage CLI session one slows 26.2%, while later sessions improve slightly.

## Startup calibration and limits

Before/shared mean startup milliseconds, in artifact identity order even when
measurement order was reversed:

| Artifact | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| library | 1.399 / 1.634 | 1.896 / 1.496 | 6.588 / 5.286 |
| cli | 1.296 / 1.581 | 1.478 / 1.552 | 5.323 / 2.357 |

Late startup drift is substantial, as are several workload tails. Both raw CPU
and wall evidence remain. Calibration is not subtracted and no native event/display
or sustained-input latency conclusion follows. This follow-up does not qualify
worst-case late equality misses, physical budgets, global resource admission or
cross-platform performance. The original sequential executor is still faster than
complete preparation on the mixed-lineage fixture; optimizing that source-reading
path and the broader feature roadmap remain open.

Select the sharing change for its exact semantics and bounded repeated-geometry
retention. Keep all unfavorable controls and scope restrictions attached to the
choice. Do not turn this limited memory result into an unrestricted throughput
claim or mark the broader performance roadmap complete.
