# Curves numerical foundation

Status: the standalone parameter and exact compiler pass native and actual Wasm
qualification. Curves is not an editable PaintCraft filter yet. The nine-profile catalog, PCND readers, execution registry
and Studio controls do not acquire another capability from this document.

## Intent and numerical policy

Pinned Palette commit `aefc84fe9233e66344147078de866860326d8cfa` defines five
fixed input knots, one shared RGB curve and freely chosen output bytes. Its
[parameters](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L858)
and [point evaluator](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L319)
establish that feature intent. The author inventory calls it five-point Curves.
PaintCraft will independently implement the exact policy below, rather than
promise equality with upstream floating-point output or Photoshop.

The proposed numerical ID is `curves-rgb-v1`. Five output bytes correspond to
fixed inputs `[0,64,128,192,255]`; the identity default has those same outputs.
Equal, decreasing and nonmonotone outputs and unlocked endpoints are valid.
Canonical parameters are exactly the five output bytes in input-knot order.
Arbitrary input positions, individual channel curves and monotonic slope limiting
are outside this profile.

For each knot, take the exact secant between its previous and next knot,
substituting the endpoint itself where a neighbor does not exist. Within each
interval, use the unique cubic Hermite polynomial having the two endpoint values
and these derivatives. Input intervals are `[0,64)`, `[64,128)`, `[128,192)` and
`[192,255]`. The last width is 63; the penultimate centered derivative has
denominator 127. Clamp the exact value to `[0,255]`, then round once to the nearest
integer with exact halves upward. Do not round tangents or intermediate terms.

Apply the resulting 256-byte lookup table independently to straight encoded RGB,
including hidden RGB under alpha zero. Preserve alpha exactly. The identity
curve preserves every RGBA byte. Original ICC remains provenance rather than
an activated working profile. A stored point-filter mask mixes the already
rounded candidate once using the existing encoded-RGB coverage rule; it does not
interpolate curve parameters or change compilation identity.

## Checked arithmetic boundary

The selected provisional baseline is checked i64 arithmetic. For fixed input,
the specified Hermite basis terms and left-to-right sums are affine functions
of the five output values. Their extrema over the entire byte-output cube occur
at its 32 vertices. The independent bound experiment checks every input byte at
every vertex, including individual products, partial sums, clamp and rounding.

For width `d`, local input `n`, and the two secant denominators `dl` and `dr`,
use denominator `D=d³*dl*dr` and this multiplication/sum order:

```text
h00 = 2*n³ - 3*n²*d + d³
h10 = n³ - 2*n²*d + n*d²
h01 = -2*n³ + 3*n²*d
h11 = n³ - n²*d
N = h00*yL*dl*dr
  + h10*d*deltaL*dr
  + h01*yR*dl*dr
  + h11*d*deltaR*dl
candidate = (2*clamp(N,0,255*D) + D) / (2*D)
```

Widen bytes before signed subtraction. Every multiplication and addition remains
checked. The largest recorded intermediate is `2,194,728,288,256`, below `2^41`,
requiring 42 signed bits. The affine argument covers all output-byte tuples for
this exact evaluation order, not arbitrary reassociation or another algorithm.
This capacity proof does not select a measured fastest backend or establish a
compiler's work, scratch or cancellation contract.

## Independent oracle and adoption gates

The frozen independent test oracle solves each segment's four endpoint/derivative
constraints by stdlib Fraction Gaussian elimination and evaluates by Horner.
It does not call the product basis implementation. Its finite corpus
contains 64 full LUTs, 12 exact half ties, 432 RGBA/mask literals and 36,864
all-alpha records. It includes all 32 output vertices, eight structural profiles
and 24 seeded profiles. This is not exhaustive over `2^40` parameter tuples.

The reviewed test fixtures live under
`crates/services/paintcraft-effects/tests/curves-fixtures/`; planning provenance
remains under `docs/validation/curves-planning/`. The independent
oracle SHA256 is `a4641d09a7e5e037ef496cde6f54db82c99b13e2247fa79bffc2e2d8c5f45e40`.
Its numeric corpus and arithmetic-bound experiment remain separate proofs.

The compiler contract requires caller-owned private LUT storage, explicit
scratch/work quotes, geometry and quota preflight before callbacks, bounded
cancellation and final-only
publication. Source/output preservation, scratch reuse and actual native/Wasm equality
are tested at the finite checkpoint below. Standalone types or compiler tests do not establish editable support.

Only after those prerequisites should typed descriptors, history, an immutable
catalog bundle, PCND6 parameters, prepared rendering, controls and automation be
connected. Source5 and Export7 may resolve a new explicitly priced registration
without an outer protocol bump; general reader recognition alone grants no
execution capability. Existing closed profiles remain closed; old saved bytes
remain unchanged. Actual native/browser lifecycle and export qualification remain
required before increasing the public supported-filter count.

## Standalone qualification checkpoint

The Core suite passes 78 tests, including the three Curves parameter-identity
controls. The compiler passes seven focused debug groups and the full Effects
release suite passes 59 tests, retaining Levels/Exposure and spatial/point controls.
Native all-target strict checks pass. No source or arithmetic fix was required.

`paintcraft_effects::curves` exposes `Scratch`, `Limits`, `Quote`, `WorkUsed`,
`quote`, `build_lut` and lookup-only `evaluate_pixel`. Scratch is a private
256-byte candidate table. Its quoted storage excludes caller output and small
fixed scalar stack/control values; it is not a process-memory bound. Compilation
charges a fixed conservative model of 128 source-step units per entry, 32,768 per
table, before each entry's formula. These are modeled bounds, not measured cycles,
CPU instructions or actual primitive counts. Geometry and scratch/work refusal
precede callbacks. Initial, per-input and final checks total 258 on success;
only the final successful check permits the complete output copy. Failure leaves
caller output unchanged and scratch reusable.

Actual Chrome execution of the real Rust Wasm plus generated wasm-bindgen
0.2.129 glue passes all 64 complete LUT hashes, 12 half ties, 432 Rust RGBA/mask
controls and 36,864 all-alpha records. It includes 90 causal hidden-RGB controls,
geometry/quota refusal before callbacks, invalid raw inputs, cancellation,
readiness, guards and reuse. Memory remains two MiB during this finite run under
a nonshared 16-MiB maximum. Neither value is a process RSS or performance result.
No fake imports or host implementations replace the Rust compiler or mask mix.

The actual Wasm receipt is
`docs/validation/curves-foundation/wasm/final-freeze.json`, SHA256
`aea8220f6016bf359b67a37b1180400ef7cf0e6d2b421ff352473307d41c6f70`.
Native logs are under `docs/validation/curves-foundation/compiler/native-first/`.
All original processes are terminal. Editable descriptors, catalog/PCND registration,
prepared job ownership, UI and automation are still subsequent work. Exposure's
pushed CI result is a separate scope and does not qualify this local foundation.
