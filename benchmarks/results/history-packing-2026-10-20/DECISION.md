# Recorded history ownership: local cost and memory comparison

Keep adaptive cold packing as a memory-pressure option, preserving shared hot COW
states and raw fallback. Do not pack every edit by default. Sparse histories use
substantially less process memory when older decoded tiles are released, at a
material cost in complete edit/undo/redo time. Full-extent noisy tiles show no
memory gain and roughly double lifecycle time. Metadata-only records demonstrate
that compacting references can reduce traversal costs without reducing process RSS.

The 32/64/128-edit checkpoints are experimental cold-age policies. The shipping
adaptive policy triggers under retention pressure, with at most 64 entries/64 new
sources prepared per publication; this experiment does not establish that bound
as an optimal threshold. It does not establish a universal packing interval or
codec. Maintain T04/T07/T11 and BF18 as partial.

## Whole-process evidence

Three independent forward/reverse/forward sessions, each 20 runs and three warmups,
produce 60 retained measurements per case/variant (1,200 total). The table reports
pooled median elapsed milliseconds and median peak resident MiB, as `ms / MiB`.
All timings include new session construction, typed commits, CRC capture, explicit
checkpoint packing where requested, full recorded undo/redo verification, reader
release and document/history destruction. They exclude compilation and exports.

| Case | Shared COW | Every 32 | Every 64 | Every 128 |
|---|---:|---:|---:|---:|
| Sparse, 256 edits | 31.7 / 66.1 | 63.6 / 35.5 | 65.5 / 49.9 | 67.4 / 60.2 |
| Sparse, 1,000 edits | 193.9 / 253.5 | 291.0 / 57.2 | 293.4 / 86.5 | 298.4 / 119.1 |
| Held readers, 1,000 edits | 199.0 / 253.7 | 283.1 / 77.4 | 290.6 / 99.5 | 293.5 / 127.0 |
| Full-extent noise, 128 edits | 14.9 / 34.1 | 30.4 / 34.3 | 30.6 / 34.3 | 30.8 / 34.2 |
| Metadata, 1,000 edits | 81.9 / 3.2 | 59.5 / 3.8 | 59.5 / 3.6 | 60.9 / 3.6 |

For the 1,000-edit sparse case, 32-edit packing lowers peak RSS by about 77%, while
the complete lifecycle takes about 50% longer. The held-reader case pins every
eighth snapshot until the explicit release phase. It still lowers peak RSS, but
less than the unheld case. These figures concern tiny changes to a sparse canvas;
they do not establish photographic-region-delta performance.

All variants have the same 768 MiB retention allowance and retain every required
state. Packed variants use 16 MiB staging and two hot states. No shortened undo
chain, different reference image or destructive no-op is used to improve results.
The sparse fixture is 257×259 with hidden RGB at alpha zero; its 1,000 distinct
single-pixel commits concentrate within one tile. Noise replaces a complete 256²
tile using seeded bytes; metadata changes layer opacity over one shared tile.

## Internal samples, phases and reader release

Both parameter orders retain 20 flat Criterion samples per combination: 800 raw
samples across 40 invocations. `criterion-summary.csv` reports per-sample complete
lifecycle time (measured time divided by iteration count). Raw JSON/CSV and command
logs remain in forward/reverse archives with their own verified manifests. These
boundaries include replay correctness checks and destruction, not just codec time.
The held1000/pack64 internal samples have substantial variability: forward median
331 ms/max 821 ms and reverse median 304 ms/max 499 ms. All remain included; no
noise-free ordering, universal checkpoint winner or tail guarantee is asserted.

Separate single-replay traces retain every event and report descriptive
p50/p95/p99/max in `phase-summary.csv`. For sparse 1,000-edit histories:

| Phase | Shared | Every 32 edits |
|---|---:|---:|
| Undo p95 | 0.0047 ms | 0.0187 ms |
| Redo p95 | 0.0046 ms | 0.0178 ms |
| Edit cycle p99 | 0.3219 ms | 4.2710 ms |
| Checkpoint compaction maximum | — | 4.6876 ms |

Edit-cycle events include capture/CRC and occasional explicit packing; navigation
events exclude the subsequent fingerprint verification. The final extra packing
checkpoint is included in lifecycle timing, but is outside per-edit events. These
are one trace per strategy/case, with linear-interpolated percentiles; they do not
qualify queue-inclusive pointer latency, rendering, pressure-trigger latency or a
service-level tail guarantee. Tracing and RSS subprocesses run outside accepted
throughput measurements.

In the held-reader/pack32 trace, decoded capacities fall from 33,292,288 to 524,288
bytes when readers release; 32,768,000 bytes were explicitly reported as leased.
Encoded payloads remain 2,209,200 bytes. Current RSS remains 88,960 KiB immediately
before/after release on this run: the allocator can retain freed pages. The measured
peak-RSS improvement concerns the whole packed process, not an asserted immediate
RSS drop at this release point. `ownership-summary.csv` records every case/variant.

## Validity and remaining work

Preflight and timed replays compare recorded state IDs, full tile CRCs and opacity
at every undo and redo. Independent tests cover pixels, metadata, off-canvas data,
held-reader lifetime, corruption, admission/cancellation, epoch overflow and branch
publication. Tests pass without effect/brush replay. The benchmark uses the same
production session/history code as the editor, with explicit packing API calls.
The shared control uses the current ownership ledger too; it is not a historical
pre-change binary. This does not qualify the default pressure policy against the
previous release or establish that new accounting has no per-commit overhead.

Measurements are from one Apple M1 Pro developer host with no affinity, thermal
or power control. Raw runs, including outliers and tied/regressing cases, remain
in the evidence. Hyperfine process counters and separate native `time -l` resource
logs are process-wide; ownership weights are not RSS or physical admission.
Calibration constructs only the fixture and is excluded from strategy comparisons.

LZ4/Zstd, previous-version photographic deltas, disk spill and segment GC, global
logical storage, background work, branch graphs and queue-inclusive interactive
latency remain required. No codec superiority, universal memory ratio, persistent
GPU memory saving, full-command export speed or display-latency claim follows.
