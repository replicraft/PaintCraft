# Recorded history storage

History keeps recent immutable snapshots hot and can replace older snapshots with
recorded metadata and independent tile payloads. Restoring a state never replays a
brush, command, effect, or plugin. Document/state/history IDs, layer order and
properties, selection, off-canvas layer pixels, hidden RGB and finite tile padding
are preserved. Navigation prepares the complete target before changing the cursor,
editor document or epoch. Failed admission or checksum validation leaves publication
unchanged. Editing after a cold restore promotes the preceding state to hot storage.

## Payload ownership

Each distinct immutable tile version has one shared payload while referenced by
packed entries. A solid tile stores its RGBA value inline. A sparse tile stores the
bounding rectangle of pixels different from its layer default, including all four
channels. Reconstruction fills the default and copies the rectangle. This is an
independent recorded representation, not a chain of differences against earlier
states. A full-extent tile retains its original immutable allocation rather than
creating a second raw copy. CRC32 validates reconstructed bytes before use.

Packed records cache decoded tiles and documents through weak references. They do
not pin the decoded allocation of solid or patch payloads. A render/save reader can
retain an immutable lease while history changes. Raster mutation atomically retires
weak identities or performs an admitted fallible copy when a strong reader exists;
weak-reader upgrades cannot bypass pixel allocation admission.

`MemoryUsage` reports decoded Vec capacities, encoded heap payloads, a conservative
metadata weight, and decoded bytes leased beyond hot/raw-owned records. The latter
includes active restored documents and external readers. Retention weight excludes
those separately leased pixels; known weight includes them. These values describe
tracked ownership, not allocator fragmentation, process RSS, GPU memory or a global
physical-memory limit. Reference tables shrink after large retention/branch removal.

## Pressure, admission and failures

Adaptive packing runs under pressure before rejecting an edit or applying explicit
trimming. Each automatic preparation batch considers at most 64 entries and
classifies 64 new tile payloads, with a
separate 64 MiB staging allowance for preserving publication (the explicit trimming
policy keeps its smaller allowance cap). Larger
snapshots register remaining immutable raw sources without scanning their pixels.
Later transfer/refinement batches classify those sources; uniform records stay
inline and checksums exist before encoded/disk records publish. This bounds newly
classified pixel payloads per batch;
it does not establish an interactive latency bound for metadata traversal/accounting.
Manual compaction also accepts cancellation and a staging allowance. Prepared
entries publish together; cancellation/admission failure leaves the timeline intact.

Recent states stay hot when they fit. Pressure can cool the preceding state while
keeping the new current state hot. With default `Retention::Preserve`, a new edit that
cannot fit retained ownership is rejected before publication; it never opts into
history deletion. Explicit `TrimOldest` keeps the former two-state floor and soft
trimming policy. Reader-held bytes cannot be forced free under either policy.
Failed automatic packing is recorded diagnostically; preserving admission can try
native spill and otherwise reject the edit. Undo/redo/seek can fail if decoded
restoration exceeds its pixel allowance. The UI reports that error, and callers
can raise the allowance and retry the same recorded transition.

## Qualification and remaining work

Tests cover actual decoded-allocation release, retained reader leases, exact full
undo/redo, failed/cancelled preparation and restoration, corrupt payload rejection,
metadata/selection/off-canvas preservation, raw fallback, bounded pressure work,
reference-table reclamation, branching after a cold restore and epoch exhaustion.
The release benchmark compares equal complete timelines using shared COW and
32/64/128-edit packing checkpoints, including destruction and correctness checks.
Explicit checkpoints in that experiment differ from automatic pressure policy.

This implements part of T04/T07/T11 and BF18. A global logical tile store, natural/mask/depth codec
comparisons, differences against previous tile versions for photographic edits,
segment relocation, background priority work, branching history graphs,
process-wide physical admission and interactive latency qualification remain.
The timeline still truncates redo when a new edit branches from an older state.

Local [history measurements](../docs/benchmarks/history-packing-2026-10-20/DECISION.md)
retain 1,200 whole-process runs, 800 internal samples and separate phase traces.
Sparse history uses less peak RSS at higher lifecycle/checkpoint cost; full-extent
noise gains no memory. Reader allocation release does not force immediate RSS return.

## Independent compressed blocks

`Session::set_history_compression` selects `None`, `Lz4`, `Zstd1` or `Zstd3` for
newly prepared payloads. Existing recorded payloads retain their encoding. The
default stays `None`; changing this preference neither edits document content nor
silently recompresses old records. Solid payloads bypass codecs. Other tiles are
encoded as independent full 256² RGBA8 blocks and retained only if smaller than
the existing default-relative rectangle or raw allocation. No dictionary, prior
version or effect replay is needed to decode a block. Incompressible tiles keep
shared raw storage. `compressed_bytes` is a subset of `encoded_bytes`.

