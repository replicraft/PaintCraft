# Canvas dimensions

## Engine operations

`canvas.resize` retains its existing canvas-only semantics: resize the document
rectangle, translate layers according to the anchor, clip selection and preserve
off-canvas content. It does not silently change raster-layer bounds.

The additive batch-v2 operation `layer.expand` extends a raster layer's finite
bounds. It requires `layer_ref`, `width` and `height`; dimensions cannot decrease
or exceed the existing 1,048,576-pixel side limit. Existing pixels, hidden RGB,
layer identity, origin and implicit default are retained. Added pixels use that
default. Resident tile buffers remain shared, including partial edge tiles whose
padding was initialized to the default. Subsequent writes use the existing COW
admission path. Invalid expansion rolls back the entire atomic batch.

Classic's top-left canvas-size workflow combines `canvas.resize` with explicit
`layer.expand` operations where layers need larger right/bottom bounds. A white
Background therefore extends with white; transparent layers extend with their
own transparent default. Shrinking preserves layer extents and off-canvas pixels.
Undo/redo restores the complete canvas and layer bounds in one history entry.
The transaction keeps the layer origins and current editing target. Existing
aggregate operation and geometry limits still apply.

This is a documented PaintCraft policy. Reference-build qualification, other
anchors in the Classic dialog and interactive resize handles remain separate work.

## Classic preferences

Settings v3 stores the configured default, the last canvas size and the choice
between them. The initial default is 1000 × 486 pixels, with remembering enabled.
Until a last size exists, remembering falls back to the configured size. Settings
v1/v2 retain their interface choices and receive these defaults on migration.
Damaged or unfamiliar settings retain the existing move-aside preservation path.

Startup and New use the selected size for a fresh white Classic document. Opening
an existing file retains its dimensions. Successful New/Canvas size actions record
their size; failed and cancelled resizes do not. Undo/redo changes the document;
it does not undo user preferences or the recorded last size. Studio's default
remains 320 × 320. Preferences never flatten or otherwise edit an open document.
