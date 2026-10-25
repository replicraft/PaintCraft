# Existing-layer image Paste

The shared editor now supports frozen external-image placement onto an existing
visible layer, separately from [new-layer insertion](PASTE_IMAGES.md). The CLI
consumes it through `paste-image --onto-layer`. [Browser ordinary Paste](FLOATING_PASTE.md) now consumes it with private floating
placement. [Desktop ordinary Paste](NATIVE_FLOATING_PASTE.md) now connects private placement,
worker previews and ordered publication through the native macOS reader. Windows PNG/TIFF reads are now cross-compiled; native Windows execution, DIB,
other native OS readers and reference qualification remain unfinished.

```sh
paintcraft-cli paste-image drawing.pcnd picture.png --onto-layer LAYER_UUID -o combined.pcnd
paintcraft-cli paste-image drawing.pcnd picture.png --onto-layer LAYER_UUID --blend replace --x -10 --y 20 -o combined.pcnd
paintcraft-cli paste-image drawing.pcnd picture.png --onto-layer LAYER_UUID --x 20 --y 30 --expand-canvas -o combined.pcnd
```

The default existing-layer blend is source-over; `--blend replace` preserves raw
source samples including hidden RGB under zero alpha. `--name` is exclusive to
new-layer insertion. Explicit blending requires `--onto-layer`; malformed,
missing or hidden targets fail without writing output. Existing image/profile,
loss-consent, hash guards and safe native output remain in effect. Discovery lists
`image.rgba8.paste-onto` as implemented and unqualified.

## Placement and publication

`Session::paste_image_onto` and `Preparation::paste_image_onto` take a frozen
`PasteDraft`, target ID, explicit blend mode and optional RGB transparent-color
key. A key skips matching incoming RGB regardless of alpha; it does not clear
old pixels. Source-over composites once against the target's raw pixels. Layer
opacity remains a display property and is not baked into the new samples.

The old selection does not clip incoming pixels. The target's finite extent
expands to the union of its old extent and the image. Negative positions rebase
its origin; unrelated layers and the root ordering stay intact. Pixels outside
the old finite extent are transparent, even if its implicit default is opaque or
contains hidden RGB. Off-canvas image pixels stay in the target. `PasteCanvas::Grow`
optionally expands the canvas right/down. Source dimensions, placement, resulting
extent and document coordinates remain checked against the existing policy.

The previous selection becomes the pasted rectangle clipped to the resulting
canvas. Selection, canvas and pixels publish through the existing atomic history
boundary with one **Paste** entry. Undo restores the full previous snapshot and
selection. The source session/epoch, current selection and resource-policy guards
apply to detached publication. Failed preparation, late history denial, cancelled
publication and discarded drafts never advance the owner.

The operation digest includes target, image dimensions and every raw sample,
position, resulting canvas, blend mode and key RGB. Key alpha is intentionally
excluded because it does not affect this policy. The receipt's `pasted` alias
identifies the existing target; no layer is created.

## Raster preparation

`RgbaPastePlan` captures the immutable image and COW source layer. It plans only
candidate tiles: the image footprint, rebased explicit source tiles, and finite
boundary/default corrections. Same-origin untouched tiles remain shared. A
complete sample pass records changed tiles and all-default removals before replay;
replay reserves the entire required decoded tile peak before allocating any tile.
An identical placement can retain every tile without decoded allocation.

Candidate-table capacity uses the existing per-plan/shared paint metadata
accounts. A provisional 67,108,864 sampled-pixel limit bounds preparation's
candidate regions, including tile-edge overcoverage. Replay repeats those
samples; it does not decode or reread the clipboard. Source cloning, hash-table
allocator overhead, aggregate service/physical-memory admission and wall-clock
cancellation tails remain outside this qualification. No measured speedup is
claimed.

## Validation

Raster tests compare every finite sample against an independent replacement
oracle over negative origins, tile boundaries, off-canvas placement, opaque and
transparent defaults, hidden RGB and RGB keys. They check source-over samples,
COW identity, partial-tile padding, frozen-source ownership, metadata/sample-limit
failure, complete tile admission and every observed replay cancellation point.

Editor tests cover target metadata, untouched layers, explicit policies,
selection/canvas/history, deterministic identity, detached preparation/publication,
foreign/stale sources, resource changes, quota failure and late history denial.
Every observed preparation cancellation preserves the owner and releases new
capacity. CLI subprocesses save and reopen PCND output, compare inserted samples
and verify source preservation, policy flags and invalid-target rejection.

Logs are local in ignored `docs/validation/paste-onto-2026-10-24/`. The current
macOS/browser new-layer clipboard workflows are unchanged. Native macOS ordinary Paste now connects private placement and worker-rendered
preview; other native read adapters and browser/reference/device/platform
qualification remain partial.


## Desktop ordered publication prerequisite

The desktop controller's `start_paste_onto` accepts the frozen image placement,
captured active-layer ID, explicit blend and optional RGB key. It rejects a changed
target, stale source, unresolved earlier edits or an active file barrier before
submission. The shared preparation worker retains the destination policy through
decoded-tile and metadata admission retries; retries never reread the clipboard.
The same destination dispatch preserves the existing new-layer paste route.

A successful active-layer placement uses the existing publication/receipt path,
keeps the target and layer count, records one Paste entry, and remembers authored
canvas growth. FIFO file capture waits for publication. Escape cancellation of a
waiting request releases ownership without a late commit. Impossible capacity
fails rather than waiting; a competing publication invalidates a waiting paste
rather than rebasing it.

Controller tests compare exact source-over/keyed samples, replacement alpha,
synchronous/worker results, canvas growth, Undo/Redo, immutable-owner identity,
waiting reservations and cancellation cleanup. [Native private placement](NATIVE_FLOATING_PASTE.md) now connects worker-rendered
preview, drag/nudge/Enter/Escape controls, ordinary Paste routing and guarded
file/tool boundaries. Availability follows the native clipboard reader; macOS is
connected and Windows PNG/TIFF transport is cross-compiled. Native Windows
execution, DIB and other OS read adapters remain unfinished.

This prerequisite passes all 367 desktop library tests (eight opt-in captures
ignored), including 12 paste tests, and strict desktop all-target Clippy.
Rust formatting, the 800-line source limit and seven-locale validation pass.
No window or general system clipboard was opened by these tests. Logs and source
hashes are local in ignored `docs/validation/native-paste-publication-2026-10-25/`.
