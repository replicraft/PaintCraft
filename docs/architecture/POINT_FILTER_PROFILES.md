# Versioned point-filter profiles

The six supported point profiles are Invert, Brightness/Contrast, Black & White,
Threshold, Posterize and Solarize. The four additions are implemented through
Core/Effects, persistent document descriptors, Commands and CLI/MCP; Native/Web
Studio host controls are also qualified. Formats I/O, renderer/required Metal
and actual production Chrome Worker/export controls pass for these additions.
All four visible Studio UI flows also pass in actual Chrome; their exact
interaction and appearance scope is recorded below, separately from raw Worker checks. See [integration and qualification](#integration-and-qualification).

Black & White, Threshold, Posterize and Solarize extend the first two profiles.
Their reference intent comes from Palette commit
[`aefc84fe9233e66344147078de866860326d8cfa`](https://github.com/spiffyfitz/palette/tree/aefc84fe9233e66344147078de866860326d8cfa),
specifically its [point kernels](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs)
and [registry/defaults](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs).
The retained source manifest hashes were checked for the independent
specification. Its
`Filter` enum has 132 variants; that source inventory is not 132 PaintCraft
implementations or an executed upstream qualification. See
[Palette integration](PALETTE_INTEGRATION.md) for the broader boundaries.

These are independent exact arithmetic contracts. Palette's floating point
implementation motivates their intent; bit-for-bit equivalence to that
implementation or another application is not claimed. No upstream source was
copied into the oracle or adopted wholesale as the product implementation. Layer
styles and the rest of that fixed 132-entry inventory remain open; six exact
profiles do not establish broad catalog parity.

## Shared meaning

Input is straight RGBA8. All arithmetic below uses **encoded RGB byte values**,
without an ICC transform, linear-light conversion or premultiplication. Retained
source profiles remain provenance. Alpha is unchanged, including alpha zero;
RGB hidden under transparency is evaluated normally.

Let `round(n/d)` mean exact nearest integer with ties upward, for nonnegative
integer `n` and positive integer `d`:

`round(n/d) = floor((2*n + d)/(2*d))`.

There is no intermediate byte cast, floating point rounding or implicit channel
clamp unless a profile explicitly specifies one. The additional-profile parameter
domains
keep every result within `[0, 255]`. Typed construction validates the domains;
scalar evaluation uses sufficiently wide integer arithmetic. The original
Brightness/Contrast profile retains its explicit clamp and exact rational rule.

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
These accepted semantic IDs are versioned contracts. Required wire fields remain
required even when a UI draft has a default.

| Kind | Studio label | Parameter | Inclusive range | Default |
|---|---|---|---|---|
| `invert-rgb-v1` | Invert | none | none | none |
| `brightness-contrast-v1` | Brightness/Contrast | `brightness`, `contrast` | −100…100; −99…99 | 0; 0 |
| `black-white-rgb-v1` | Black & White | `strength`, shown as Strength (%) | 0–100 | 100 |
| `threshold-luma-v1` | Threshold | `level`, shown as Threshold | 0–255 | 128 |
| `posterize-rgb-v1` | Posterize | `levels`, shown as Levels | 2–32 | 6 |
| `solarize-rgb-v1` | Solarize | none | fixed threshold 128 | none |

Defaults describe a newly created descriptor/UI draft; they do not authorize
omitting required wire fields.

Black & White, Threshold and Posterize are Palette's Adjustments entries;
Solarize is its Stylize entry. Black & White is a single weighted desaturation control, not Photoshop's
six-color-channel mixer. Posterize intentionally follows the pinned 2–32 range;
256 levels is not an identity option in this profile.

### Original profiles

`invert-rgb-v1` maps each channel `c` to `255-c`. It has no whole-domain
identity setting. `brightness-contrast-v1` uses signed integer hundredths
brightness `b` and contrast `c`, with gain `(100+c)/(100-c)`, midpoint `127.5`
and offset `255*b/100`. Clamp the exact rational result to 0…255, then round
nearest with ties upward. Contrast 100 is refused. Brightness and contrast both
zero are byte identity. Both profiles preserve alpha and hidden RGB and retain
their original parameter hashing and PCND byte representation. See the
[original arithmetic contract](PALETTE_INTEGRATION.md#persistent-model-and-arithmetic).

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

`out.rgb = [255, 255, 255] if L >= 10000*t, otherwise [0,0,0]`.

Equality is white. Level zero makes every RGB input white; level 255 makes only
`[255, 255, 255]` white. There is no whole-domain identity setting. Already black
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
triples. Frozen hashes are expected-reference evidence, not proof that a
product matches them. Qualified scalar tests now compare the implementation to
these frozen streams and literal controls; separate integration checks below
exercise actual command consumers. The Python `Fraction` cross-check evaluates
separate
rational expressions for literal controls, unary quantization and selected
cross-channel/boundary samples; no binary float is used for expected outputs.

## Integration and qualification

All six profiles use the existing ordered nondestructive stack, atomic editor
preparation, persistent masks, history, source-preserving PCND persistence and
captured exported-copy consent. Descriptor updates preserve existing saved masks.
They do not rewrite source pixels. Changed parameters participate in stack/cache
and consent identity; hidden and disabled stored intentions remain persistent.

The input grammar selects the lowest version needed by the operations:

- New-kind `filter.add`/`filter.update` require closed `paintcraft.batch/v4`.
- Original-kind Add/Update and scalar enable/reorder/remove/duplicate require v2.
- Mask operations require v3 unless another operation in the batch requires v4.

Frozen batch v2/v3 reject new-kind Add/Update. Inspection uses CLI
`paintcraft.inspect/v3` or distinct MCP `paintcraft.mcp-inspect/v3` iff any
stored instance has a new kind, including disabled instances. Otherwise original
masked inspect/v2 and old unmasked shapes remain unchanged. Results and
file-command envelopes select v1/v2/v3 from final masks and published aliases
when the document has no spatial intentions; these six point kinds themselves
introduce no new result feature. Stored Box now selects result/envelope v4,
separate inspect/v4 and PCND5, as documented in the spatial contract.
The point-filter, mask and optional source-metadata feature order is unchanged.
See [automation contracts](../FILTER_COMMANDS.md#cli-and-result-contracts).

PCND's algorithm registry explicitly adds tags 3–6 within existing PCND3/4:
Black & White strength byte 0…100; Threshold level byte 0…255; Posterize levels
byte 2…32; Solarize with no parameter bytes. Original tags 1/2 and existing
specimens are unchanged. Unknown or unavailable algorithm versions must return a
typed refusal even when disabled. For point-only documents, a mask selects PCND4, an unmasked nonempty stack
PCND3, and an empty stack exact PCND1/2. Stored Box takes precedence with PCND5. The unchanged 8 MiB
metadata cap includes intentions and masks; adding a kind does not bypass it.

Qualified checks at this checkpoint include Core/Effects/Document numeric and
persistence-meaning controls; 21 Commands tests; five actual CLI/stdio-MCP groups;
61 independent schema controls over 32 genuine receipts; 43 legacy CLI/filter/
mask/persistence regressions; Native/Web/Studio host controls and strict/Wasm
checks. Actual consumers compare literal and fractional-mask pixels against the
independent oracle, including implicit defaults, ordering, disabled inspection,
PCND save/reopen, raw-source preservation and unchanged result profiles.
The locally retained consumer receipt is
`docs/validation/point-profile-consumers/final-freeze.json`; it records original
harness failures separately from corrected passes. UI host Worker readiness is
simulated in those host tests; it is not an actual Worker/browser proof.
Independent JS oracle controls (63) and the full Node suite (185) also pass;
Node execution does not establish production-browser execution.

The full Formats suite passes 769 tests. All 108 renderer tests pass, including
36 required offscreen Metal controls. The actual Chrome 156.0.8078.79 production
probe passes 305 persistent render Worker cases and 32 DDS/MSP protocol 4 export
cases. Nine positive export artifacts (eight DDS and one exact bilevel MSP) are
independently decoded with Pillow 10.3.0 and match the fixture planes. The other
MSP profiles retain their defined bilevel refusal; acknowledgment does not widen
that capability. The 27 production assets are frozen in
`docs/validation/point-filter-production/final-freeze.json`; raw browser evidence
is retained under `browser/attempt-xsCYSS/production.json` in that directory.

Four final actual Chrome Studio flows pass for Black & White, Threshold,
Posterize and Solarize, with seven evidence records per flow. Each verifies Add,
Cancel with byte-identical project/source, Apply, exact Undo/Redo states and fresh
reopen/save. The three numeric profiles also verify changed parameters; Solarize
has no numeric parameter. Independent red/green fixtures make every default
preview causally different from the source, with exact two-color screenshot
markers at the same canvas geometry. Changed Threshold 200 produces uniform
black: its exact bound Worker plane is verified without a two-color screenshot
causality claim. Apply may legitimately promote the already admitted private
texture; fresh reopen separately checks a strict committed BASE response bound
to the saved source SHA/state, null operation/target and exact full ROI.

Final UI receipts under `docs/validation/point-filter-production/ui` are
`attempt-EXAZqE`, `attempt-bjAaVV`, `attempt-SzoTzH` and `attempt-lpUwEY`.
Calibration and earlier failed attempts remain retained separately. These four
flows do not newly qualify existing-instance Edit, duplicate or preview-off;
older two-kind evidence for those interactions keeps its original scope.
No strict frame-latency, new GPU shader, external-application parity or complete
Palette coverage claim follows from these checks. Layer styles and the remaining
inventory are separate milestones.

Brightness alone can reuse the existing BC descriptor with contrast zero as an
explicit preset; it need not duplicate a kernel or inflate the algorithm count.
Exposure, Levels gamma, HSL/Vibrance and seeded noise remain later proposals:
they need explicit deterministic transcendental or coordinate contracts before
implementation.

## Spatial integration boundary

The seventh editable filter, [Box Blur](SPATIAL_FILTER_INTEGRATION.md), is a
separate spatial, alpha-changing contract. It does not alter these six halo-zero
arithmetic profiles or their old registry bytes. Box Add/Update requires batch v5;
point Add/Update retains v2/v4, masks v3, scalars v2. A mixed document stores Box
under PCND5 and uses inspect/result spatial profiles even if that Box is disabled
or radius 0. Point-only documents keep their minimal older profiles.

Box local Core/Effects/Document, Renderer/required Metal, PCND, CLI/stdio and
Native/Web host/Worker/strict checks pass. Actual production browser Box runtime
and UI remain pending; the six-point actual browser evidence above does not
qualify the new spatial profile. The broader 132-entry catalog/styles/layer-mask
milestones remain open.
