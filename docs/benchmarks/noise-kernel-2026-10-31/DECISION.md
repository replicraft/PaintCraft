# Native noise kernel baseline

The qualified scalar implementation has measured native costs. No alternative
algorithm was compared, and these results do not qualify editor responsiveness,
Wasm latency, cancellation acknowledgment or browser execution slices.

Apple M1 Pro, AC power at 100%; other desktop work was active and thermal
telemetry was unavailable. The measured source was the qualified noise foundation
plus the exact benchmark working-tree manifest. Criterion 0.8.2 used three ordered
sessions, 100 flat samples per condition, one-second warmups and two-second
requested measurement windows. Actual calibrated iterations and all raw samples
remain recorded. Hyperfine 2.0.0 separately measured whole-process costs with
20 runs and three warmups per condition in each session.

The table gives the range of the three session means, in milliseconds. These
ranges are not pooled confidence intervals. Amount is 10, seed is 1; source RGB
varies procedurally and alpha is opaque. Caller input/output buffers are allocated
before kernel timing. Each row uses its original source coordinate, including
the input plane's repeated 256-row storage period.

| Canvas | Distribution | Mode | Kernel mean range (ms) |
|---|---|---|---:|
| 1000×486 | Uniform | Monochrome | 2.929 to 2.953 |
| 1000×486 | Uniform | Color | 3.547 to 3.569 |
| 1000×486 | Gaussian | Monochrome | 21.843 to 22.248 |
| 1000×486 | Gaussian | Color | 52.976 to 55.196 |
| 1920×1080 | Uniform | Monochrome | 12.546 to 13.025 |
| 1920×1080 | Uniform | Color | 15.103 to 15.795 |
| 1920×1080 | Gaussian | Monochrome | 92.936 to 97.331 |
| 1920×1080 | Gaussian | Color | 226.525 to 230.178 |

Complete-process Gaussian/color costs were 59.047 to 59.348 ms for the default
canvas and 239.305 to 253.315 ms for 1080p. Process timing includes construction,
independent probes, output hashing and teardown. Identity was a separate
calibration workload, never subtracted: its kernel means were 1.685 to 1.793 ms
for the default canvas and 7.202 to 8.018 ms for 1080p across the conditions.
Different distributions and mono/color choices produce different images; they
are workload conditions, not interchangeable contenders or an algorithm winner.

## Evidence and limits

All 27 serial stages pass. The records retain 4,800 Criterion samples, including
365 reported outliers, and 960 Hyperfine measurements with 144 warmups. Full
per-session mean/median 95% confidence intervals, raw extrema and Tukey fences
are in `docs/validation/noise-latency/measurement-1/analysis.json`; no samples were
filtered from the source data. The final inventory binds 818 artifacts and 303
fixed inputs, including the original 288 build/correctness inputs and actual
optimized executables. The original parent tool session expired before the final
poll; the runner records all subprocess exits zero and a successful final
closeout. No returned parent exit code or timed rerun is invented.

Receipt `docs/validation/noise-latency/measurement-1/final-freeze.json`, SHA256
`16ddd1e7cac58b210ec3558cc26f2e5a18b03259e7d628279560e1d4b345c2fa`,
retains the earlier metadata/configuration failures and derived compiler-report
normalization separately. None changed the math or required a timed rerun.
The build/correctness receipt is
`docs/validation/noise-latency/build-correctness/final-freeze.json`, SHA256
`f334cd2681c73e6447f183558cf1fa409bf08a2831d2dfbf48532a9551471e67`.
The primary eight workloads pass independent procedural probes, source/alpha
preservation, quoted/actual work and actual callback counts. The original
4,380-pixel/26,280-mask numerical qualification remains separate. Optional
2000×1000 workloads were not measured and are not called 1080p.

Keep the scalar implementation as the measured baseline. Normal canvas admission
still needs the [scoped workload contract](../../architecture/NOISE_WORKLOAD_CONTRACT.md).
Actual Wasm measurements and real Native/Main/Worker adoption, latency and
cancellation gates remain open. New execution-slice policies need their own
versioned allowance and actual browser evidence; native kernel time alone cannot
authorize a larger synchronous browser call.
