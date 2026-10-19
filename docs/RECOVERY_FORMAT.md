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

Provisional limits are 1 GiB per snapshot, 128 KiB per record, 32 KiB for the original
path display string, 4 GiB retained snapshot payloads per writer, 64 retained/tail
segments, 256 owned files per session, and 100,000 directory entries per scan. Payloads
use the native parser's document/raster admission limits too. These are resource limits,
not measured memory or performance qualifications. Process-wide disk admission remains
pending. Both recovery reading and the desktop restore path use bounded reads. The status tooltip
reports the last confirmed generation/time and whether it contains the current state.
A restored document is checkpointed under its new session before its old orphan is
discarded. Failed handoff retains the original copy.

Validated legacy `<session>.pcnd` / `<session>.json` checkpoints remain recoverable.
Malformed legacy data is retained with a diagnostic. New checkpoints use the journal.
The protocol has local macOS filesystem tests for ordering, injected interruption,
corruption, tails, GC, locks and discard. This is not power-loss, Windows, network-filesystem
or release qualification. Native periodic checkpoints and cleanup use the bounded
save FIFO; stale completions cannot advance the active session. Initialization,
discovery/restore and the first recovery handoff retain foreground paths. The deterministic
UI harness retains synchronous checkpoint services. Benchmark evidence remains pending.
