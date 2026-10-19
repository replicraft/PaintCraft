# Pointer gesture ownership

`paintcraft-input` exposes normalized Primary/Secondary button identities and
explicit `ButtonDown`/`ButtonUp` events. Legacy `Down`/`Up` events and the existing
controller/tool methods remain primary-button shortcuts. An unqualified legacy
Up does not finish a secondary gesture. Move events retain viewport-mapped integer
document coordinates; no pen/touch qualification is implied by mouse tests.

`paintcraft-tools` owns the initiating button together with the layer ID, colors,
options and sampled points. Starting another button cannot replace that owner.
Only its matching release creates an action; cancellation drops ownership and
publishes nothing. Swatch changes after pointer-down do not change captured paint.
The tools crate now consumes the normalized input crate through a workspace
dependency; it retains no frontend or platform dependency.

The native editor controller validates the target, commits pending text where
appropriate, and passes the captured action through the existing transaction
outbox. The gesture color never requires temporarily swapping user preferences.
The emitted batch is the same typed color operation used by primary gestures and
the CLI; command schemas, raster algorithms and native document formats do not
change for button-aware input.

## Studio binding

Studio binds right-button canvas gestures to secondary color for Pencil, Brush,
Fill, Line, Rectangle and Ellipse. Shapes reverse primary/secondary as a pair, so
Both uses secondary outline and primary fill for a secondary gesture. Fill retains
its pointer-down seed. Selection, Text and Eraser secondary policies remain
unavailable; their distinct behavior must be implemented separately. Classic's
existing primary-button mapping is unchanged pending its compatibility work.

The canvas accepts pointer-down only inside its document and on its own layer.
Release coordinates from a non-owning button cannot extend or publish a gesture.
Switching documents, Escape, focus loss, tool changes and modal entry cancel
pending strokes. Modal entry retires ownership even when the modal consumes the
release, preventing a stroke from resuming after dismissal. Canvas input stays
blocked while the modal or native file dialog owns interaction.

Unit and executable GUI tests cover normalized events, captured preferences,
shape/fill colors, owner mismatch, cancellation, per-document routing, a modal
consuming mouse-up, exact staged-brush preview/publication pixels and undo/redo.
The ignored `secondary::render_studio_secondary_stroke_for_review` test writes
`/tmp/paintcraft-secondary-stroke.png` through the native GPU UI renderer. These
are native implementation tests, not an independent Paint.NET input oracle or
physical pen/tablet qualification. Reference-specific eraser, selection and text
policies, pressure/tilt and platform capture traces remain roadmap work.
