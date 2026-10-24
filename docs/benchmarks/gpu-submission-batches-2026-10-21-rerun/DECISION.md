# Bounded ordered GPU submission batching: complete retry

This comparison isolates bounded submission batching within each existing source
policy. It does not compare GPU output with a weaker scalar implementation or
establish a CPU/GPU crossover. The automatic desktop scaled path remains CPU.
Keep serial single-view submission as the default. Expose explicit native
`PAINTCRAFT_GPU_BATCHING=1` (batching) and `=regions` (batching plus regional sources)
for the measured tradeoff; positive sampling levels also require
`PAINTCRAFT_GPU_SCALED=1`. Five-layer timing is mixed and process RSS rises.
This is an opt-in host policy, not a universal acceleration claim.

## Frozen source and reproduction

Measured source/runner commit: `addabc079a48ee83f42b7845c0e61028e4f30a9f`.
That documentation-only archive commit contains implementation `6781978`, native
integration `bbf27f5` and driver `2fdb90f`. The `benchmarks/results/gpu-submission-batches-2026-10-21-rerun/manifest.json` (local result) records
full Rust/WGSL/manifest/runner hashes, executable hash, compiler, workload settings
and exact ignored test identity. No build or other test is intentionally run
alongside timing. Compiler flag/wrapper/target-directory environment overrides were
absent; the recorded Cargo profiles and explicit build command apply.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --suite gpu-batches --binary <reported-lib-test-executable> --output <new-result-directory>
```

Variants `gpu64`/`gpu64batch` use the original single-view-per-layer source policy;
`gpu64regions`/`gpu64regionsbatch` use bounded regional retention. Within each pair,
only submission policy changes. All use 64-sample output tiles, exact ordered
native composition before wide-integer box reduction, final egui premultiplication,
completed queue work and actual App native texture acceptance. Timed display uses
no readback; explicit verification readback is outside timing. Private output
retention is zero in all variants.

Each sequence includes device/renderer/font/document setup and teardown, actual
incremental desktop preview staging, and eight frames adding eight size-16 translucent
brush points each. The uniform control instead publishes eight typed opacity edits
and retains zero resident raster tiles. Source images and required pixels match
within each pair. Preflights and postflights after every session independently
execute commands, full native composition and direct box filtering; every output
byte, authoritative source guard and uniform publication count is checked.

| Case | Source | Layers | Level | Output |
| --- | --- | --- | --- | --- |
| Tiny | 32×32 sparse | 1 | 1 | 16×16 |
| Dense | 512×256 dense | 5 | 1 | 256×128 |
| Layers20 | 512×256 dense | 20 | 1 | 256×128 |
| Layers100 | 512×256 dense | 100 | 1 | 256×128 |
| Large | 4096×2160 sparse | 1 | 2 | 1024×540 |
| Uniform | 4096×2160 implicit white | 1 | 2 | 1024×540 |

## Actual work and bounds

The batch window admits at most 16 layers, normally at most 1 MiB of source-view
sizes and a 1 MiB pending-key flush threshold. One larger source runs alone; one
final metadata key can cross the threshold under its existing per-key admission.
See [the ownership contract](../../../docs/GPU_BATCHING.md). Separate compute passes
retain original layer rounding/order; no layers are regrouped mathematically.
The native worker declares 4 MiB extra bounded pending resources. This is not a
physical driver/RSS budget or asynchronous transfer implementation.

| Per eight-frame sequence | Serial compositor submissions | Batched submissions |
| --- | --- | --- |
| Tiny | 8 | 8 |
| Dense 5 | 320 | 64 |
| Dense 20 | 1,280 | 128 |
| Dense 100 | 6,400 | 448 |
| Sparse 4K | 8 | 8 |
| Uniform 4K | 0 | 0 |

Counts exclude separate reduction/clear/presentation/readback submissions. Maximum
observed contributing layers are 5/16/16 in dense 5/20/100, with source windows
327,680/1,048,576/1,048,576 bytes and key windows 640/2,048/2,048 bytes. These are
actual application counters, not hardware bus or GPU timestamps.

Source upload/allocation/hit counters are equal within every serial/batched pair.
Under single-view retention, dense 5/20/100 stage 20/80/400 MiB and create
320/1,280/6,400 sources. Regional retention independently reduces source work,
including the known over-entry-limit cases; batching does not take credit for
those different source policies. All dense cases compose 1,048,576 native output
pixels per sequence, with the original layer count applied to each.

## Evidence and scope

Ratios below divide each serial process median by its batched median under the
same source policy. Above one means batching was faster. Each range spans the
three completed sessions; session two reverses all four variants.

| Case | Single-view serial / batched | Regional serial / batched |
| --- | --- | --- |
| Tiny | 0.941–1.063 | 0.981–1.001 |
| Dense 5 | 0.580–1.298 | 0.906–1.358 |
| Dense 20 | 1.801–2.366 | 1.521–1.767 |
| Dense 100 | 2.029–2.208 | 2.168–2.400 |
| Sparse 4K | 0.998–1.031 | 0.986–1.007 |
| Uniform 4K | 0.885–1.030 | 0.990–1.039 |

For twenty layers, single-view serial medians are 482.62–721.56 ms per eight-frame
sequence versus 267.98–305.01 ms batched. For one hundred they are
2,015.66–2,173.11 ms versus 950.77–1,003.74 ms. The five-layer first-session
regression remains: 221.52 ms serial versus 382.11 ms batched under single-view
retention. The third regional session also regresses (196.87 versus 217.41 ms).
Single-layer/uniform controls have identical required submission counts and still
vary. No minimum winning layer count or global dispatch threshold is inferred.

Twenty-layer process RSS medians rise from about 36–37 MiB serial to 52–54 MiB
batched under single-view retention, and from 36 MiB to 50–57 MiB regionally.
Hundred-layer medians rise from 77–78 MiB to 96–97 MiB single-view, or 77–79 MiB
to 94–98 MiB regionally. These measured host costs exceed the extra 4 MiB of
declared application pending resources: opaque driver/allocator effects are outside
that declaration. It is not a total physical/RSS bound. This tradeoff is part of
the reason for explicit opt-in rather than global enablement.

All maxima remain: single-view twenty-layer serial reaches 2,739.97 ms and
regional hundred-layer serial reaches 3,424.86 ms. Calibration is un-subtracted.
The `benchmarks/results/gpu-submission-batches-2026-10-21-rerun/summary.json` (local result) retains every session's medians/min/max/RSS plus raw
statistical sample summaries; Criterion arrays retain exact times/iteration counts.
Within-process samples and whole-process observations are distinct scopes.

Criterion medians (serial/batched, ms per sequence) are 39.98/49.10 tiny,
224.26/151.61 dense 5, 500.78/370.86 dense 20, 2,289.07/1,154.66 dense 100,
35.27/35.91 sparse 4K and 23.58/22.67 uniform under single-view retention.
Regional values are 48.60/51.11, 188.14/123.86, 485.30/289.80,
2,051.64/1,170.51, 35.63/37.18 and 24.12/24.23 respectively. The small-control
regressions and statistical/process differences remain visible; these samples do
not replace native input/display tail measurements.

This retry retains 480 raw statistical samples and 1,500 process observations
(1,440 comparison plus 60 calibration), all with zero exit status. The runner
checks source/binary identity after each session; `benchmarks/results/gpu-submission-batches-2026-10-21-rerun/identity-verification.json` (local result)
also checks all recorded blobs against the frozen commit after measurement.
Final native-option integration has separate source/validation evidence.

The [interrupted first attempt](../gpu-submission-batches-2026-10-21/DECISION.md)
retains another 480 statistical samples and 300 raw process observations, including
the partial three-variant export. Its fourth hundred-layer result exists only as
a printed summary and is not promoted into missing raw samples. No complete
process comparison is claimed for that attempt. The full retry was preplanned
after its terminal ENOSPC error, not selected by excluding slow observations.
Only generated Cargo dev-profile artifacts were reclaimed; the frozen release
executable and source identities stayed unchanged. `benchmarks/results/gpu-submission-batches-2026-10-21-rerun/free-space.json` (local result)
records a minimum sampled free space of approximately 15.59 GiB during the retry. No concurrent build/test
was intentionally run with either timing attempt.

Raw Criterion arrays contain 20 flat samples per variant; process sessions use
20 runs and three warmups, reversing variant order in session two. Calibration
is retained without subtraction. Every slow observation and memory measurement
remains; no unsuccessful baseline is timed as a gain. Raw original Hyperfine
JSON bytes are gzip-preserved alongside compact equal-valued JSON and SHA sidecars.
Logs are compressed byte-for-byte to preserve diagnostics and tool whitespace.

The experiment is completed texture readiness on one Apple M1 Pro/Metal host.
It includes cold setup/teardown, not OS event/display latency. There are no affinity,
thermal, power or background-load controls. Physical GPU residency and allocator
pressure are not measured by logical source counters or the declared extra bytes.

Real Metal tests cover scalar equality across 1/5/20/100 layers, reversed order,
exact byte-window boundaries, larger solitary inputs, source cache overflow,
cancellation at every observed checkpoint, device retirement and retained-frame
immutability. Actual egui rendering compares serial/batched native and scaled
textures, including partial edges and old frames after scratch reuse. An admitted
20-layer worker checks exact output and the declared source window. A development
Clippy run caught an obsolete unused test helper; it was removed before freezing
the measured source and the final strict run passes.

Asynchronous transfer/completion services, device reconstruction, physical GPU
budgets, priorities, native pressure/tail latency, other adapters and broader
feature/reference qualification remain open. T12/T13/T21/T37 remain partial.
