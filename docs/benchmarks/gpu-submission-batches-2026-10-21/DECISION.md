# Interrupted GPU submission-batching attempt

This attempt stopped with ENOSPC while updating the first 100-layer process
export. The JSON retains three variants (60 raw observations); the fourth has
only its printed summary. Three earlier cases retain another 240 raw process
observations. All six Criterion cases retain 480 raw samples. No complete
three-session process result or default-policy decision is claimed here.

`benchmarks/results/gpu-submission-batches-2026-10-21/interruption.json` (local result) records the terminal failure and
Cargo dev-profile-only cleanup. Input sources and the frozen release executable
were verified unchanged. The separate
[retry](../gpu-submission-batches-2026-10-21-rerun/DECISION.md) repeats the entire
preplanned comparison, with disk headroom recorded. This directory is preserved
without excluding slow observations or merging partial process samples into a
completed comparison.

The original method follows for auditability.

# Bounded ordered GPU submission batching

This comparison isolates bounded submission batching within each existing source
policy. It does not compare GPU output with a weaker scalar implementation or
establish a CPU/GPU crossover. The automatic desktop scaled path remains CPU.

## Frozen source and reproduction

Measured source/runner commit: `addabc079a48ee83f42b7845c0e61028e4f30a9f`.
That documentation-only archive commit contains implementation `6781978`, native
integration `bbf27f5` and driver `2fdb90f`. The `benchmarks/results/gpu-submission-batches-2026-10-21/manifest.json` (local result) records
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
