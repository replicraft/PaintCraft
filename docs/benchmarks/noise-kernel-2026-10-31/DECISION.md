# Noise kernel measurements

The qualified scalar implementation has measured native and actual Chrome Wasm
call costs. No alternative algorithm was compared, and these results do not
qualify editor responsiveness, cancellation acknowledgment or Worker execution
slices.

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

## Actual Chrome Wasm call boundary

On Chrome 156.0.8078.79, the same Rust `Frame` helper passed all 432 independent
scalar probes and all 16 complete-frame native/Wasm SHA256 comparisons before
any timed sample. Counted correctness calls also matched source preservation,
alpha, quoted/actual work and 8,748 callbacks for the default canvas or 34,560
for 1080p. Callback counting was disabled during timing; the same actual atomic
cancellation checks still executed. No JavaScript noise arithmetic or fake Wasm
imports were used.

Three sessions used forward, reverse and rotated condition order, three warmups
and 100 raw samples per condition: 4,800 samples retained. Each sample times
`performance.now()` → one actual Rust `Frame.run(false)` → `performance.now()`.
Setup, probes and hashes are outside that interval; JS/Wasm call-boundary cost
remains included. These are separate measurements from native Criterion and
Hyperfine, not interchangeable benchmark boundaries or algorithm contenders.

The table reports ranges across the three per-session means and p95 values,
plus the largest observed sample. No pooled confidence interval or independent
sample assumption is made for these sequential browser observations.

| Canvas | Distribution | Mode | Mean range (ms) | p95 range (ms) | Maximum (ms) |
|---|---|---|---:|---:|---:|
| 1000×486 | Uniform | Monochrome | 3.520 to 3.669 | 3.700 to 3.800 | 3.900 |
| 1000×486 | Uniform | Color | 4.120 to 4.200 | 4.300 to 4.400 | 4.600 |
| 1000×486 | Gaussian | Monochrome | 34.046 to 34.343 | 35.000 to 35.600 | 36.700 |
| 1000×486 | Gaussian | Color | 79.360 to 79.981 | 81.900 to 83.720 | 91.700 |
| 1920×1080 | Uniform | Monochrome | 14.970 to 15.192 | 15.305 to 15.805 | 16.600 |
| 1920×1080 | Uniform | Color | 17.522 to 17.675 | 18.100 to 18.500 | 18.900 |
| 1920×1080 | Gaussian | Monochrome | 145.204 to 146.488 | 148.210 to 152.335 | 154.700 |
| 1920×1080 | Gaussian | Color | 337.982 to 344.455 | 344.100 to 353.900 | 374.100 |

Identity remained a separate calibration, not subtracted: mean ranges across
conditions were 1.374 to 1.493 ms for the default canvas and 5.840 to 6.101 ms for
1080p. The observed minimum positive browser clock tick was approximately 0.1 ms;
`crossOriginIsolated` was false. This is an observation, not a timer-accuracy or
fixed-resolution guarantee. Active desktop processes, AC power at 100% and
unavailable thermal telemetry remain environmental limitations. A read-only
remote CI watch was active during this browser measurement; no local compiler
or test job overlapped it. CI success is a separate authoritative result.

Actual nonshared Wasm memory began at 2,097,152 bytes and reached a high-water of
12,386,304 bytes, below its 16,777,216-byte ceiling. Each previous full frame was
dropped before allocating the next. Freed allocator pages did not shrink linear
memory, so this is neither live-payload equivalence nor a global RSS claim.
The sole import was real bindgen initialization, with pinned bindgen 0.2.129 and
libm 0.2.16 using only `unstable-public-internals`.

The original first build stopped at a harness artifact-selection assertion:
Core/Effects library builds do not emit the shared helper's dev-dependency
artifacts. The reviewed second attempt explicitly built the existing locked
serde_json 1.0.154 and sha2 0.11.1 packages, then passed build/inventory. No product
math, kernel, `Frame`, numeric expectation or native digest changed. The actual
Chrome semantic/timing attempt passed on its first run. Browser receipt
`docs/validation/noise-latency/wasm/browser-attempt-wRzqsU/final.json`, SHA256
`b5a3f78d035c9d659bf5ab8501111588297b5051a11acd5ed889e35cf3da8ff2`,
retains raw samples and source checks. Full per-condition means, medians, tails,
maxima and retained outlier counts are in that directory's `analysis.json`.
The build receipt SHA256 is
`fc3ff5f328c6b548503d2047abac852c0758a457419c64cfaaeeaebe4740c5e6`.
Its 298 captured inputs were exact through the completed proof boundary, before
subsequent integration promotion; this is not a current-source equivalence claim.
The earlier native registry metadata publication is an explicit bound delta,
not a rewritten native preimage.

Keep the scalar implementation as the measured baseline. Normal canvas admission
still needs the [scoped workload contract](../../architecture/NOISE_WORKLOAD_CONTRACT.md).
Real Native/Main/Worker adoption, delivery latency and cancellation gates remain
open. A candidate Noise-only `2^27` step allowance is a work-price policy, not a
measured ≤50 ms guarantee. Reverse halos and repeated prefix evaluation must be
charged truthfully within the proposed `2^35` cumulative Noise grant, while old
base-work/byte caps remain unchanged. Actual Source6 Worker slicing and retirement
acknowledgment need their own browser qualification; neither native kernel time
nor these standalone Wasm full-frame calls close that gate.