LZ4 uses `lz4_flex`'s safe, checked slice APIs without framing or allocation features.
Zstd uses the C-backed `zstd` block API at fixed levels 1/3, a maximum window log of
18 and no codec worker feature. Restore requires one exact frame with the expected
content size and consumes the whole frame before CRC verification. Truncation,
trailing bytes, concatenated frames, wrong content size and corrupt output fail.
Both decoders write into an admitted, fixed-size tile; there is no header-sized
output allocation. Checksums provide integrity checks, not authentication.

A compaction operation reuses one output scratch buffer and codec context across
its prepared tiles. Scratch and retained bytes use fallible reservation. Before
codec construction, staging reserves conservative headroom (16 KiB for LZ4's stack
table, 8 MiB for Zstd's native context) plus the codec output bound. Zstd context
size is observed after each block and rejected if larger than the reservation.
`Compaction` reports peak observed workspace and scratch capacity separately.
The native allocator is not intercepted: this headroom reservation and post-check
are not a hard physical admission guarantee. Restoration also uses transient
native decoder state outside decoded pixel accounting. Process resource counters
must qualify that cost; a global native allocator/store budget remains pending.

Cancellation is checked before codec work and after each bounded block. A codec
call cannot be interrupted midway. Preparation failure/cancellation never publishes
a partly replaced timeline. The pressure work limit on new source tiles still
applies. Recorded tests cover source release, exact hidden RGBA, small/raw/solid
fallbacks, insufficient staging/restoration, corruption and cancellation after
encoding. Disk spill, masks/higher-depth codecs and previous-version deltas remain.

API references: [LZ4 block slices](https://docs.rs/lz4_flex/0.14.0/lz4_flex/block/index.html)
and [Zstd bulk buffers](https://docs.rs/zstd/0.14.3/zstd/bulk/index.html).

Local [codec measurements](../docs/benchmarks/history-codecs-2026-10-20/DECISION.md)
retain 1,800 process runs, 1,200 internal samples and separate phase traces. LZ4
reduces structured-tile peak RSS at higher CPU cost; Zstd improves stored ratio
without delivering the best process result. The default remains raw/patch.

Failed restoration also reclaims dead weak aliases registered by earlier tiles
in the private target. A 128-retry corrupted two-tile regression verifies that
these retries neither accumulate aliases nor publish a new cursor/state. Live
reader identities remain retained by the ordinary liveness check.

Native [session spill and preserving exhaustion](HISTORY_SPILL.md) now extend this
ownership model. Preserving rejection is the default; the former soft trimming
behavior is available only through explicit `Retention::TrimOldest`. Historical
benchmark artifacts retain their original source/policy boundaries.

## Detached edit preparation

`Session::preparation` fallibly captures timeline metadata and shares immutable
payloads. `Preparation::apply` can execute on a bounded worker; the published session
retains its document, selection, epoch, history cursor and redo until the owner
calls `publish_prepared`. That call rejects another session, an advanced epoch,
a changed history/staging policy, or cancellation. A save acknowledged while work
was preparing is preserved independently. Dropped results release private payload
leases, including provisional native extents.

Cancellation is checked around command execution and during history classification,
transfer and required synchronization. Individual command kernels and filesystem
calls are not preemptible. Timeline capture still copies metadata synchronously;
the APIs establish no process-wide physical-memory or interactive-latency bound.
The [native desktop consumer](DESKTOP_EDITING.md) now dispatches edits and recorded
navigation through one shared worker, with ordered file-capture barriers. `Preparation::undo`,
`redo` and `navigate_history` restore private targets; `publish_navigation` applies
the same guard. Cold restoration checks cancellation between tiles and before
changing the private cursor. Tests include real worker transfer, same-content foreign
sessions, edit/undo ABA, policy changes, no-ops, save acknowledgements and cancellation
after a native multi-batch spill barrier with exact redo retention and quota cleanup.
Navigation tests cover worker undo/redo/seek, stale and cancelled publication, save
acknowledgement, and every cancellation boundary of a two-tile cold restoration.


## Decoded residency ownership

Raw tile allocations now retain [residency leases](RASTER_RESIDENCY.md) through
recorded history, active documents and external readers. Compaction only returns
payload capacity once every strong decoded owner has released it. Cold undo
materialization reserves capacity again; weak decoded aliases do not pin it.
Default edit/import/restore admissions share a provisional 2 GiB decoded payload
realm. This excludes encoded/native buffers and metadata, and is separate from
history retention and worker declarations. Native [guarded pressure maintenance](HISTORY_PRESSURE.md)
now visits inactive tabs and parked frontend workspaces without changing logical
history or dirty state. Request-sized admission, cross-window notification,
adaptive physical budgets and residency/RSS qualification remain incomplete.
