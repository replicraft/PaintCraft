# Native history spill: preserving ownership at measured I/O cost

Keep native spill as a preserving exhaustion mechanism, with raw/patch compression
still the default. Do not eagerly spill every short timeline or claim interactive
responsiveness from these measurements. The implementation can retain all required
undo states under a small ownership allowance, but synchronous file preparation
and restoration cost time. Native bytes and decoded allocations are not RSS or a
global physical-memory budget. T04/T07/T11/T13 and BF19 remain partial.

## Correctness and publication

History now stages a new record, cold preparation and spill before publication.
If it cannot meet the allowance, or storage fails, the edit is rejected while the
document, selection, cursor, epoch and redo path remain unchanged. The default is
`Retention::Preserve`; trimming requires explicit `TrimOldest` selection. The desktop
injects one native store per editor. Opaque storage contracts live in core, files
and OS locks in platform, and recorded payload interpretation remains in history.

Appends return provisional handles. Dirty segments synchronize at rotation and a
checked final barrier before publishing references; cancellation is checked again
after that barrier. Dirty state survives closing the append file, so a collector
cannot let another caller bypass synchronization. Failed writes attempt prefix
rollback; failed rollback keeps bytes charged and closes the writer. Leases prevent
internal collection of required extents. Missing/corrupt reads fail atomically.

550 workspace tests (20 explicitly ignored), strict all-target Clippy and required
real Metal parity pass. Cases cover complete raw/codec navigation and reader pins,
rejected edits preserving redo, quota exhaustion, provisional cleanup, cancellation
after writing/after synchronizing, missing/corrupt/short segments, permission and
GC errors, partial-write disk-full faults, failed rollback/synchronization, closed
provisional barriers, concurrent readers and killing an owner process. Restart
history is not promised; durable document recovery remains a separate subsystem.

## Complete lifecycle measurements

Three forward/reverse/forward sessions each retain 20 runs and three warmups per
parameter: 1,200 process measurements. The table reports pooled median lifecycle
milliseconds / median peak resident MiB, with every outlier retained.

| Case | Heap raw/patch | Disk raw/patch | Heap LZ4 | Disk LZ4 |
|---|---:|---:|---:|---:|
| Sparse, 256 edits | 74.1 / 51.5 | 105.9 / 52.1 | 77.3 / 52.0 | 110.1 / 52.8 |
| Held readers, 1,000 edits | 307.4 / 103.7 | 464.7 / 107.1 | 332.4 / 103.3 | 462.3 / 105.2 |
| Noise, 128 edits | 36.1 / 34.6 | 179.4 / 34.5 | 41.2 / 35.1 | 181.5 / 35.5 |
| Structured, 256 edits | 71.6 / 67.1 | 354.5 / 52.4 | 95.4 / 56.7 | 144.2 / 54.8 |
| Metadata, 1,000 edits | 69.5 / 4.3 | 76.0 / 4.4 | 69.1 / 4.6 | 76.0 / 4.6 |

Raw disk storage lowers structured peak RSS by about 22%, at about 5× lifecycle
time. Compressed disk is much faster than raw disk for this structured fixture,
but still slower than compressed heap. Sparse, held and short noise cases show
no established process-RSS advantage from spilling. None of these figures measures
a natural-photo corpus, a cold storage device, full-command export or UI latency.

All strategies use the same 768 MiB retention allowance, 16 MiB preparation staging,
64-edit checkpoints and two hot states. Each creates a 256 MiB native store with
4 MiB target segments, so directory/owner-lock setup and teardown are common.
Heap strategies leave payloads in memory; disk strategies spill prepared records.
Every required history state is retained and independently checked through full
recorded undo/redo. Destruction, final store collection and directory removal finish
inside the measured lifecycle. Fixtures/CRC capture/typed commits are included;
compilation, tracing and analysis are not. Calibration constructs only the fixture
and is not subtracted. Source is frozen at `e74bdb7` with exact input/binary hashes.

## Ownership, segment bytes and phases

After releasing readers, noise128 owns 33,816,576 decoded bytes in heap mode versus
524,288 in disk mode. Its 127 cold payloads occupy 33,292,288 body bytes and
33,298,384 reserved segment bytes across ten segments. Peak RSS nevertheless stays
close: the allocator, hot checkpoints, transient reads and kernel caching matter.
Do not turn logical allocation release into an immediate OS-page-return claim.

Structured256 raw heap owns 67,371,008 decoded bytes. Raw disk retains 524,288
decoded bytes and 66,846,720 disk body bytes, reserving 66,858,960 segment bytes in
20 segments. Disk LZ4 stores 4,953,112 body bytes, reserving 4,965,352 bytes in four
segments. Metadata1000 shares one tile and spills one four-byte payload (52 bytes
including its header); there is no per-step pixel-copy or per-step disk record.
`ownership-summary.csv` includes logical bodies, reserved lengths, segment counts,
reader pins, scratch/context metrics and current RSS before/after reader release.
Filesystem allocation rounding, lock/directory overhead and page cache are excluded
from reserved-length accounting. Whole-segment dead space remains charged.

Two parameter orders retain 800 raw Criterion samples across 40 invocations. Their
JSON/CSV and command records are archived. Significant variability remains: reverse
metadata1000/disk has a 387 ms sample median and 1,014 ms maximum, far above its
process median; forward sparse256/disk-lz4 has a 103 ms median and 692 ms maximum.
No cause is assigned. Internal and process boundaries differ; these samples do
not qualify precise universal rankings or a service-level tail guarantee.

Separate single-replay traces record every phase. A checkpoint includes cold
packing plus spill, synchronization and GC when enabled. Structured raw-disk undo
p95 is about 0.187 ms versus 0.002 ms on heap; its checkpoint maximum is 62.48 ms.
The noisy raw-disk checkpoint maximum is 159.17 ms. Held1000/raw-disk undo has an
8.67 ms maximum despite a 0.036 ms p95. Navigation timing excludes subsequent
fingerprint checks; edit cycles include capture/checkpoints. Final extra packing
and store teardown remain in complete lifecycle timing but outside per-edit events.
Tracing and RSS subprocesses do not run in accepted throughput measurements.

## Pilot and remaining gates

The original per-extent-sync pilot is preserved in `excluded-per-extent-pilot.tar.gz`
and its runner log. It was deliberately interrupted during held1000 after the
completed sparse256 session showed excessive synchronization overhead. It is not
included in accepted summaries or used for a qualified speedup ratio. The final
implementation batches the same required barrier before publication.

The OS temporary filesystem and page cache are not flushed. These are decoded-cold
restores of newly written session files on one Apple M1 Pro host, without power,
affinity or thermal control. Required bytes may still reside in kernel cache.
Physical RSS, system cache, GPU residency and per-store quota are separate scopes.
There is no global process/disk allocator qualification or Windows/Linux native run.

Automatic packing still skips snapshots requiring more than 64 new source scans;
large documents can reject an edit despite available disk space. Manual preparation
supports larger snapshots, but incremental logical-store work remains required.
Background priorities, queue-inclusive latency, efficient relocation of live segment
extents, photographic before/after deltas, u8 masks/higher depth, persistent branch
history and historical-accounting regression measurements remain. Target size and
spill thresholds are bounded defaults, not statistically selected optima.
