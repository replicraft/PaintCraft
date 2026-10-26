# Sampled brush qualification

PaintCraft has a bounded backend for adopting an admitted ABR preset into an
immutable native-size R8 tip. Raster coverage, captured tool gestures and the
existing atomic mask executor are exercised together by owned pixel fixtures.
CLI and MCP inspect/adopt/painting workflows use these same contracts; see
[Painting with MCP](MCP.md#imported-brush-resources).
Studio resource selection, UI preview and pointer routing are still pending;
this is not a claim that Studio currently installs or uses imported presets.

## Explicit native-grid policy

`StaticTipPolicy::NativeGridV1` preserves every eight-bit mask value and the
source dimensions. Pixel `(width / 2, height / 2)`, using integer division, is
centered on an integer gesture position. Signed original ABR sample bounds remain
in the retained source resource; they do not offset this explicitly chosen tip
anchor. There is no resizing, rotation, roundness change, flipping or precision
reduction. The adopted preset retains its original immutable `Arc<Library>`,
including original ordered descriptor/pattern/sample sections.

The first position produces a dab. Subsequent dabs follow source size times
source spacing along the captured polyline. Spacing and translations use Q8
positions. Each polyline segment has integer arc length
`isqrt((dx_q8² + dy_q8²) << 48)`, rounded down in Q32 pixel units.
Arc lengths accumulate as checked integers; dab interpolation rounds each signed
relative coordinate to nearest Q8, with exact ties away from the segment origin.
No floating-point square root, accumulation or translation determines dab placement.
Spacing rounding requires explicit `QuantizedSpacing` acknowledgement;
the source descriptor is never changed. Input vertices and the final endpoint
do not force additional dabs. Fractional translations use integer bilinear
weights, with rounded eight-bit results. Overlapping dabs use maximum coverage;
color opacity is applied once by `raster.mask_spans`. These are PaintCraft
semantics, not Photoshop or GIMP stroke-rendering parity.

Preview and release resolve the same captured tip, points, layer, color and
spacing. Commands use the existing mask schema and executor. The editor's
mask-only preparation wrapper supplies a Brush history label through the same
admission, execution and publication path; ordinary text masks retain their Text
label. For an empty document, a strict initial-layer preparation admits only a
transparent canvas-sized first layer followed by masks targeting its alias.
Creation and painting share one transaction; ineffective coverage discards the
staged layer. Empty coverage and zero color opacity create no effective publication
or history entry. Cancellation, rejected resource admission, stale preparations and
non-mask batches cannot partially publish a brush stroke.

## Admission and unsupported features

Only sampled presets with one unambiguous existing R8 sample, native maximum
extent as size, zero rotation, full roundness, no flips and enabled valid spacing
are admitted. Both the parsed convenience model and original descriptor fields
are inspected. Unknown fields/types, duplicate keys and unmodeled active flags
fail closed, including active texture flags whose convenience model is absent.

Computed or dynamic tips, high-depth masks, size overrides, rotation, roundness,
flips, disabled/invalid spacing, non-default outer spacing, shape/pressure
controls, scattering, textures, dual brushes, color dynamics, transfer, brush
pose, wet edges, noise, texture protection, interpretation and tool options are
reported as unsupported. The resource remains inspectable with exact original
samples and metadata. There is no silent stock-round substitution, high-byte
conversion or approximation of these features. Texture-pattern resources are
retained but do not become brush-engine textures.

Tip storage is shared, immutable and declared against the supplied existing
`SharedBudget`. Tip sides are capped at 16,384 and pixels at 4,194,304. Captured
points and dabs each allow 65,536, total attempted dab-pixel work 67,108,864,
coverage bounds 4,194,304 pixels, and output 262,144 disjoint spans. Spacing permits
1 through 16,777,216 Q8 units. Checked bounds precede large loops/allocation;
cooperative cancellation runs during copying, dabs, coverage rows and span work.
Coverage scratch and retained capacities are declared; tip clones share one
allocation/account. These are logical resource declarations, not allocator/RSS
interception, measured speed gains or a hard cancellation-latency guarantee.

## Polarity evidence and validation

The polarity is independently supported by current primary GIMP sources:

- [ABR v6 loader](https://raw.githubusercontent.com/GNOME/gimp/master/app/core/gimpbrush-load.c)
  reads raw or PackBits bytes directly into a `Y u8` brush mask, without inversion.
- [Brush core](https://raw.githubusercontent.com/GNOME/gimp/master/app/paint/gimpbrushcore.c)
  passes that brush mask to the paint core.
- [GEGL mask application](https://raw.githubusercontent.com/GNOME/gimp/master/app/gegl/gimp-gegl-loops.cc)
  multiplies destination alpha by mask coverage and opacity.

Consequently zero means no coverage and 255 means full coverage in this explicit
policy. Those source observations do not qualify execution against GIMP itself.
No third-party implementation or specimen is copied into the repository.

The standard-library-authored `native-presets.abr` includes asymmetric
`[0, 1, 127, 128, 254, 255]` pixels, negative source bounds, exact 16-bit data,
quantized spacing, active pressure/texture flags, transforms, unknown metadata,
a missing sample and an all-zero tip. The optional pinned JavaScript reader
checks metadata and mask values. It shares ancestry with the selected Rust
reader, so it is not an independent renderer oracle. Independent dense stamp
and explicit pixel expectations qualify coverage and actual editor publication.

Run focused checks from the repository root:

```sh
cargo test -p paintcraft-raster --lib sampled_brush:: --offline --locked
cargo test -p paintcraft-raster --test sampled_brush --offline --locked
cargo test -p paintcraft-tools --lib --offline --locked
cargo test -p paintcraft-editor --test sampled_brush --offline --locked
cargo test -p paintcraft-formats --lib abr:: --offline --locked
```
