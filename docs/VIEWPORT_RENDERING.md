# Exact regional viewport presentation

Desktop display requests now cover the visible document region rather than always
allocating/composing a full-canvas output. Classic and Studio still own their
separate canvas layout and view state. Both route the clipped viewport through
`paintcraft_ui::view::View::visible_region`: inverse-transform the screen rectangle,
floor its leading edges, ceil its trailing edges, and intersect the finite canvas.
Partially visible edge pixels are retained at fractional zoom and DPI. Empty or
nonfinite rectangles produce no display request. Rendering starts after layout
knows the viewport; there is no initial pre-layout full-canvas request.

## Frame identity and positioning

The native presentation key includes state identity, publication epoch, preview
revision, exact document-space region and sampling level. An older preview may make progress only
within the same live gesture, region and sampling level. Changing the viewport cancels
obsolete work through the existing admitted worker. A completion must still match
its job handle, current document/preview/region and device epoch before installation.
A GPU texture additionally carries its own region; equal dimensions at a different
origin are not interchangeable. Invalid bounds reject native registration and
fall back to CPU rendering of the requested region.

A displayed frame remains positioned at its own document bounds during pan/zoom;
it is never stretched over the whole canvas. Pending work may therefore leave a
checkerboard portion temporarily visible. Current overlays, cursors, rulers and
gridlines retain their existing document/view mapping. Final saves, exports and
clipboard consumers use authoritative documents and their complete requested
output, independently of display textures.

`App::rendered_region()` reports installed display bounds. `App::rendered()` is a
tightly packed straight RGBA8 buffer on CPU-backed displays and is empty for
direct native GPU display. `rendered_size()` supplies its actual dimensions and
`rendered_sample_step()` the source-pixel span of each texel. Source bounds alone
are insufficient to index a coarse buffer. A failed display has no installed region.

## Backend and resource boundaries

The CPU path uses the existing exact regional compositor/cache. Cache dependency
identities and preview/committed partitions are unchanged. GPU composition clips
ordered layers against the region, preserves world-space source-cache identities,
and rebases only output indices. It uses a requested-region straight composite
buffer and completed premultiplied display texture. Explicit
[private output retention](GPU_OUTPUT_POOL.md) can reuse compositor capacity;
fresh allocation remains the measured default. Full GPU APIs delegate to the
same path with the full canvas. Native worker output declaration uses requested
output dimensions, including coarse sampling; existing source/preview/cache allowances remain. No physical GPU/RSS budget
or whole-preview staging bound follows from this declaration.

CPU upload validates the backend's maximum texture side before constructing an
egui texture. GPU output validates device limits. Requests exceeding these limits
fail truthfully and can recover after a smaller request. The
[bounded CPU multiresolution path](MULTIRES_DISPLAY.md) now filters zoomed-out
views before upload; it does not materialize the whole native-resolution source.
GPU multiresolution, tiled display textures, display-texture pooling and asynchronous
GPU transfers remain open. Exact-pixel hardware limits retain truthful failure. Preview
commands still stage the complete operation, including offscreen edits, so this is
regional composition/presentation rather than full incremental edit execution.

## Verification scope

Exact GPU tests compare regional straight output to the scalar CPU oracle with
negative/positive layer offsets, order/opacity, tile/workgroup boundaries, warm
source reuse, odd readback padding, single-pixel output and subsequent full export.
A small viewport of a 16384-square canvas succeeds even when full GPU output is
rejected. Invalid/empty/extreme bounds and cancellation retire private source state.

Desktop tests cover stale regional preview rejection, native texture origin
validation, CPU fallback, backend texture limits and later recovery. Actual egui
GPU draws compare full and cropped native textures to managed CPU images over a
background. GUI tests exercise a 16384-square document at multiple DPI/zoom settings
and forced layout passes without full-canvas display buffers or document changes.
These checks do not qualify Windows historical behavior, OS presentation latency,
all-service memory budgets or sustained multi-window interference.
