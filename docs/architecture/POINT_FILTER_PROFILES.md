# Proposed next point-filter profiles

Status: **specification and independent oracle only**. These four descriptors are
not implemented or exposed by this checkpoint. The existing Invert and
Brightness/Contrast kernels remain unchanged. This document does not establish
Palette catalog coverage, Photoshop parity, GPU filter execution or a completed
Studio feature.

The next finite slice is Black & White, Threshold, Posterize and Solarize. Their
reference intent comes from Palette commit
[`aefc84fe9233e66344147078de866860326d8cfa`](https://github.com/spiffyfitz/palette/tree/aefc84fe9233e66344147078de866860326d8cfa),
specifically its [point kernels](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs)
and [registry/defaults](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs).
The retained source manifest hashes were checked before this proposal. Its
`Filter` enum has 132 variants; that source inventory is not 132 PaintCraft
implementations or an executed upstream qualification. See
[Palette integration](PALETTE_INTEGRATION.md) for the broader boundaries.

These are independent exact arithmetic contracts. Palette's floating point
implementation motivates their intent; bit-for-bit equivalence to that
implementation or another application is not claimed. No upstream source was
copied into the oracle.

## Shared meaning

Input is straight RGBA8. All arithmetic below uses **encoded RGB byte values**,
without an ICC transform, linear-light conversion or premultiplication. Retained
source profiles remain provenance. Alpha is unchanged, including alpha zero;
RGB hidden under transparency is evaluated normally.

Let `round(n/d)` mean exact nearest integer with ties upward, for nonnegative
integer `n` and positive integer `d`:

`round(n/d) = floor((2*n + d)/(2*d))`.

There is no intermediate byte cast, floating point rounding or implicit channel
clamp unless a profile explicitly specifies one. The proposed parameter domains
keep every result within `[0,255]`. A future Rust implementation must validate
before evaluation and use checked, sufficiently wide integer arithmetic.

Each filter consumes the preceding stage's rounded RGBA result. It computes all
three candidate channels from that same input tuple before writing any channel.
This is essential for the cross-channel Black & White and Threshold operations.
The final composition policy remains `paintcraft.core/default-v1`.

Each profile is source-local and has **halo zero**: an output sample depends on
only the incoming sample at the same source coordinate and its own mask sample.
Evaluation is confined to the finite layer source extent. Implicit raster default
pixels participate, including sparse layers with no allocated tiles; evaluating
only allocated tiles is incorrect. These profiles do not generate new coverage.

For a stage input `p`, compute the full candidate `q=F(p)`, then apply that
instance's enabled A8 coverage `a` **once**:

`out[c] = round(((255-a)*p[c] + a*q[c])/255)`, for RGB channels.

`out.alpha = p.alpha`. A missing or disabled instance mask means full coverage
255; an enabled mask supplies zero outside its finite domain/support. A disabled
filter instance returns `p`. Coverage is not also multiplied into the descriptor
strength, applied to the source raster, or applied again after the whole stack.
See [persistent filter masks](FILTER_MASKS.md).

## Parameters and defaults

All parameters are integers; floats, NaN, booleans, missing required parameters,
unknown fields, unknown kind versions and out-of-domain values must be refused.
The wire IDs below are proposed semantic IDs, not currently accepted commands.

| Proposed kind | Studio label | Parameter | Inclusive range | Default |
|---|---|---|---|---|
| `black-white-rgb-v1` | Black & White | `strength`, shown as Strength (%) | 0–100 | 100 |
| `threshold-luma-v1` | Threshold | `level`, shown as Threshold | 0–255 | 128 |
| `posterize-rgb-v1` | Posterize | `levels`, shown as Levels | 2–32 | 6 |
| `solarize-rgb-v1` | Solarize | none | fixed threshold 128 | none |

Defaults describe a newly created descriptor/UI draft; they do not authorize
omitting required wire fields.

The first three are Palette's Adjustments entries; Solarize is its Stylize
entry. Black & White is a single weighted desaturation control, not Photoshop's
six-color-channel mixer. Posterize intentionally follows the pinned 2–32 range;
256 levels is not an identity option in this profile.

### Black & White

For input `(R,G,B)`, retain the exact scaled encoded luma:

`L = 2126*R + 7152*G + 722*B`.

For each input channel `c` and strength `s`:

`out[c] = round(((100-s)*10000*c + s*L)/1_000_000)`.

Round only once at the final channel. Do not first round `L/10000` to a gray
byte, and do not reuse an already updated channel when calculating `L`.

Strength zero is byte identity for every RGBA input. Every gray input is byte
identity for every strength. Strength 100 yields equal RGB values; encoded red,
green and blue primaries become 54, 182 and 18 respectively. At strength 50,
`[255,0,0]` becomes `[155,27,27]`. At strength 100, `[20,115,0]`
has exact gray 86.5 and therefore rounds upward to 87. With mask coverage 128,
that rounded candidate is mixed with the stage input to give `[54,101,44]`;
fusing the unrounded luma calculation with the mask would incorrectly give
`[53,101,43]`.

### Threshold

Compare **unrounded** scaled encoded luma to the integer threshold `t`:

`out.rgb = [255,255,255] if L >= 10000*t, otherwise [0,0,0]`.

Equality is white. Level zero makes every RGB input white; level 255 makes only
`[255,255,255]` white. There is no whole-domain identity setting. Already black
and white pixels remain unchanged for levels 1–255, but that is not a reason to
elide a stored descriptor.

For example, `[0,178,4]` has scaled luma `1_275_944`. Direct Threshold 128 is
black. Black & White 100 first rounds it to gray 128, so a subsequent Threshold
128 is white. The difference is intentional stage ordering, not numerical drift.

### Posterize

For `n` levels, let `k=n-1`. Independently for each RGB channel `c`:

1. `q = round(c*k/255)`.
2. `out[c] = round(255*q/k)`.

These are **two distinct rounding operations**; combining their rational
expressions changes the quantizer. Every level count preserves endpoints 0 and
255. With two levels, 127 maps to 0 and 128 maps to 255. With three levels, the
middle output is 128 because the final 127.5 tie rounds upward. No permitted
level count is identity across all 256 channel inputs.

### Solarize

For each channel byte `c`:

`out[c] = 255-c if c >= 128, otherwise c`.

There is no parameter or whole-domain identity configuration. Values below 128
are unchanged; 127 and 128 both map to 127, and 255 maps to zero. Applying this
full-coverage kernel twice is idempotent; fractional masked repetitions need
not be. It remains a stored editable intention. It is
not an alias for whole-range Invert or luma Threshold.

## Oracle and scope of its evidence

The [fixture directory](../../crates/services/paintcraft-effects/tests/point-fixtures/README.md)
contains an independent stdlib Python reference and `oracle.json`. The JSON
freezes literal results and compact SHA-256 streams, with input ordering and
record framing stated for regeneration. The generator imports no PaintCraft
code and does not invoke a renderer, CLI or another product.

The current numerical checkpoint covers:

- Every Solarize byte and every Posterize byte for every supported level count.
- Every gray byte at every Black & White strength, with alpha 0, 1, 128 and 255.
- Every gray byte against every Threshold level, with the same four alphas.
- Chromatic brackets immediately below/at/above the blue-channel threshold
  crossing for every threshold and a declared finite set of red/green anchors.
- Black & White cross-channel samples at every strength, manual primary/tie/order
  specimens, fractional mask samples and rejected parameter domains.

The chromatic set is deliberately disclosed: it is not all 16,777,216 RGB
triples. Frozen hashes are expected-reference evidence, not proof that a future
product matches them. The Python `Fraction` cross-check evaluates separate
rational expressions for literal controls, unary quantization and selected
cross-channel/boundary samples; no binary float is used for expected outputs.

## Required implementation gates

Implementation has **not started** in this checkpoint. It must reuse the ordered
nondestructive stack, atomic editor preparation, persistent masks, history,
source-preserving PCND persistence and captured exported-copy consent. A
destructive endpoint alone does not satisfy this slice.

Before adding descriptors, settle the next closed input and inspection profiles;
do not silently widen already frozen batch v2/v3 or mask inspection profiles.
Assign unambiguous persistence tags and retain typed refusal for unknown
algorithms, including disabled instances. A container version change is a
separate format decision, not implied by these proposed semantic IDs.

The current scalar evaluator updates channels in place for channel-independent
kernels. New cross-channel kernels must first capture the original RGB tuple.
Descriptor parameter hashing must become exhaustive: the current special case
for Brightness/Contrast must not omit new strength/level/levels values. Preserve
all existing Invert/BC hashes and byte results, and bind every new parameter to
stack/cache/policy identity, serialization and inspection.

Qualification must compare product output against these independent specimens,
then cover order, identity/no-op behavior, alpha and hidden RGB, mask 0/128/255,
sparse defaults, CPU/regional display/required GPU composition, current Worker
frames, atomic failure/cancel/stale publication, Undo/cold history, PCND
save/reopen, CLI/MCP and Native/Web UI. The existing metadata cap and resource
admission remain in force. No completed integration, performance gain, shader
execution or application-parity claim follows from this document.

Brightness alone can reuse the existing BC descriptor with contrast zero as an
explicit preset; it need not duplicate a kernel or inflate the algorithm count.
Exposure, Levels gamma, HSL/Vibrance and seeded noise remain later proposals:
they need explicit deterministic transcendental or coordinate contracts before
implementation.
