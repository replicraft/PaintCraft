# Session history spill and exhaustion

`paintcraft-core::types::storage` defines opaque immutable byte leases and a store
interface without filesystem implementation. `paintcraft-platform::spill` owns
native files, locks, append extents and cleanup. History owns payload interpretation,
checksums, recorded state metadata and publication. Editor sessions compose them;
there is no dependency from platform back to documents, history or UI.

## Preserving publication

The default retention policy is now `Preserve`. Recording an edit stages the
prospective history, including any removal of the old redo path, before publishing
anything. Memory pressure first prepares bounded cold records, then spills cold
payloads if a store is configured. If the retained state still exceeds its allowance,
or any required write fails, the edit fails before publication. The old document,
selection, cursor, redo path and publication epoch remain intact. Saving/reopening,
raising allowances or explicitly selecting trimming can resolve exhaustion.

`Retention::TrimOldest` is the explicit alternative. It preserves the old soft
budget and two-state minimum, with counted/reported removals. Tests of that policy
select it explicitly; low memory no longer silently opts the default into deletion.
The history allowance concerns retained ownership, not process-wide physical memory.
External reader bytes remain separately reported and cannot be forcibly reclaimed.

Recording currently clones history reference tables into a private publication
candidate. It does not copy all pixel data, but metadata work grows with timeline
length. This cost, and the 64 MiB publication staging allowance, require qualification;
the change is not claimed to be an O(1) history graph or a global allocator limit.
Automatic preparation registers complete large snapshots while classifying at
most 64 new tile sources per batch. Other sources retain immutable raw leases
without a pixel scan. Transfers finish classification in further batches, keeping
uniform data inline and applying the configured codec before writing a payload.
Raw sources still owned by hot documents are skipped during automatic transfer;
manual explicit spill retains its full-transfer semantics. Pressure relief repeats
while it makes progress, and can cool the previous state if two hot states cannot
fit. The current state remains hot. All of this work stays in the unpublished
candidate; a failure in a later batch discards every earlier provisional result.
These are bounded batches, not a bound on synchronous time for the whole edit.

## Recorded payloads

Spilling replaces cold tile records with opaque extent handles plus their immutable
raw/patch/codec interpretation. Solid tiles stay inline. New extent handles and
replacement metadata are prepared privately; they replace recorded references only
when the entire requested preparation succeeds. Cancellation or failure drops
unpublished handles and requests garbage collection. A cleanup failure remains
diagnostic and its bytes stay reserved; successful logical publication is not
misreported as an unchanged timeline because optional garbage collection failed.

The active document remains decoded. Restoring a cold state prepares every layer
and selection before changing the cursor or editor state. Raw extents read directly
into an admitted full tile. Patch/compressed extents use at most one bounded input
buffer, then restore into an admitted tile. Storage integrity and the original
full-tile CRC are checked before a decoded tile is installed. Missing, short or
corrupt data produces a retryable error with the current state untouched. A still
live, immutable decoded reader can satisfy a restore without rereading a file.

`MemoryUsage::disk_bytes` counts unique referenced disk payload bodies; headers and
dead extents are not included. `SpillUsage::reserved_bytes` counts the store's
reserved segment lengths, including headers, dead extents and conservative failed
write reservations. It excludes filesystem block rounding, directory/lock metadata,
kernel page cache and allocator overhead. These metrics must not be conflated.

## Native append segments and leases

The desktop configures an 8 GiB allowance per editor and 4 MiB target segments in
an application-specific OS temporary directory. Each editor gets a unique directory
and an OS file lock. Unix directories/files are private (0700/0600). Core sessions
without an injected store still preserve history, rejecting an edit when they cannot
meet its allowance. A filesystem setup failure therefore does not enable trimming.

Each extent has an eight-byte format marker, little-endian body length, SHA-256
body digest and at most 262,144 body bytes. The opaque handle retains the expected
length/digest and its segment lease. Appends serialize within a store. A complete
header/body write returns a provisional handle. Dirty segments synchronize when
they rotate and at a checked `SpillStore::flush` barrier before history publication.
Cancellation is checked again after that barrier. Dirty state follows the segment,
so closing a writer during another caller's preparation cannot bypass a later
barrier. Direct store clients must flush successfully before publishing handles.
This is session storage, not a file-operation durability receipt or power-loss
qualification. Headers and descriptors do not make restart-persistent undo available.

A partial write attempts to truncate back to the previous committed prefix. Whether
rollback succeeds or fails, the writer is closed rather than reused. Existing
leased prefixes remain readable; a failed rollback keeps the full attempted
reservation charged until the segment can be removed. Every actual write can fail
even after quota admission; an allowance is not a free-disk-space guarantee.

An extent keeps its segment alive. Garbage collection closes the active append
file and removes only segments with no remaining leases. Readers can outlive the
store itself. Failed deletion keeps bytes charged and can be retried. A segment
containing live extents may also contain dead extents; those bytes remain charged.
In-place relocation/repacking of live extents is not implemented. Targets are
rotation thresholds, so a single legal extent may exceed a smaller target.

OS locks also protect abandoned-session cleanup across processes. Startup removes
only marked session directories whose owner lock can be acquired. A killed process
releases its lock; a live store or surviving extent lease keeps its owner locked.
Unmarked directories are left alone. Session undo is not promised after restart;
durable document recovery uses the existing separate checkpoint/journal pipeline.

## Validation and remaining qualification

Native tests cover append/read checksums, quota rejection, concurrent readers,
corrupt/truncated/missing data, leases outliving stores, live-lock protection,
process kill/orphan cleanup, permission errors, deletion retries, a disk-full fault
after writing a partial record, failed truncation rollback, failed synchronization,
and the barrier on a closed provisional segment. Permission checks
report when a privileged host bypasses Unix permissions; non-Unix platforms need
their own native qualification.

Editor tests retain and restore complete raw/codec timelines under a small allowance,
check live reader release, reject an edit without losing redo, retry with a usable
store, bound manual spill batches, cancel after an actual write, reclaim provisional
segments and preserve tickets/cursors when a required segment disappears.

Storage operations remain synchronous and session-scoped; the
[native editor](DESKTOP_EDITING.md) now prepares them on its shared worker. Background priorities,
queue-inclusive interactive latency, shared process/disk admission, segment
relocation/GC efficiency, global logical tile authority, restart-persistent branch
history, higher-depth/mask data and photographic before/after deltas remain.
See [history ownership](HISTORY_STORAGE.md) for the warm/raw/codec contracts.

Local [spill measurements](../benchmarks/results/history-spill-2026-10-20/DECISION.md)
retain 1,200 process measurements, 800 internal samples and separate phase traces.
They include real storage synchronization and destruction, with every required
history state retained. Results show material I/O costs and workload-dependent
peak RSS; short noise histories do not establish a peak-RSS improvement.

Large-snapshot regressions cover 130-tile shared bases, a full-surface replacement
requiring more than two transfer batches, later-batch failure preserving redo,
hidden-RGB uniform records without disk, and warm codec completion without disk.
A preparation test registers 80 sources while scanning 64 and deferring 16; raw
leases remain authoritative until classified. CRC validation is mandatory for
encoded and disk-backed records.

The [large-snapshot comparison](../benchmarks/results/history-large-2026-10-20/DECISION.md)
retains 360 process runs, 240 internal samples and six phase traces at identical
allowances. Automatic preparation reduces work versus explicit full preparation
for shared bases; it does not establish lower RSS or uniformly better latency.
