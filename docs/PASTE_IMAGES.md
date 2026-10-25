# External-image Paste foundations

The editor now supports a frozen Paste draft and atomic insertion of its image as
a new topmost layer. The CLI consumes this path through `paste-image`. Desktop and
browser canvas Paste controls remain disabled: clipboard reads, interactive
placement, oversized-image choices and profile-specific paste behavior are still
work. This tranche does not complete clipboard support or the frontend roadmap.

## Command-line use

```sh
paintcraft-cli paste-image drawing.pcnd picture.png -o combined.pcnd
paintcraft-cli paste-image drawing.pcnd picture.png --x 20 --y 30 --expand-canvas -o combined.pcnd
```

`--name` names the new layer. The default placement is the canvas's top-left.
Without `--expand-canvas`, the canvas stays unchanged and the complete image,
including off-canvas samples, remains in the new layer. Growing expands right/down
to contain a nonnegative placement. Negative-origin growth and dimensions beyond
the document limit are rejected. Existing layers keep their extents and contents;
new canvas margins outside those layers are transparent.

Discovery reports `image.rgba8.paste-layer` as implemented and unqualified.

The source project and image are acquired as immutable byte snapshots. Optional
`--expect-input-sha256` and `--expect-image-sha256` guard those snapshots before
parsing; these are acquisition guards, not filesystem compare-and-swap guarantees.
Broader concurrent-writer qualification remains in the file-outcome roadmap. Image decoding uses the existing format/frontend admission and loss
policy. A depth/profile/metadata loss requires the same explicit `--allow-loss`
consent used by image import. Encoding is sniffed through the current codec path;
image input is capped at 64 MiB and decoded output at 16,777,216 pixels before
pixel decode. Actual RGBA vector capacity is checked against the clipboard image
budget. Output uses the existing safe native writer and overwrite rules. A direct
alias of the source image cannot be replaced by the output project.

`paste-image` reads an image file, not an OS clipboard. It does not claim a
clipboard read or interact with the user's clipboard.

## Draft and transaction ownership

`PasteDraft` owns a cheap clone of an admitted immutable RGBA image, the source
session's owner/state/epoch ticket, source canvas size, origin and canvas policy.
Repeated `place` calls modify only unpublished metadata. An invalid position
retains the last valid placement; dropping the draft cancels it. Source pixels are
not serialized into a huge batch JSON string or re-read from an external path.

`Session::paste_image` and `Preparation::paste_image` require that exact source
session and epoch. A changed selection or Undo followed by restoring equal pixels
still invalidates the draft. Detached preparation additionally retains the normal
resource-policy guard at owner publication. A successful save during preparation
keeps its independent saved-state acknowledgment through the existing publisher.

Insertion ignores the old selection when reading the incoming image. It adds one
visible, fully opaque layer containing exact straight RGBA8 samples, including
hidden RGB and partial alpha. The new selection is the image's canvas-clipped
rectangle, or Empty for entirely off-canvas placement. Other layers, document ID
and title remain unchanged. Optional growth and insertion publish together and
record one **Paste** entry. Undo/Redo restore complete snapshots, including the
previous selection, without rerunning decode or paste.

The inserted layer, new state and history identity use a versioned hash of image
dimensions and raw RGBA, placement, resulting canvas dimensions and layer name,
combined with the source state. The receipt exposes the new layer as `pasted`.
No new batch wire operation is advertised; this is a typed external-resource path.

## Raster admission and cancellation

The new checked RGBA constructor first counts non-default tiles, then reserves the
complete private decoded tile peak before allocating them. It copies rows directly
into admitted tile buffers and preserves transparent-black defaults sparsely.
There is no second complete image buffer or packed-tile scratch allocation.
Cancellation is checked while scanning, copying rows, hashing and recording
history, and again before publication. Errors expose no partial layer/document.

Immutable image bytes retain their separate capacity lease; tile snapshots retain
decoded leases until the final reader/history snapshot drops. Admission objects
may retain their explicit reservations until the owning attempt drops them. Tile
index/document metadata and codec internals retain their existing policies; these
payload admissions are not an allocator or physical process-memory bound.

The original codec `from_rgba` path is unchanged. The checked two-pass constructor
is a correctness baseline for external Paste; no speedup, native latency, RSS or
comparative benchmark qualification is claimed.

## Validation and remaining scope

Independent pixel oracles cover non-square images, tile edges, sparse defaults,
hidden RGB, partial alpha, negative and off-canvas placement, growth, untouched
layers and Undo/Redo. Tests inject every observed checked-ingestion and Paste
preparation cancellation, reject decoded capacity before tile allocation, and
verify late history denial releases the admitted tile payload. Exported snapshots
retain capacity through Undo/Redo and release it after the final reader drops.

Draft tests cover repeated/private placement, invalid-coordinate rollback,
foreign sessions, selection changes, Undo epochs, resource-policy changes,
cancelled publication and deterministic identity. Real CLI subprocesses reopen
saved PCND files, compare every pasted sample and original layers, verify input
preservation, reject hash mismatches/invalid growth/image-output aliases, and
require explicit consent for 16-bit reduction. Existing transaction conformance,
v2 grammar, detached preparation and history regressions remain checked.

Interactive/native/browser Paste, external flavor enumeration and signature
policy, clipboard ownership changes during read, richer profile/DPI conversion,
active-layer paste, full oversized-image choices, internal exact-copy matching,
cross-application/native reference behavior and performance qualification remain
unfinished. Local validation is in ignored
`docs/validation/paste-images-2026-10-24/`.
