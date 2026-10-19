# Recovery store v1

This is the owner of the application's private recovery encoding. It implements the
ordering requirements in the reference pack's `SAVE_RECOVERY.md`. It does not change
PCND or the command wire formats. Recovery includes supported semantic document content;
undo history is not persisted.

Each session has a random UUID and an advisory-locked `<session>.lock`. Its writer is
serialized by a mutex. Scanners and explicit discard acquire the orphan's lock; a live
session is not offered or deleted.

| File | Contents |
|---|---|
| `<session>.payload-<uuid>.pcnd` | Immutable full semantic snapshot |
| `<session>.journal-<20-digit-sequence>.json` | One immutable commit segment |
| `<session>.head-0.json`, `.head-1.json` | Alternating checkpoint anchors |
| `<session>.cleared` | Durable discard marker, authoritative over old files |

A commit envelope has `schema: paintcraft.recovery-commit/v1`, `body`, and `sha256`.
The checksum is SHA-256 of canonical body JSON. The body contains a positive decimal
string `sequence`, `previous_sha256` (null for sequence 1; otherwise the previous
segment's complete-byte digest), payload UUID, payload byte length and SHA-256,
document/state UUIDs, and optional original path for display. Paths to payloads are
constructed from validated UUIDs, never read from an arbitrary path field.

A head envelope has `schema: paintcraft.recovery-head/v1` and the same body checksum
rule. Its body contains `sequence` and `commit_sha256`. Each head anchors the complete
snapshot at a validated journal position; this permits reclaiming earlier segments.
Writers use canonical JSON. Readers require exact supported fields, canonical UUIDs,
positive decimal strings and lowercase digests. Checksums detect corruption; they are
not authenticity proofs.

Publication order is:

1. Stream the snapshot into a bounded same-filesystem staging sink, validate its digest,
   sync the file, create the immutable payload, read back its digest, and sync the directory.
2. Create and durably publish the checksummed commit segment. Only then advance the
   writer's journal position. An uncertain segment write stops further writes pending
   reconciliation.
3. Publish head `sequence % 2` through the safe-save adapter. This acknowledges a durable
   checkpoint. A crash between steps 2 and 3 can be recovered from the journal tail.
4. Keep payloads/segments for both head generations and any unpublished tail. For older
   generations, remove segment references and sync the directory before reclaiming
   payloads, then sync again. Cleanup failure is a warning on an already durable checkpoint.

Scanning validates head checksums, exact referenced segment digests, payload sizes and
hashes, and native semantic admission/identities. It selects a valid checkpoint anchor,
then reads only consecutive, predecessor-matching, fully backed segments. A missing,
truncated, corrupt or unsupported tail stops replay; later segments cannot bypass it.
A missing/corrupt latest payload falls back to the preceding retained checkpoint.
With no valid head, the journal can be reconstructed only from sequence 1. Files whose
validity cannot be established are retained and reported.

Background jobs carry an owner-scoped `RecoveryToken`. Invalidation advances a checked
atomic epoch without waiting on filesystem work. Checkpoint publication checks that
token at admission and before commit/head publication. The coordinator queues cleanup
under the new token before admitting a new episode; superseded checkpoint and cleanup
jobs cannot recreate or remove the newer episode. Backend serialization keeps in-flight
old publication ahead of accepted cleanup even if invalidation occurs during a file write.

Discard durably publishes the session-specific marker before deleting its files. A
scanner finishes interrupted discard rather than reviving old segments. Subsequent
completed edits may begin a new episode after successful cleanup. Only canonical owned
filenames, including authorized safe-save staging filenames, are removed; unrelated
files and other sessions are retained.

A recovery offer records the exact snapshot SHA-256 and document/state IDs. Claiming
it checks its owned filename, acquires its existing orphan lock, rejects a durable
discard marker, and rechecks content and semantic identity. The claim holds that lock
through preparation and exposes a scoped discard capability. Cancelling releases the
lock without deleting the source. These are cooperative local-filesystem checks, not
OS confinement or exclusion of arbitrary external writers ignoring the lock.

Provisional limits are 1 GiB per snapshot, 128 KiB per record, 32 KiB for the original
path display string, 4 GiB retained snapshot payloads per writer, 64 retained/tail
segments, 256 owned files per session, and 100,000 directory entries per scan. Payloads
use the native parser's document/raster admission limits too. These are resource limits,
not measured memory or performance qualifications. Process-wide disk admission remains
pending. Both recovery reading and the desktop restore path use bounded reads. The status tooltip
reports the last confirmed generation/time and whether it contains the current state.
A confirmed checkpoint or explicit native save protects the restored document before
its old orphan is discarded. A failed handoff retains the original copy under its
claim; an unavailable recovery store does not prevent loading the document or saving
it explicitly elsewhere.

Validated legacy `<session>.pcnd` / `<session>.json` checkpoints remain recoverable.
Malformed legacy data is retained with a diagnostic. New checkpoints use the journal.
The protocol has local macOS filesystem tests for ordering, injected interruption,
corruption, tails, GC, locks and discard. This is not power-loss, Windows, network-filesystem
or release qualification. Native periodic checkpoints and cleanup use the bounded
save FIFO; stale completions cannot advance the active session. Native startup queues
discovery and initialization after window creation, using editor/configuration fences
and retrying unadmitted initialization when capacity returns. Unused owners are retired
through the same queue. Active-store directory migration is not yet offered; existing
protection is retained. Native restore decoding, initial checkpoints, source retirement
and explicit orphan discard use the same admitted I/O service. The deterministic UI
harness retains synchronous checkpoint services. Benchmark evidence remains pending.

Native restore keeps one bounded pending intent when read admission is unavailable.
Publication checks the editor session, StateId/epoch, binding generation, recovery
configuration, outstanding tool input and layer edit drafts. Ask Later or close can
cancel preparation; in-flight reads are not preempted, and rejected decoded data is
disposed on the worker. A completed claim becomes the current editor's handoff source.
Its lock remains held until a checkpoint/save is confirmed or the user explicitly
discards that document. Tests cover changed/deleted sources, edits/ABA during reads,
new Save As choices, cancellation, unavailable replacement storage and deferred handoff.


## Captured native save attempts

`saved-attempts/` is the only child namespace included in general recovery discovery.
Each captured native save has an independent ordinary recovery owner, immutable PCND
snapshot, commit/head, and `<owner>.save.json`. Active owner locks suppress offers;
source claims protect orphaned snapshots during restore. The additional `save.json`
filename and its safe-save staging names belong to that same owner and are reclaimed
under its durable discard marker. The snapshot survives removal of the original file.
Restoring it creates unsaved, unbound work and confirms new protection before retiring
the source. Stored receipts never grant write or binding authority in a new session.

The private `paintcraft.saved-attempt/v1` envelope contains `schema`, `body`, and the
SHA-256 of canonical body JSON. Body fields are owner UUID, editor session UUID, document
UUID, source StateId, decimal save sequence/binding generation, native save operation,
phase, and optional `paintcraft.file-operation/v1` receipt. Identity, field, checksum,
phase and stamp validation are bounded to 256 KiB; malformed evidence is retained.
Receipt damage does not invalidate an independently validated semantic snapshot.

Phases are `capturing` (before snapshot publication), `captured` (complete snapshot),
`prepared` (staging validated, receipt persisted before user-visible replacement), and
`finished` (observed save outcome). Capturing/captured records have no receipt. Prepared
records explicitly have unknown replacement, unconfirmed durability and no
acknowledgement: a process can exit on either side of replacement before recording its
final result. Finished records preserve the real outcome and original operation ID,
sequence and binding generation. A later in-process reconciliation can persist a revised
finished receipt for that same attempt without repeating replacement. Restarted UI
currently offers document recovery; destination reconciliation from persisted evidence
remains a separate pending tranche.

Admission uses one cooperative `.admission.lock`, a 64-owner limit, 1,024-entry cap
(with file slots reserved for new publication), and a conservative 4 GiB disk policy
that reserves a whole maximum 1 GiB snapshot plus 64 MiB metadata slack before starting.
Unrelated and corrupt files consume admission rather than being deleted automatically.
Scanning this fixed namespace is separately capped at 1,024 entries. These bounds do
not qualify physical disk/RSS behavior, external writers, network filesystems or power
loss. If configured retention cannot be established, the native save is not attempted
and its captured document remains available in memory; the UI distinguishes confirmed
restart copies from volatile work. Successful saves or explicit forget requests enqueue
retirement through the same native FIFO, preserving failed cleanup for inspection.
