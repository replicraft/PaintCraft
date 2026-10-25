# Canvas clipboard Cut

Cut is enabled in Classic 7/10/11 and Studio on desktop, and Classic in the browser.
Use Edit > Cut, the older skins' Home > Cut button, or Ctrl/Cmd+X. Text widgets
and editable text drafts retain input ownership. Canvas Paste remains unfinished;
[shared image-placement foundations and CLI insertion](PASTE_IMAGES.md) are available.

## Source and acknowledgment

Cut captures the active visible layer's raw RGBA pixels using the same exact
canvas-clipped bounds, binary masks and budgets as [Copy](CLIPBOARD_COPY.md).
Without an active selection, it captures the canvas area. Empty selections and
unresolved canvas gestures, text, shape or floating drafts cannot start a Cut.

The private capture records the source session owner, publication epoch, state,
selected field, target layer and clear color. The clipboard acknowledgment alone
allows attempting removal; it never bypasses the editor transaction guard. Before
removal, the host rejects changed documents, selections, targets and new drafts.
Native completion also checks its document tab. Undo then returning to the same
content still changes the epoch, so an older Cut cannot remove it.

Failed writes preserve source pixels and the previous internal capture. Escape
cancels a pending Cut and drops its write job. An opaque OS call or browser promise
may already have written the clipboard; cancellation prevents deleting pixels and
cannot undo that external write. A successful but stale Cut becomes a Copy, retains
its exact acknowledged capture and reports why removal was cancelled. Switching
to another native tab cannot cause a late Cut to delete that tab's content.

## Removal and Undo

Classic clears to its captured canvas ground. Studio clears to transparent black.
These are explicit PaintCraft policies pending native reference qualification.
The removal affects only the canvas, finite target-layer extent and exact selected
coverage. Other layers, off-canvas storage, hidden RGB outside that field, metadata,
canvas dimensions and the selected field remain unchanged.

A changed removal records one **Cut** history entry. Undo restores exact source
pixels and selection; Redo reapplies the recorded result. Cutting pixels that
already equal the clear color creates no empty history entry. Allocation,
preparation or publication failure leaves the document intact while retaining
the acknowledged clipboard capture. Native removal uses the normal asynchronous
editing worker, capacity retries and guarded publication.

## Shared command

`paintcraft.batch/v2` adds `raster.cut_selection` with exactly `op`, `layer_ref`
and `clear` fields. `layer_ref` uses the existing ID/alias grammar; `clear` is a
straight RGBA hex color. Version 1 rejects this operation.

This primitive implements **only removal**, with no clipboard read/write. The
native/browser host must acknowledge its clipboard write before submitting it.
CLI batches can use this explicit removal primitive and the same atomic/history
semantics; a CLI invocation does not claim to populate an OS clipboard.

The shared compiler uses the existing admitted paint plan and exact mask filter.
The operation composes correctly with preceding writes and rolls back with a
later failing operation. The session base guard remains active through detached
preparation and final publication. Clipboard budgets and their limits are
unchanged; no performance or physical-memory qualification is claimed.

## Evidence and remaining qualification

Independent all-pixel oracles cover rectangles, concave/inverted binary masks,
negative layer origins, finite extents, hidden RGB, partial alpha, off-canvas
preservation, both clear policies, Undo/Redo and unchanged metadata. Engine tests
inject every observed cancellation checkpoint, reject decoded admission and test
late mixed-batch rollback. Guards reject foreign sessions, target changes,
selection changes and restored content with a newer epoch.

Native host tests use fake clipboard acknowledgment and include the real editing
worker, coalesced Cut/key events, delayed success, write failure, Escape, new drafts,
IME, changed sources and tab changes. Actual Cut controls run across all three
Classic skins and Studio in English/Japanese. Browser host tests cover successful
removal, failure, Escape, replacement owners, drafts, source changes and quota
denial after acknowledgment. The real CLI reopens the output and checks exact
selected/unselected pixels, unchanged input and destination preservation on v1
rejection. Diagnostics are translated across all seven locales.

These are portable/host checks. The unchanged browser PNG adapter has earlier
active-layer Copy runtime evidence; this tranche adds no new browser-runtime,
cross-application, Windows DIB/PNG, Wayland/X11 or matched Paint/Paint.NET
qualification. Logs are local in ignored `docs/validation/clipboard-cut-2026-10-24/`.
