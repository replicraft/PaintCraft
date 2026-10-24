# Studio canvas size

Studio's Image menu has its own Canvas size dialog. It stages pixel width,
height and one of nine placement anchors. Center is initially selected. Applying
changes the canvas without scaling the artwork. Cancel and unchanged dimensions
publish nothing. Classic keeps its existing top-left resize/default-size workflow.

## Placement and preservation

The shared `canvas.resize` contract translates every layer origin by
`floor((new - old) * anchor)`, with anchor components 0, 1/2 or 1. Signed floor is
intentional: shrinking a width by three pixels moves centered artwork left by two.
The active selection moves by the same offset and clips to the new canvas. A
fully clipped selection stays active and empty. Undo/Redo restore the complete
recorded state, including selection.

Layer IDs, order, metadata, local pixel values, sparse tiles and local bounds are
preserved. Off-canvas content remains available. Uncovered canvas area is
transparent. This operation does not pad or scale layer rasters; automatic layer
expansion for painting into newly uncovered margins is still separate work.
Destructive Crop is a different operation.

The reusable anchor descriptor lives in `paintcraft_core::types::canvas::Anchor`;
`paintcraft_commands::Anchor` reexports it for existing callers. Command parsing
and the v1/v2 wire identifiers are unchanged. Tool intentions and native/browser
adapters use the same shared command builder. The browser retains its pixel/size
limits and rejects invalid/oversized requests before resolving a text draft.
CLI batches already support the nine anchors through `canvas.resize`.

## Ownership and presentation

The native host resolves pending input and waits for earlier accepted edits before
capturing the dialog's document revision and starting dimensions. Cancelling the
wait leaves those earlier edits intact. Once the dialog is ready, a changed tab,
changed publication or unexpected queued edit retires the staged dimensions.
Apply routes through the normal ordered editing worker. Studio canvas edits do not
change Classic's configured/remembered startup dimensions.

The Studio crate owns the dialog, independent of Classic widgets. Dimension
fields align, the nine anchor buttons expose selected state and accessible names,
and a scrolling body keeps Apply/Cancel fixed on compact displays. Captions,
tooltips and the waiting state have English/Japanese Fluent messages. The native
macOS menu has a separate Image section for these actions, with unique IDs and
ordinary enable/ownership checks; no action is duplicated in Edit.

## Validation and remaining scope

Tests cover all nine anchors with growth and negative odd-size shrink offsets,
raw/hidden RGB preservation, unchanged tile identities, zero decoded-pixel
admission, selection movement, no-op/error behavior and Undo/Redo. Native worker
tests hold an actual preparation worker to prove the wait/capture boundary,
Cancel behavior and stale owner/revision rejection. Browser adapter tests cover
anchors, selection restoration, size limits and text draft preservation.

Real Studio GUI tests exercise every anchor, one publication per Apply, aligned
fields, unchanged Classic preferences, exact Undo/Redo, Cancel/no-op behavior and
fixed actions on compact English/light 1x and Japanese/dark 2x layouts. The explicit
offscreen review test is `export_studio_canvas_size_for_review`. Local evidence
is under `docs/validation/studio-canvas-2026-10-24/` (ignored).

This implements the core placement contract. Native Paint.NET parity, image
resampling/skew, new-margin painting, physical units/DPI and related image
properties are not completed by this change.
