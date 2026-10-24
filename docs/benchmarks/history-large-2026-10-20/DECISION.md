# Large snapshot pressure: bounded classification and selective transfer

Automatic preserving history no longer skips a snapshot merely because it needs
more than 64 new tile references. It registers immutable raw leases for unclassified
sources, classifies at most 64 sources per preparation batch and continues pressure
relief across batches while progress is possible. Transfers classify deferred data,
keep uniform payloads inline, apply the selected codec and synchronize required
bytes before publication. Raw sources still owned by hot documents are excluded
from automatic transfer. Manual explicit transfer retains its complete semantics.

Recent states stay hot when they fit. If necessary, the previous state can become
cold while the new current state remains hot. A failure in a later batch drops the
whole candidate, including earlier provisional disk extents, without changing the
published document, cursor, selection, epoch or redo path. This fixes an admission
limit, not a global logical tile store or asynchronous history pipeline.

## Process comparison

Three forward/reverse/forward sessions retain 20 timed runs and three warmups per
parameter: 360 process measurements. Each case executes 32 edits and full recorded
undo/redo (96 work units), with no trimming. Medians pool all 60 runs per parameter.

| Resident base | Automatic lifecycle | Explicit lifecycle | Automatic peak RSS | Explicit peak RSS |
|---|---:|---:|---:|---:|
| 65 tiles | 132.5 ms | 278.3 ms | 39.8 MiB | 38.3 MiB |
| 130 tiles | 166.8 ms | 354.1 ms | 72.2 MiB | 70.8 MiB |
| 260 tiles | 235.7 ms | 527.6 ms | 136.8 MiB | 136.0 MiB |

Automatic lifecycle medians are about 2.1–2.2× faster than this explicit preparation
strategy. Peak RSS is similar and slightly higher for automatic in these medians;
no general RSS reduction is established. Setup/import and allocator behavior are
part of the process peak. Do not apply these ratios to unrelated history, effects,
exports, GPU work, large full-surface filters or interactive response time.

The explicit strategy uses the current public full-scan compaction and complete
spill APIs after each edit. That pipeline was available before this fix and can
retain the required timeline under these allowances; the old automatic strategy
could reject these workloads and is not presented as a timed successful baseline.
This is an API/policy comparison on the same binary, not a historical binary claim.
The explicit strategy scans/spills raw base references even while hot states own
them. Automatic avoids that work; neither variant omits any required state/output.

Every strategy uses `(tile count + 4) × 262,144` retained bytes, 64 MiB preparation
staging, a 128 MiB native store and 4 MiB segment targets. Sources are deterministic
high-entropy RGBA, 256 pixels high and 65/130/260 tiles wide. The 32 distinct point
edits touch tile zero; other tile bytes remain unchanged. This tests large shared
bases and low headroom, not a photographic or higher-depth corpus.

The measured boundary includes generator/import/reference CRC construction, store
setup, typed JSON commits, pressure or explicit preparation, verification, complete
navigation, history/store destruction, collection and directory removal. Compilation,
statistical analysis and trace output are outside. Calibration only constructs the
fixture and is not subtracted. The native filesystem/page cache is not flushed;
these are newly written session extents, not cold-device timings.

## Storage and phase observations

After 32 edits, automatic versus explicit logical disk bodies are 7,864,320 versus
24,903,680 bytes at 65 tiles; 8,126,464 versus 41,943,040 at 130; and 8,126,464
versus 76,021,760 at 260. Automatic avoids copying the unchanged hot base to disk.
At 260 tiles, reserved segment lengths are 8,127,952 versus 76,035,680 bytes, in
17 versus 48 segments. These are referenced body/reservation counters, not filesystem
block allocation or system memory. Both strategies retain all 33 states.

Forward/reverse exact-filtered Criterion runs retain 240 raw samples across 12
invocations. `criterion-summary.csv` reports measured time per iteration. Reverse
large260/explicit has a 511.8 ms sample median and 857.3 ms maximum; every sample
remains included. Commands, raw JSON/CSV and inner manifests are archived.

Separate single-replay traces are descriptive, not service-level tail qualification.
At 130 tiles, edit-cycle p95/p99/max are 10.77/15.12/16.60 ms automatic versus
6.93/76.43/107.61 ms explicit. At 260 tiles, maxima are 13.62 versus 228.37 ms.
Automatic does not improve every percentile: its occasional pressure batches can
raise p95 relative to frequent explicit work. An explicit `record` event excludes
its subsequent manual preparation, so compare full `edit-cycle` events across
strategies. Those include reference checks and ownership accounting too. Navigation
phases exclude subsequent fingerprint checks. No queueing, actual pointer samples,
rendering or GPU presentation is measured.

## Correctness boundaries

556 workspace tests (20 explicitly ignored), strict all-target Clippy and real
Metal parity pass. New tests use 130-tile documents and compare complete pixels
through navigation, including an 80-tile preparation with exactly 64 scanned and
16 deferred sources. Full-surface replacement requires more than two transfer
batches. Failure after the first 64 writes preserves the old document/redo and
releases provisional storage. Uniform hidden RGB stays exact without disk, and
deferred structured tiles can finish warm compression without a store.

Preflight derives tile CRCs independently from generator rows and checks every
unchanged tile's bytes at every recorded transition. The timed replay independently
checks expected changed-tile bytes, every other immutable tile identity and all
unchanged CRCs at the beginning and both ends of navigation. Fingerprints retain
weak identities, not source leases; they cannot artificially pin decoded history.
Full byte tests/preflight supplement the timed identity invariant. Every recorded
state ID is checked on undo and redo. There is no effect/brush/plugin replay.

Deferred records contain immutable raw authority, not unchecked encoded input.
Their CRC is computed during classification; encoded/disk restoration requires a
recorded checksum. When no representation or store can meet the allowance, the
candidate is still rejected before publication. No history is silently removed.

## Scope and remaining gates

Source/binary/artifact hashes and exact commands identify the frozen implementation.
One Apple M1 Pro developer host is measured without affinity, thermal or power
control. All outliers remain. Owned/leased decoded bytes, logical payloads, reserved
segment lengths, RSS, filesystem allocation and kernel cache are different scopes.
The history allowance does not prove a process-wide physical-memory bound.

Batch sizes bound classification and transfer work per step, not the duration of
an entire edit. Large operations can run multiple synchronous batches; background
scheduling, queue-inclusive latency and global admission remain necessary. Metadata
reference-table work grows with timeline size. A current raw document that cannot
fit its allowance can still cause rejection. Global logical tile authority,
photographic deltas, masks/higher depth, efficient segment relocation, persistent
branches and broader platform/corpus qualification remain. T04/T07/T11/T13 and BF19
stay partial; these measurements do not complete the roadmap.
