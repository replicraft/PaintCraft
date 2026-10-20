# Independent history codecs: local evidence and policy

Keep raw/solid/default-relative history packing as the default. Expose LZ4 and
Zstd levels 1/3 as explicit policies; do not select a universal codec or interval.
LZ4 lowers peak RSS for the structured full-tile case at extra lifecycle cost.
Zstd produces smaller payloads, but that ratio does not translate into the best
process memory or CPU result. Incompressible tiles retain raw ownership, and
trying a codec still costs time and workspace. BF19/T11 remain partial.

## Complete process comparisons

Three independent forward/reverse/forward sessions retain 20 timed runs and three
warmups per parameter: 1,800 process measurements. All packed variants use the
same 64-edit checkpoints, two hot states, 16 MiB staging and 768 MiB retention.
Every variant preserves every required state; no history is trimmed. Shared COW
has no checkpoint packing. The table shows median lifecycle ms / peak resident MiB
pooled across all 60 runs per parameter.

| Case | Shared | Raw/patch | LZ4 | Zstd 1 | Zstd 3 |
|---|---:|---:|---:|---:|---:|
| Sparse, 256 edits | 37.3 / 66.4 | 68.5 / 46.4 | 71.3 / 47.7 | 87.2 / 50.2 | 89.6 / 48.8 |
| Sparse, 1,000 edits | 197.6 / 253.8 | 296.1 / 82.3 | 316.5 / 87.6 | 333.6 / 87.2 | 337.3 / 91.1 |
| Held readers, 1,000 edits | 199.4 / 253.9 | 291.6 / 97.3 | 315.1 / 100.9 | 341.7 / 94.3 | 331.9 / 101.9 |
| Noise, 128 edits | 14.7 / 34.1 | 29.7 / 34.2 | 32.1 / 34.8 | 33.7 / 35.4 | 34.9 / 36.2 |
| Structured, 256 edits | 31.1 / 66.4 | 62.8 / 66.6 | 87.8 / 48.5 | 198.2 / 51.4 | 192.5 / 56.5 |
| Metadata, 1,000 edits | 79.6 / 3.2 | 58.4 / 3.7 | 58.9 / 3.9 | 59.1 / 4.1 | 59.1 / 4.6 |

LZ4's structured peak RSS is about 27% lower than raw/patch, with about 40% higher
complete lifecycle time. Zstd 1 takes over twice LZ4's lifecycle time there despite
storing fewer bytes. These are not isolated encoder throughput or display latency.
The fixed source is one structured 256² RGBA tile with coarse color ramps and
alpha stripes, then distinct small pixel edits. It is not a natural-photo corpus.
Sparse/held/metadata/noise fixtures use the same seeded generators as the previous
ownership experiment. This experiment's raw strategy is the current implementation,
not a historical pre-change binary or a no-accounting baseline.

The measured boundary includes fixture creation, typed JSON commits, recorded
state CRC capture, checkpoint packing, reader release, full undo/redo verification,
and session/history destruction. Compilation, trace subprocesses, exports and
statistical analysis are outside it. Work units count commits plus undo/redo,
including the metadata fixture's setup transition. Calibration constructs just the
fixture and is excluded from policy comparisons. All cases retain the same output
requirements and budgets. No codec worker threads are enabled or granted free.

## Actual ownership and workspace

After packing/releasing readers, sparse1000 retains 524,288 decoded bytes in each
packed strategy. Total encoded bytes are 2,209,200 raw/patch, 953,564 LZ4, 71,513
Zstd 1 and 59,441 Zstd 3. The much smaller Zstd payload does not deliver lower peak
RSS than raw/patch in that process workload. Peak RSS includes hot/captured states,
allocator behavior, transient decoding, metadata and codec workspaces.

Structured256 raw/patch still owns 67,371,008 decoded bytes. Each codec releases
old decoded source allocations, retaining 524,288 decoded bytes plus 4,953,112
LZ4, 2,678,782 Zstd 1 or 2,686,262 Zstd 3 encoded bytes. Noise128 keeps 33,816,576
decoded bytes and no encoded payload in all strategies: raw fallback avoids a
second retained copy, while unsuccessful compression work remains a real cost.
Metadata-only operations share one pixel payload; they are not new tile copies.

LZ4 uses a conservative 16,384-byte stack-table allowance and a 288,378-byte
scratch buffer. Observed Zstd encoder context peaks are 582,672 bytes at level 1
and 1,041,424 at level 3; output scratch is 263,168 bytes. Staging conservatively
reserves 8 MiB native headroom before context creation and checks observed size
per block. That reservation is not an intercepted native allocator or a hard
physical cap. Native decoder state is transient and outside decoded pixel metrics;
process counters include its cost. Solid payloads bypass codec setup entirely.

## Internal samples and phase traces

Two parameter orders retain 20 raw flat Criterion samples per combination: 1,200
samples across 60 invocations. `criterion-summary.csv` reports measured time per
iteration; command logs and raw JSON/CSV remain in verified archives. These
include correctness replay and destruction, not just the compression primitive.
Outliers remain: reverse held1000/zstd3 has a 333 ms median and 1,573 ms maximum;
forward noise128/shared has a 17 ms median and 289 ms maximum. No cause is assigned,
and these measurements do not support a noise-free order or tail guarantee.

Separate single-replay phase traces report descriptive interpolated percentiles
in `phase-summary.csv`. Structured256 undo p95 is 0.0045 ms raw, 0.0289 ms LZ4,
0.1458 ms Zstd 1 and 0.1438 ms Zstd 3. Checkpoint maxima are 7.99, 12.43, 25.86
and 25.90 ms respectively. Sparse1000 edit-cycle p99 is 8.27, 8.84, 9.00 and
9.57 ms. Edit cycles include CRC capture/checkpoints; navigation events exclude
subsequent fingerprint verification. Final extra packing is included in complete
lifecycle timing but lies outside per-edit events. These traces do not measure
queueing, pressure-policy interference or pointer/display latency.

`ownership-summary.csv` includes compressed payload counts/bytes, observed encoder
workspace, scratch capacity and current RSS around reader release. For held1000/
Zstd 1, decoded bytes fall from 33,292,288 to 524,288 after readers release; the
32,768,000 leased bytes disappear. RSS goes from 91,744 to 91,984 KiB in that trace:
allocation release does not prove pages were returned immediately. RSS subprocesses
and phase tracing run separately from accepted throughput measurements.

## Validation and limits

531 workspace tests (20 explicitly ignored), strict all-target Clippy and required
real Metal parity pass. Tests include independent decoded-byte expectations,
hidden RGB, full recorded IDs/navigation, live reader leases, staging/restoration
failure, cancellation before/after compression, corrupt encoded data, short/wrong
Zstd content, trailing/concatenated/truncated frames and raw/small/solid fallback.
Preflight and timed replay compare every retained state's ID, full tile CRC and
opacity. A failed decode does not publish a cursor/document/epoch change.

Measurements use one Apple M1 Pro developer host without thermal, power or affinity
control. All outliers and regressions remain. Current RSS and allocated ownership
are distinct; freeing a decoded Vec does not force pages back to the OS. Local
lifecycle comparisons do not qualify a global allocator, pressure-triggered tail
latency, queue interference, natural photographs, u8 masks, higher-depth samples,
disk spill, previous-version deltas, background work, GPU memory or file codecs.
The codecs are real independent stored blocks, not effect replay or delta chains.
