# ASE palette workload baseline

This measures the implemented F14 palette resource workflow using the selected
`ag-psd` 0.3.0 adapter. It supplies a local BF25 baseline for T08/T20; it does not
select an optimized replacement, compare different codecs, or complete F14 or
release qualification.

The 2026-10-25 local run used an Apple M1 Pro, macOS arm64, Rust 1.99.0,
Criterion 0.8.2, Divan 0.1.21 and Hyperfine 2.0.0. Artifacts were built at release
optimization level 3 with `CARGO_PROFILE_RELEASE_DEBUG=0`, frozen before timing,
and checked against Cargo's JSON build receipts. The only lockfile change adds
the already selected Criterion development dependency to the formats package;
no package versions changed.

## Correctness and boundaries

Four owned fixtures cover empty, six mixed-model swatches, and 256/4096 grouped
swatches. Larger fixtures contain RGB, CMYK, Gray and LAB, all three swatch types,
Unicode/non-BMP names and varied finite values. Encoded sizes are 12, 282, 11,488
and 193,412 bytes respectively. Exact adapter roundtrips run before measurement.
The independent pinned `swatch` 0.4.0 parser verifies every name, group, type,
model and f32 channel bit across all 4,358 swatches. It also checks the actual
CLI exports before and after timing. Both statistical harnesses generate
byte-identical fixtures. All source and destination checks passed.

Fixture setup/preflight is outside in-process measurement. Decode includes
structural preflight, upstream decoding, typed-resource validation and result
teardown. Encode includes resource validation, upstream writing, output preflight
and teardown. No decoded result or output buffer is leaked to move destruction
outside timing. The throughput denominator is encoded bytes, not raster pixels.

Criterion uses the production system allocator, 100 flat samples per cell,
1-second warmup and 2-second measurement targets. Three sessions run in
forward/reverse/forward case order, retaining 24 sample JSONs and 24 raw CSVs:
2,400 calibrated samples total. Divan separately records allocation-profiled
statistics, 30 samples with 16 iterations each. Its supplementary wall-clock CSVs
are unadjusted 16-operation batches, not reconstructed Divan samples. Divan's
normalized `max alloc` columns are not treated as physical heap peaks or RSS.

Hyperfine runs complete `palette inspect --json` and `palette export --json`
commands, with existing output replacement enabled for export. Three sessions
use 20 measured executions and three warmups per cell, including reverse order.
All 480 measured executions returned success. Inspect includes file acquisition,
validation, hashing, JSON construction/output and process startup/teardown.
Export additionally includes encoding and the durable staging/replacement
coordinator. These are different operations; automatic cross-command speed
rankings in the raw Hyperfine report are not optimization comparisons.

## Observations

Ranges below span the three session medians. Criterion medians are derived from
calibrated per-operation batch averages, not individual interactive events.

| Swatches | Decode, Criterion | Encode, Criterion | CLI inspect | CLI durable export |
| --- | ---: | ---: | ---: | ---: |
| 0 | 86.8–87.4 ns | 114–125 ns | 1.63–1.96 ms | 12.38–12.94 ms |
| 6 | 1.77–1.84 µs | 1.33–1.42 µs | 1.96–2.11 ms | 12.72–12.91 ms |
| 256 | 70.0–71.0 µs | 53.4–54.8 µs | 2.38–2.66 ms | 12.80–13.12 ms |
| 4096 | 1.146–1.174 ms | 0.948–0.984 ms | 9.77–13.72 ms | 20.44–30.57 ms |

Hyperfine's native process peak-RSS metric is explicitly reported in bytes.
The maximum observed 4096-swatch values were 8.48 MiB for inspect and 8.95 MiB
for export, across 60 measured executions of each operation. This is process
resident memory, including startup and JSON reporting, not an adapter allocation
limit or desktop/browser memory measurement. Raw CPU-cycle/instruction fields
are retained without interpreting them as exclusive codec costs.

The third large-palette CLI session was slower; it remains included. Earlier
manual-run CLI observations and the initial Divan correctness-mode invocation
are also retained. The latter has no calibrated statistics and is explicitly
excluded from that evidence. A runner trial failed at oracle preflight because
resolving a virtual-environment interpreter symlink discarded its environment;
its logs remain retained and it contributed no timing data. The checked-in
runner preserves that interpreter path and completed the full protocol.

No builds or tests were intentionally run alongside the final measurement
sessions. Other user workload, CPU affinity, power and thermal state were
uncontrolled. These warmed local observations do not establish interactive p99,
cancellation tails, hostile-input memory bounds, original Adobe compatibility,
other platforms, or a faster backend. Keep the current implementation as a
measured functional baseline; further optimizations require matched comparisons.

## Evidence and reproduction

Final local evidence is under ignored
`benchmarks/results/palette-20261026T005210Z-calibrated/`: frozen binaries, Cargo
receipts, source/binary/fixture hashes, independent pre/postflight logs, commands,
Criterion raw samples/CSV/estimates, Divan tables and supplementary raw batches,
and unmodified Hyperfine JSON with memory/CPU metrics. Earlier trials are under
sibling directories with the same timestamp, including `-reproduced` and
`-verified-runner`; none was deleted or silently promoted into calibrated data.

Build optimized artifacts while retaining compiler receipts:

```sh
CARGO_PROFILE_RELEASE_DEBUG=0 cargo build --release --locked \
    -p paintcraft-formats --bench palette_assets --bench palette_samples \
    --message-format=json > /tmp/ase-bench-build.jsonl
CARGO_PROFILE_RELEASE_DEBUG=0 cargo build --release --locked \
    -p paintcraft-cli --message-format=json > /tmp/ase-cli-build.jsonl
```

Create a separate Python environment with `swatch==0.4.0`, as described in
`crates/services/paintcraft-formats/tests/ase-fixtures/README.md`. Use the two
benchmark executable paths reported by `compiler-artifact` events in the build
JSON, then run:

```sh
python3 benchmarks/tools/run_palette_assets.py \
    --bench 'target/release/deps/palette_assets-<artifact-hash>' \
    --samples 'target/release/deps/palette_samples-<artifact-hash>' \
    --cli target/release/paintcraft-cli \
    --build-jsonl /tmp/ase-bench-build.jsonl \
    --cli-build-jsonl /tmp/ase-cli-build.jsonl \
    --oracle-python /tmp/ase-oracle/bin/python \
    --output benchmarks/results/new-ase-baseline --execute
```

Without `--execute`, the runner prints a read-only plan. It refuses existing
output directories and non-optimized/mismatched build receipts, freezes the
artifacts, verifies independent meaning before timing, and checks retained
samples and artifact/source hashes afterward. It never builds, installs tools,
removes caches, or overwrites an earlier session. Raw result averages are not
subtracted by an empty-palette startup calibration.
