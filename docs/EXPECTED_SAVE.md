# Captured-content guards for native saves

This advances T32 and FILE-07 without completing native destination authority or
filesystem qualification. It extends the existing save adapter and receipts;
the batch grammar and file-receipt schema are unchanged.

`apply` and `paste-image` now capture whether the destination refers to their
input before acquiring input bytes. An in-place native output passes the SHA-256
of those exact acquired bytes into the platform save adapter. A separate output
keeps its existing Save As behavior. `--expect-input-sha256` continues to guard
input acquisition before parsing; the new check also applies when that option is
omitted.

The save adapter already resolves and hashes the current destination before
staging. `save_with_expected_sha256` compares that observed digest with the
captured input digest before creating a staging file or invoking the encoder.
A missing or changed destination refuses replacement. Invalid expected digests
fail before destination access. Matching bytes follow the existing staging,
validation, preparation, replacement and durability protocol, including its
later destination fingerprint check.

Detecting in-place intent before acquisition matters when an external writer
deletes the input while the command is working. That disappearance must not turn
a Save into Save As and silently recreate the file.

The command may have completed its private in-memory edit before detecting a
file conflict. The JSON response therefore retains the successful engine result
with `file-incomplete`, a `not-replaced` receipt and exit 6. The external file is
left intact, or remains absent if it was deleted. No automatic retry, rebase or
overwrite occurs. The caller can acquire the current source again or choose a
separate output.

## Evidence

Three platform tests cover missing/changed content, equal-length changes,
invalid expectations, refusal before encoding/staging, successful matching
content and an external modification made during encoding. The latter confirms
that the existing late conflict check still preserves the external bytes and
removes the private staging file.

The real CLI integration test covers both commands against replacement and
deletion after source capture. A FIFO reader/writer handshake establishes the
ordering: the child has acquired and hashed the document before it opens the
operations/image FIFO. The test changes the source before supplying the payload.
It checks the actual process exit, successful engine receipt, retained Save
intent, known non-replacement, exact external bytes/absence and lack of staging
debris. Unix tests use the POSIX `mkfifo` utility and safe nonblocking `rustix`
opens; rustix is a workspace-inherited, Unix-only development dependency of the
CLI. The production CLI gains no clipboard/UI dependency.

Validation passes 90 platform tests (four opt-in native checks ignored), all 39
CLI tests, and strict CLI/platform host all-target Clippy. Platform cross checks
also pass for Linux x86-64/ARM64, FreeBSD x86-64 and Windows x86-64/ARM64. These
cross checks do not establish native filesystem behavior on those systems.

Raw logs and source identity are kept locally in ignored
`docs/validation/expected-save-2026-10-25/`.

## Limits and remaining work

This checks content observed by the existing adapter, not a universal
compare-and-swap against arbitrary external writers. Existing final-check to
rename races remain. Equal content does not establish exclusive namespace,
inode or association authority, and same-content ABA changes are not rejected.
Symlink retargeting, hardlink/case aliases, shared destinations, network/sync
providers and cooperative locking need their own authority rules and native
qualification.

## Desktop association bindings

The desktop Open path now pins a canonical source namespace before its bounded
acquisition and computes the expected SHA-256 from the exact acquired bytes.
`Editor::open` uses that same bounded acquisition. Imported pictures and recovered
snapshots remain unbound documents. Neither recovery restoration nor an undo
entry restores write authority over an original file.

Each editor association owns a shared expected version. Queued implicit Saves
retain that association rather than copying the initial digest into every job.
The I/O worker checks the current expected digest and namespace before staging,
then advances them after an acknowledged replacement, before UI completion.
Thus two queued saves can follow each other while an external replacement or
deletion is refused. The binding lock is never held during disk I/O; concurrent
attempts through the same binding refuse dispatch. A worker panic disables that
binding because it cannot establish the external outcome.

Known non-attempts/non-replacements retain the previous expected content. Unknown
outcomes block later writes through that association until resolved. An
acknowledged replacement advances its expected content even if requested
durability remains incomplete; existing receipts, dirty state and recovery still
require qualified success before clean marking. Live reconciliation can strengthen
the same operation's binding. Its operation ID prevents an older inspection from
regressing a binding advanced by a later write.

Save As uses a new, explicitly authorized association. Existing session,
generation and sequence checks still decide whether the UI adopts it. It does
not mutate the original association. Synchronous failed Save As restores both
the old path and its original expected version. A successful Save As establishes
the new baseline for subsequent implicit Saves. Parked tabs retain their editor's
binding; the worker and incomplete-attempt records retain their own shared owner.

The platform's `Expectation` contains the captured path-free namespace identity
and SHA-256. `save_with_expectation_and_preparation` checks both before encoding,
then uses the existing write-ahead recovery callback and late fingerprint check.
Changing an opened symlink to another equal-content file fails the namespace
check instead of authorizing that other target. This remains a restriction on a
parent-authorized write, not a transferable filesystem capability or a universal
cross-process compare-and-swap.

Trusted embedded callers can still select a new authorized destination through
`Editor::with_document(..., Some(path))` or a changed `set_path`. Existing files
should be acquired through `Editor::open`; ordinary desktop Open uses the guarded
acquisition. Reassigning the same path preserves its current guard.

Three binding tests cover expected-version advancement, unknown/panic/concurrent
attempt handling and stale reconciliation. Six real desktop coordinator tests
cover queued Saves before completion delivery, external replacement/deletion,
Save As adoption, synchronous failed Save As restoration, live reconciliation
and symlink retargeting in both synchronous and asynchronous paths. The platform
also tests equal content in a different namespace and write-ahead preparation.

The follow-up passes 388 desktop library tests (nine opt-in captures ignored),
91 platform tests (four native checks ignored), 105 formats tests and 39 CLI
tests. Strict desktop/platform/CLI all-target Clippy and Windows x86-64/ARM64,
Linux x86-64/ARM64 and FreeBSD x86-64 platform cross checks pass. No window or
general clipboard was opened. Logs and source identities are local in ignored
`docs/validation/desktop-save-binding-2026-10-25/`.

Dedicated inspect/reload/copy/explicit-overwrite conflict controls, preconditions
for an existing Save As target between user authorization and worker acquisition,
full alias/provider authority, final-check/rename races and native filesystem
qualification remain unfinished. Ordinary conflict status and Save As to a
different output remain available. Restart inspection stays separate from current
editor association adoption. This does not complete T32 or FILE-07.
