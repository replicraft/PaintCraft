# Box blur kernel and integration boundary

This is the numerical and allocation-free kernel foundation for a future
editable spatial filter. It does **not** add a seventh Studio filter, a PCND
algorithm tag, a command descriptor or a browser preview mode. The shipped
catalog remains the [six point profiles](POINT_FILTER_PROFILES.md).

## Numerical policy

`BoxBlurV1` accepts integer radius 0 through 32, with default 2. Its fixed square
window has `(2r + 1)^2` taps. Clamp each tap to the **true finite source edge**,
including repeated edge samples. A tile, output rectangle or stripe is not an
image boundary. Output has the same finite extent as the source.

Radius zero copies straight RGBA bytes exactly, including hidden RGB. At positive
radius, let `Sa` be the sum of source alpha and `Sc` the sum of source channel
times source alpha over all taps. Compute:

- Alpha: nearest integer to `Sa / number_of_taps`.
- Each RGB channel: nearest integer to `Sc / Sa` when `Sa > 0`, otherwise zero.
- Nonnegative half ties round upward. Do not round between horizontal and
  vertical sums. A positive `Sa` can round to byte alpha zero while retaining
  nonzero hidden RGB; do not discard that RGB.

These are encoded RGB bytes, with no linear-light or ICC conversion. This
independently specified policy intentionally differs from Palette's per-axis
rounded [box passes](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/neighborhood.rs). The pinned upstream source review is evidence of design
intent, not byte parity or execution of Palette.

## Coverage after the candidate

First round the entire unmasked candidate. Then mix it **once** with this
stage's incoming pixel. Coverage zero returns incoming RGBA exactly; coverage
255 returns candidate RGBA exactly. At intermediate coverage `m`, incoming alpha
`Ap` and candidate alpha `Aq`, let `W = (255-m)Ap + mAq`.

- Alpha is nearest integer to `W / 255`.
- RGB is nearest integer to `((255-m)Cp*Ap + mCq*Aq) / W` when `W > 0`.
- When `W = 0`, interpolate hidden RGB directly as
  `((255-m)Cp + mCq) / 255`, rounded once.

The alpha-preserving point coverage function cannot define this operation.
Layer opacity and source-over composition follow all filter stages.

## Checked regions and caller-owned work

`BoxBlurPlan::new` validates nonempty finite source and output rectangles,
requires the output to lie inside the source and computes its clipped halo.
Input is exactly that tightly packed halo. Input/output lengths must match the
quote; scratch must cover the quoted prefix. Validation precedes callbacks and
writes. Checked signed coordinates support negative origins without wrapping.

The two sliding passes store unrounded horizontal sums in caller-owned `u64`
scratch. The kernel allocates nothing. Its quote includes simultaneous input,
output and used scratch bytes, plus a deterministic source/aggregate visit
count. The caller must separately charge actual allocation capacities, stored
source pixels, masks, prefix stages and concurrent jobs. A quote alone does not
admit a job. Partitioning the output bounds scratch without changing edge rules.

`required_input` and `box_affected_output` are separate read-dependency and dirty
propagation operations. Both conservatively expand by the radius and clip to
the finite source for this symmetric kernel. Cancellation checks occur during
both passes; failed private output and scratch must be discarded. There is no
millisecond cancellation guarantee.

## Remaining integration gates

Before exposing an editable spatial descriptor, evaluate ordered prefix regions
with cumulative halos, bounded ping-pong buffers and admitted per-worker peak
storage. Route CPU composition, GPU uploads, display reduction, thumbnails,
sampling and export through that evaluator. Local transparent/uniform shortcuts
and cache invalidation must account for upstream halo pixels and masks.

Extend document identities, history, persistence, typed automation, private
previews and actual UI workflows together. Current PCND3/4 unknown-algorithm
fixtures use tag 7. Move those active controls to a reserved sentinel only if
that same container/profile registry later accepts tag 7, retaining historical
evidence. The preferred [spatial integration plan](SPATIAL_FILTER_INTEGRATION.md)
uses conditional PCND5 with distinct spatial grammar support. A PCND5-scoped
tag 7 would not invalidate unknown-tag-7 refusals in unchanged PCND3/4: gate
algorithms by their container/profile. No numeric Box tag or PCND5 wire layout
is frozen by this foundation. Do not reinterpret a legacy negative as supported
solely because a different container version knows that numeric value.

The independent Python/Fraction oracle under
`crates/services/paintcraft-effects/tests/box-fixtures` specifies numerical
expectations. Product comparisons, resource/cancellation tests and eventual
renderer/host checks are distinct qualification gates. Neither a generated
oracle nor a standalone kernel establishes an editable filter implementation.

## Local foundation qualification

Core passes all 62 unit tests. Effects passes all 23 tests, including nine new
geometry/storage/cancellation groups and two groups comparing actual kernel
outputs with the independent Fraction oracle. Those comparisons consume 15
literal planes, 24 partition outputs, 45 mask literals, 12 rounded blur/mask
planes and 54,726 records across three frozen hash streams. Strict Core/Effects
checks and Wasm compilation in the real web dependency context pass. All owned
source files remain below 800 lines; no new dependency is required.

The retained evidence is `docs/validation/box-blur-foundation/`. Failed test-helper
coordinate arithmetic, a test iterator lint and an isolated Wasm invocation
without the web app's UUID randomness feature are retained alongside corrected
checks. None established a kernel numerical failure. This checkpoint has no
actual blur UI, GPU blur, performance or external-application parity claim.
