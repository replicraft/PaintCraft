# Captured-content guards for in-place CLI saves

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

Desktop open-to-save bindings do not yet retain and advance the expected content
through ordered saves, Save As, recovery and reconciliation. Their existing
save-time checks remain unchanged. That integration, user inspect/reload/copy
choices and broader T32/FILE-07 qualification remain unfinished.
