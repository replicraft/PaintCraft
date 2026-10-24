# CPU sparse row-span decision (T12 / BF29)

## Implementation and disposition

Retain the borrowed row-span compositor for the existing flat RGBA8 CPU policy.
It removes the scratch-row copy and specializes implicit transparent/opaque runs.
It does not change tile storage, integer rounding, document publication, cancellation
or GPU dispatch. This is local evidence for this CPU implementation, not completion
of the dirty-cache case or any release qualification.

The production implementation is committed as `4adc8b4`; timing occurred before
commit with the identical source bytes verified by the artifact hashes.
The implementation changes `paintcraft-raster` and `paintcraft-render`; the
whole-document regression lives in `paintcraft-formats`. Divan 0.1.21 is the sole
new dependency, used only for development measurements. Root workspace dependencies
and the existing grouped crate boundaries are preserved.

## Method and provenance

Apple M1 Pro, 32 GiB RAM, macOS 14.7.2, Rust 1.99.0/LLVM 23.1.1, optimized
release builds, Divan 0.1.21 and Hyperfine 2.0.0. Ten logical CPUs were available;
the tested compositor is single-threaded. Power/thermal state and background load
were not controlled. Source parent is 736ad27 plus the exact contender/generator
file hashes in each config; the prebuilt binary and lockfile are hashed as well.

Each process comparison has three sessions (forward/reverse/forward), 20 timed
runs and three warmups per variant per session. Pooling therefore uses 60 runs
per variant per case. Output equivalence is checked before every timing run.
Setup and final buffer destruction are included in process times; setup is outside
Divan timing, where result destruction and allocation profiling are included.

The frozen baseline is the actual scalar row-copy implementation from 95db5d0.
The dense fixture populates all tiles; uniform5 is a separate uniform-translucent
case. Sparse patches may cross tile boundaries. No unlike workload average is used.

## Complete-process results

Times are medians per process in milliseconds, including fixture setup and shutdown.
The iteration count is identical between variants. RSS columns are medians of
per-process peak RSS in MiB from Hyperfine, not live-tile byte accounting.

| Case | Renders/process | Baseline ms | Span ms | Ratio | Peak RSS baseline/span MiB |
|---|---:|---:|---:|---:|---:|
| tiny | 65536 | 78.94 | 62.78 | 1.26× | 1.9 / 1.7 |
| opaque1 | 256 | 1226.41 | 78.55 | 15.61× | 33.7 / 25.5 |
| sparse1 | 128 | 236.55 | 42.81 | 5.53× | 34.0 / 21.7 |
| sparse5 | 64 | 523.16 | 51.50 | 10.16× | 35.0 / 26.7 |
| sparse20 | 16 | 504.89 | 44.79 | 11.27× | 30.7 / 22.4 |
| sparse100 | 4 | 657.00 | 79.32 | 8.28× | 39.4 / 39.3 |
| dense5 | 32 | 694.04 | 703.57 | 0.99× | 55.4 / 54.5 |
| uniform5 | 8 | 191.78 | 147.46 | 1.30× | 22.8 / 18.8 |
| clipped100 | 256 | 90.66 | 88.49 | 1.02× | 51.6 / 51.4 |

Dense mixed-alpha is approximately 1.4% slower by pooled process median; this
is inconclusive within observed run variation. Clipped layers have no meaningful
speedup. Preserve both controls rather than claiming a universal win. The opaque
case is an implicit solid background, not a fully occupied imported image.

`process-summary.csv` also records mean/stddev/min/max times, amortized throughput,
peak RSS, CPU time, cycles and instructions. These are process metrics, not frame
latency percentiles or per-event p99. Large scheduling outliers are retained.

## Steady-state and allocations

| Case | Baseline median | Span median | Span throughput |
|---|---:|---:|---:|
| opaque1 | 5.150 ms | 0.2657 ms | 3,763 frames/s |
| sparse20 | 30.93 ms | 2.277 ms | 439 frames/s |
| sparse100 | 153.9 ms | 10.52 ms | 95 frames/s |
| dense5 | 22.51 ms | 20.55 ms | 49 frames/s |

Divan uses 100 samples per variant/case. The dense result does not establish a
gain because process sessions disagree and outliers/background load are material.
For 1024-wide intersecting layers, measured per-render allocations fall from two
(approximately 4.198 MB) to one (4.194 MB): the 4 MiB final image remains, while
the 4 KiB scratch row disappears. Authoritative source tile residency is unchanged.
RSS varies by collector/run; do not infer a general multi-megabyte reclamation
claim from that variation. Native time logs are retained alongside Hyperfine data.

Zero-render calibration files record process/fixture overhead without subtracting
it. Sparse100 setup is material compared with its faster renderer, so its process
ratio is smaller than the steady-state ratio. Other workload counts amortize much
of initialization, but no blanket below-5% overhead claim is made.

## Evidence and correctness

- `divan.txt.gz`, `divan-metadata.json`, `hardware.json` and `build-environment.json`
  retain the native run and build identities.
- `process/session-*.tar.gz` preserve all 27 unmodified runner outputs byte-for-byte,
  including raw Hyperfine 2 JSON/Markdown, correctness logs, execution metadata,
  native resource logs and original manifests. Compression keeps generated raw
  exports intact while respecting the repository source-file line limit.
- `process/ARCHIVES.sha256` hashes the archives. The summary tool verifies original
  manifests and completed execution before calculating the CSV.
- `exploratory-baseline.txt.gz` is a preliminary baseline; `exploratory-drop-excluded.txt.gz`
  came from a harness that excluded result destruction. Both are explicitly
  ineligible for this decision and retained rather than erased.
- Row-boundary/COW/hidden-RGB tests, independent all-alpha/opacity pixel tests,
  existing canonical conformance vectors and the full on-disk PCND/PNG editing
  workflow are the correctness gates. Final validation: 493 workspace tests passed
  (20 explicitly ignored), strict workspace/all-target Clippy passed, and the real
  M1 Pro Metal adapter parity test passed with `PAINTCRAFT_REQUIRE_GPU=1`.
  Original validation logs are compressed losslessly beside `validation.json`.

Reproduce the process protocol with prebuilt code and a fresh output directory:

```sh
python3 benchmarks/tools/run_composite.py --output /tmp/paintcraft-cpu-comparison --execute
python3 benchmarks/tools/summarize_composite.py benchmarks/results/cpu-spans-2026-10-19/process --output /tmp/cpu-summary.csv
```

## Remaining work

Implemented and tested: exact borrowed-span CPU composition and regression/benchmark
adapters. Locally measured: opaque/default and sparse full-composite improvements.
Not qualified: other ISAs, controlled hardware/event-tail latency, fully occupied
opaque import fast paths, huge images and general resident-memory reclamation.

Still incomplete: dirty-region rendering, prefix caches, parallel partitioning,
mipmaps, GPU residency/crossover/device loss, adaptive compressed/disk-backed
branching history, full brush/selection policies and broader format interoperability.
See [the backend audit](../../../docs/BACKEND_PROGRESS.md).
