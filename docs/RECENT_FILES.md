# Desktop recent files

Classic and Studio now offer a working Recent files chooser. Windows 11 exposes
it from File → Recent, Windows 7/10 from their Recent pictures panel, and Studio
and the native macOS menu from File → Recent files. All labels, empty states,
busy messages and clear/cancel controls have seven-language translations.

The list contains the 16 most recently opened/imported or successfully saved
paths, newest first. Existing canonical paths deduplicate aliases. Choosing a
picture imports it again; it does not bind the document to an image export.
Exports, recovery checkpoints, cancelled dialogs, failed opens and unconfirmed
saves do not enter the list. Confirmed native save reconciliation does.

Opening a record calls the existing Open path. Classic asks before discarding
dirty work. Studio reuses an already-open binding or creates a document tab,
retaining the other tabs and their histories. The chooser works with no Studio
tabs open. Pending edits and native I/O disable file choices until their results
are ready. Unfinished compound shapes retain the normal finish-before-file guard.
Missing/unreadable files leave the current document and history intact, report
the ordinary open error and remain in the list for a later retry.

Clear recent history removes metadata entries only. It does not delete pictures,
change document contents, reset undo history or close tabs. The list belongs to
application preferences; it does not share document sessions between frontends.

## Persistence and limits

`recent-files.json` lives alongside `settings.json`, using its own
`paintcraft.recent-files/v1` schema. Old settings remain compatible. Startup opens
are merged with the loaded history. A bounded read accepts at most 16 entries and
65,536 encoded native path bytes per entry, with an overall file limit of about
2.1 MB. Relative, malformed, oversized and foreign-platform encodings are rejected.
Unix bytes and Windows UTF-16 code units are stored losslessly; display text alone
may substitute characters when the UI cannot represent a filename.

An unusable history is moved aside to an available `recent-files.json.unusable-N`
name. If it cannot be preserved there, automatic writes stay disabled until the
user explicitly clears the list. Metadata uses the existing atomic save adapter.
History failures appear in the chooser and do not invalidate a confirmed document
save or its binding. The bounded preference I/O remains synchronous; no new native
latency or physical-memory qualification is claimed. Concurrent application
processes do not merge their lists; the latest metadata save wins.

The browser keeps this action disabled. It does not persist native paths it cannot
reopen. Browser file-handle retention and broader recent-file reference behavior
remain future work. CLI file commands continue to use their explicit paths.

## Evidence

Store tests cover MRU bounds/order, alias deduplication, non-UTF-8 metadata,
malformed/oversized records, quarantine, blocked preservation and clearing without
changing picture files. Host tests cover startup loading, imports, failed opens,
confirmed legacy/native saves, metadata failure isolation, Classic dirty guards
and Studio tab reuse. The UI capability test verifies empty workspaces, blocked
modal states and unsupported hosts.

The real GUI chooser is exercised in every language and all three Classic skins
plus Studio, with forced multiple layout passes and both display densities across
the matrix. Tests open actual PCND files through the controls, clear persisted
entries and compare source bytes, document semantics and history tickets. A
separate Studio GUI test closes the last tab and reopens it through the list.
These are headless application tests; live OS-menu interaction, platform faults,
Windows/Linux file behavior and native latency remain independently unqualified.

Validation passed: 299 desktop library tests (8 ignored), 274 desktop GUI tests
(63 ignored), 69 browser tests, and 38 shared UI tests (1 ignored). Strict Clippy
for the changed application/UI targets, workspace checking, the Wasm target check,
and the portable browser release build passed. Formatting, source-length checks
and the seven-language translation audit also passed. Ignored tests were not
counted as executed.

Raw local logs are kept under `docs/validation/recent-files-2026-10-24/` (ignored).
T16/T19 remain partial because printing, acquisition, clipboard, additional image
operations, effects and broader native reference parity are still incomplete.
