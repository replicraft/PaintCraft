# Browser Paste as new layer

The browser editor now offers **Paste as new layer…** through the shared Edit
menu and **Ctrl/Cmd+Shift+V** when the browser exposes image clipboard reads.
Classic 7/10 expose the shortcut; Classic 11 also has the Edit menu item. [Ordinary Paste](FLOATING_PASTE.md) now supports private existing-layer placement
through Ctrl/Cmd+V. Native ordinary Paste remains unfinished.

The reader calls `navigator.clipboard.read()` directly from the user action,
without ambient polling or changing permissions. It requests the first advertised
`image/png` item, obtains its Blob and validates the encoded signature before
using PaintCraft's bounded PNG decoder. Browser API availability is checked at
runtime; denied access reports failure rather than success. The API and format
contract follow the [W3C Clipboard specification](https://www.w3.org/TR/clipboard-apis/).
Safari and Firefox behavior have not been qualified.

## Staging and ownership

The read result belongs to the captured document/session, history epoch and
Classic skin. Selection/document/skin changes, private gestures and conflicting
modals reject late results. Escape cancels a pending read; weak completion handles
prevent cancelled receivers from retaining late pixels. Existing text and IME
input take priority over the image shortcut. Copy/Cut and Paste serialize their
clipboard work.

The decoded image remains private until the dialog accepts its name, X/Y position,
optional right/down canvas growth and known import limitations. Unknown losses
fail closed. Growth is checked against the existing browser 4 megapixel limit
both in the dialog and before publication. Acceptance uses the shared editor's
source-bound atomic Paste transaction, creates a topmost layer, selects its
visible extent and records one undoable change. Remembered canvas size updates
only after successful growth. Cancel does not change the document.

## Bounds and limits

Encoded images are limited to 32 MiB. A shared 64 MiB encoded payload budget
reserves the ArrayBuffer plus Rust copy before materializing bytes. Retained
vector capacity beyond advertised length is separately admitted. Before decoding,
the host reserves 16 MiB from the same 64 MiB raw clipboard budget used by Copy;
only the resulting vector's capacity transfers into its immutable image lease.
The browser pixel limit remains 4,194,304. Excess item/type counts fail closed.

Browser-native ClipboardItem/Blob storage, garbage collection, allocator overhead
and codec scratch are outside these payload budgets. PNG decoding and the editor
transaction currently run on the wasm UI thread, as browser file imports do;
worker decoding, latency and physical-memory qualification remain open. Browser
sanitization/native interchange may change external image data; this does not
establish exact internal-copy or cross-application color/alpha fidelity.

## Validation

All 105 browser library tests and the existing preview integration test pass.
New tests cover exact hidden RGB/partial alpha in decoded and pasted pixels,
capacity transfer/release, malformed/signature-mismatched PNGs, cancellation,
selection/session/skin changes, loss consent, stale confirmation, canvas growth
limits and actual dialog controls across Classic 7/10/11 in English/Japanese.
Native and wasm strict Clippy, formatting and seven-locale checks pass.

A fresh release wasm build was tested in HeadlessChrome 156.0.8078.4, using the
headless shell's independent in-memory clipboard and a disposable browser context.
The actual Cmd+Shift+V shortcut opened the dialog without dirtying the document;
acceptance published a layer. Actual Edit > Copy then returned the exact two-pixel
red/blue PNG. Undo restored the clean document. A subsequent actual Edit > Paste
as new layer with clipboard-read denied produced NotAllowedError and no edit.
Screenshots were inspected and no browser exceptions were recorded. The general
OS clipboard was not accessed. Logs, source hashes, screenshots and the CDP
harness are local in ignored `docs/validation/browser-paste-layer-2026-10-24/`.
