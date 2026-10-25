# Paste as new layer

The native macOS app now offers **Edit > Paste as new layer…** and
**Cmd+Shift+V** in Classic and Studio. This reads a PNG or classic TIFF image from
the clipboard and stages it privately. The dialog lets you name the layer, set
its X/Y position, optionally grow the canvas right/down, and accept known import
limitations before applying. Cancel or Escape leaves the document unchanged.

This is a PaintCraft extension. Ordinary Paste/Cmd+V, active-layer placement,
on-canvas dragging, Windows/Linux/FreeBSD readers remain
unfinished. [Browser Paste as new layer](BROWSER_CLIPBOARD_PASTE.md) now connects
the same transaction to supported browser PNG reads. The legacy Paste control stays disabled. Classic 7/10 expose the
extension through the shortcut and native macOS Edit menu.

## Ownership and publication

The reader is created lazily on explicit activation. Encoded reads and decoding
run off the UI thread. A source token binds the result to its tab, document,
history epoch and frontend. Source changes, new private gestures, conflicting
modals or workspace switches discard the pending result. Completion status goes
to the originating tab. Copy/Cut and image reads serialize their clipboard work.

The encoded snapshot has a 64 MiB capacity limit. Before decoding, the host
reserves a 64 MiB output grant from the existing shared 128 MiB raw clipboard
budget. The codec enforces the existing pixel limit; the resulting vector
transfers its actual capacity into an immutable image lease without another
pixel copy. Encoded storage and unused output credit retire before readiness.
Codec scratch and native pasteboard allocations are outside these payload
budgets; codec calls are not forcibly interruptible. Cancellation discards late
results and never publishes them.

Known decoder losses require explicit consent. Unrecognized losses fail closed.
The source-bound draft then enters the normal asynchronous editing worker and
resource admission path. Capacity retries retain the frozen image without
reading or decoding again. A successful operation creates one topmost layer and
one **Paste** history entry; Undo restores the previous document/selection.
Canvas growth participates in the existing remembered-size preference.

## Validation

Seven host tests use an injected reader and real encoded PNG decoding. They cover
actual controls and shortcuts in Classic 7/10/11 and Studio, English/Japanese,
position/growth, loss consent, invalid placement, source/tab changes, cancellation,
malformed data, denied capacity, worker publication and frozen-image retry.
Fourteen existing native Copy/Cut tests pass alongside them. Core clipboard tests
cover reservation transfer, final-reader release and late cancellation. Shared
UI tests verify the capability gate and distinct shortcut; all web tests pass
with image Paste still disabled.

Strict native and wasm Clippy, formatting and seven-locale translation checks
pass. Logs are local in ignored
`docs/validation/clipboard-paste-layer-2026-10-24/`. No window or general OS
clipboard was accessed by these tests. Real cross-application image fidelity,
physical-memory/latency qualification and reference Paint behavior remain open.
