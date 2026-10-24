# Shape preview scheduling

Dragging an endpoint must allow an already running frame to finish. Previously,
shape changes advanced the minimum acceptable preview revision on every pointer
move. This cancelled or discarded the earlier render even when the document,
gesture, target layer, style, viewport and scale were unchanged. Continuous input
could therefore prevent any frame from being presented until the pointer stopped.

Shapes now retain in-flight progress within the same gesture. Once a frame
finishes, the renderer submits the newest endpoint. Each frame replaces the whole
viewport, so earlier outlines do not accumulate. Changing the origin, tool, layer
or style invalidates earlier frames. Document publication, cancellation, viewport
changes and scale changes retain their existing guards. Polygon and curve
construction can also retain progress while their geometry changes.

The regression test holds a worker behind a channel, resizes in four directions,
and checks that every shape retains the same pending job. It then releases the
worker and compares the final pixels with exact transaction execution. This failed
on the original scheduler and passes after the change for Line, Rectangle,
Ellipse and every preset. The rendering suite also exercises cancellation,
regional rendering, scaled display and device changes.

Raw before/after test output is stored locally under
`docs/validation/shape-scheduling-2026-10-24/` (ignored). This demonstrates a
scheduling fix, not a measured reduction in rasterization cost or a browser
performance claim. The browser currently renders previews synchronously and
needs separate performance qualification.
