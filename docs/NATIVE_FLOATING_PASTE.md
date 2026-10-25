# Desktop ordinary Paste and private image placement

Desktop **Paste / Ctrl+V / Cmd+V** now targets the existing visible layer, separately
from **Paste as new layer**. The macOS encoded clipboard reader supplies native
images. A Windows PNG/TIFF/DIB reader is now cross-compiled but remains unqualified
for native execution. Linux/FreeBSD named Wayland sessions now have a bounded
PNG/TIFF pipe reader. X11 sessions now have bounded direct/INCR PNG/TIFF reads
with private-selection Xvfb evidence. Linux/FreeBSD runtime and cross-application
fidelity remain unqualified. See [transport ownership and limits](CLIPBOARD_READS.md).
Actions follow the actual compiled platform capability. This work advances T14/T15; it does not qualify
reference Paint behavior or complete those roadmap tasks.

## Workflow

An explicit action captures the document, tab, frontend and active target before
reading. The existing bounded asynchronous clipboard/decoder path freezes image
pixels and reports known import losses. Changing the target or document while it
reads discards the late result. Text composition, gestures and pending edits keep
ownership; invoking Paste never flushes them to make clipboard access possible.

The placement dialog shows image dimensions, initial X/Y, optional canvas growth
and any required loss consent. Ordinary Paste omits the new-layer name and copy.
Accepting the dialog starts a private image on the captured active layer. Dragging
and releasing, dragging again, arrow keys and Shift+arrows reposition it without
changing pixels, selection or history. The outline follows the clipped pasted
rectangle. Apply or Enter places it; Cancel, Escape or Undo discards it. Pointer
interruption stops the drag while retaining the private image.

Classic 7/10/11 and Studio use their own existing canvases and keep their separate
workspaces. While positioning, changing tools, target, history or files requires
finishing or cancelling the image. View controls remain available. Unpublished
placement counts as unsaved work and cannot become an ordinary save or recovery
snapshot. No source pixels are copied back into the clipboard by placement.

## Preview and publication

`ImagePlacement` owns pointer geometry and revision identity. The controller pins
an immutable image, source document, session/selection ticket, active target,
canvas-growth choice and optional RGB key. The key is captured from transparent
selection preferences; later color changes do not reinterpret the source image.

Detached `Preparation::preview_paste_image_onto` uses the same admitted raster
kernel as publication. A native render worker builds disposable layer pixels and
feeds them to the existing CPU/GPU compositor. It does not rasterize a clipboard
image while capturing a native UI frame. Preview retains the current canvas size;
requested growth becomes visible only with the accepted transaction.

Image revisions participate in frame ownership and caching. Idle frames reuse the
rendered image, movement retains useful admitted work, and only results from the
same live placement may arrive late. Cancellation, a new image, a source change
or a publication retires older frames. Preview and final output use actual target
composition, layer order, opacity and RGB-key handling rather than an overlay
approximation.

Apply submits the ordered existing-layer path described in [Paste onto a
layer](PASTE_ONTO.md). The preview remains visible during memory waits and cannot
be dragged or submitted again while its transaction is pending. Cancelling this
wait cancels the transaction. Failed detached preparation retains the image for
correction/retry. Both a successful publication and a successful unchanged result
finish placement; only changed pixels/selection/canvas create one Paste history
entry. Undo restores the prior recorded snapshot without reading the clipboard.

## Evidence and remaining scope

Controller tests cover repeated drags, nudging, invalid-position rollback, pointer
interruption, guarded actions, exact preview/placement pixels, growth, cancellation,
waiting publication, duplicate Apply, unchanged completion and correctable failure.
Host tests exercise actual Cmd/Ctrl+V, modal consent, raw pointer dragging,
arrow movement and Apply/Cancel in every Classic skin and Studio, in English and
Japanese. A late target switch rejects a pending clipboard read. Fake readers
exercise host behavior without touching the user's general clipboard.

Render tests compare synchronous and admitted-worker pixels against an independent
dense source-over/RGB-key oracle, check moving-image scheduling, and reject late
frames after cancellation. An explicit offscreen GPU test passes on Apple M1 Pro
Metal through the admitted native render path; no native window is opened.

Native Windows/Wayland clipboard execution, additional DIB variants and Linux/FreeBSD runtime,
pinned Paint/Paint.NET clipboard
alpha/placement/DPI behavior, cross-application flavor interoperability and physical
device/input latency remain unqualified. General pasteboard access is not claimed
from deterministic reader tests. Aggregate physical memory and source/history
metadata overhead remain broader resource-accounting work. No measured speedup
is claimed by this integration.

The original floating-placement slice passed 376 desktop library tests (nine opt-in captures/checks
ignored), 280 desktop GUI tests (64 captures ignored), and the separately executed
Metal preview test. Strict desktop all-target and Wasm Clippy, Rust formatting,
985 translated messages across seven locales, and the 800-line source limit pass.
Logs and source hashes are local in ignored
`docs/validation/native-floating-paste-2026-10-25/`.

The later [packed DIB follow-up](DIB_CLIPBOARD.md) records its own decoder,
loss-consent and updated regression evidence.
