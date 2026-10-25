# Browser ordinary Paste and private placement

The browser editor now connects **Paste / Ctrl+V / Cmd+V** to an existing visible
layer in Classic 7, 10 and 11. Classic 7/10 have a working ribbon Paste button;
Classic 11 exposes Paste in Edit. **Paste as new layer** remains a separate action.
Native ordinary Paste remains unavailable; native macOS new-layer Paste is
unchanged. This is not a reference Paint clipboard-fidelity qualification.

## Workflow

An explicit activation starts the existing bounded PNG read/decoder pipeline.
The consent dialog shows image size, initial X/Y, optional canvas growth and known
import limitations. Ordinary Paste does not ask for a new layer name. Accepting
this dialog stages a private image and selects the selection tool; it does not
publish pixels, canvas size, selection or history.

Drag inside the pasted rectangle to position it. Pointer release retains the
image for another drag. Arrow keys move one pixel, Shift+arrow moves ten. Enter,
Apply or a primary click outside the image on the canvas places it. Escape,
Cancel or Ctrl/Cmd+Z cancels without navigating the owner's existing Undo history.
Focus/pointer loss stops the current drag while preserving the private image.

The placement bar shows instructions and document X/Y. Selection dimensions and
the outline follow the private image. View zoom can change without resampling the
incoming source. The browser unsaved-work guard includes a floating image. Other
editing, layer and save operations require explicit placement or cancellation;
file output cannot silently omit the private image.

## Pixels, preview and publication

The source tab/session, document/history epoch, Classic skin and target are
captured before reading. A changed source/selection/target/skin or incompatible
tool rejects the private result. The decoded image remains immutable through
repeated positioning; pointer movement never samples a previous preview.

Ordinary Paste uses explicit source-over blending onto raw target samples. The
transparent-selection option captures an RGB color key from the secondary color
when placement begins. Target opacity stays a display property. The old selection
does not clip the incoming pixels. Higher layers composite above the target in
both preview and final output.

`Session::preview_paste_image_onto` shares the exact staged raster operation with
publication and returns a private document without allocating history credit.
The browser renders it only when the source/position revision changes. Preview is
clipped to the current canvas; requested growth becomes visible on placement.
The same source-bound Paste transaction publishes pixels, target extent,
selection and optional right/down canvas growth as one **Paste** history entry.
Undo restores the full previous document and selection. Remembered canvas size
updates only after a successful growth publication.

Invalid motion/growth retains the last valid position. A failed publication keeps
the frozen pixels and placement so it can be retried or cancelled. The browser
4 megapixel canvas limit and existing raw-image, tile, metadata and sample limits
remain in force. Preview and publication still run on the wasm UI thread. Large
image latency, physical memory, workers and broader device/browser behavior remain
unqualified; this change does not claim a speedup.

## Validation

Shared placement tests cover repeated relative drags, foreign releases, invalid
coordinates, stable revisions and exact retained source bytes. Editor tests verify
that preview needs no history credit, leaves the owner unchanged and matches
published layer pixels. Existing source/resource/cancellation/Undo gates pass.

Browser tests cover preview layer ordering, source-over samples, visible selection
feedback, exact publication/Undo, Escape/Undo cancellation, stale target/tool/
selection, retained failed commits and private growth. Actual host shortcuts,
consent controls, repeated pointer drags/releases/regrabs and Apply run across all
three Classic skins in English and Japanese. Existing new-layer Paste tests pass.
Direct document/image download tests verify that private placement blocks file
output while retaining the original document, history and pasted image.

An isolated HeadlessChrome 156.0.8078.4 release-build check used its independent
in-memory clipboard. Actual Cmd+V opened the dialog; a red/blue 32x16 image was
moved, placed with Enter, copied back through Edit > Copy and independently
read/decoded as exactly 256 red and 256 blue pixels. Undo restored the clean
source. A second Paste followed by Escape left it clean. Screenshots were
inspected and no unhandled exceptions occurred. The general OS clipboard was not
accessed. Logs and the CDP harness are local in ignored
`docs/validation/floating-paste-2026-10-24/`.
