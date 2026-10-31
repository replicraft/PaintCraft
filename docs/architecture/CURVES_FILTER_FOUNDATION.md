# Curves numerical foundation

Status: the parameter and exact compiler pass native and actual Wasm qualification.
Local integration now includes typed catalog intentions, PCND6, caller-owned
prepared rendering, five Studio controls, native history and CLI/MCP. Fresh
production-browser qualification passes the finite workflows below. Curves is
the tenth editable Studio profile. The standalone CI result does not qualify
these later integration changes.

## Intent and numerical policy

Pinned Palette commit `aefc84fe9233e66344147078de866860326d8cfa` defines five
fixed input knots, one shared RGB curve and freely chosen output bytes. Its
[parameters](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L858)
and [point evaluator](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L319)
establish that feature intent. The author inventory calls it five-point Curves.
PaintCraft independently implements the exact policy below. It does not promise
equality with upstream floating-point output or Photoshop.

The numerical ID is `curves-rgb-v1`. Five output bytes correspond to
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
All original standalone processes are terminal. The seven CI jobs for standalone
commit `f89243c9` pass in
[run 41193130351](https://github.com/replicraft/PaintCraft/actions/runs/41193130351).
That result covers the foundation and earlier Exposure work, not the integration
checkpoint below.

## Local integration checkpoint

Strict catalog JSON uses `{"kind":"curves-rgb-v1","outputs":[0,64,128,192,255]}`;
exactly five integer bytes are required. The immutable ten-profile schema bundle
has contract hash
`2729c23e843ca43c502a01e8f1e41d5d2ad9c0bfba610bb9b2659274d8e780e5`.
Older eight/nine-profile files remain byte-identical. All 26 genuine Curves
CLI/MCP outputs validate against the new bundle and fail both older bundles;
historical Exposure and Levels outputs still validate against their original and
new bundles. These are actual process outputs, separate from generator fixtures.

PCND6 stores the five output bytes using the profile-scoped `params-v1` codec.
Identity and disabled instances retain their full intent. Fourteen independent
positive/negative format specimens cover strict decoding and preservation.
No outer PCND or worker protocol version changes. Source2/3/4 and Export4/5/6
remain closed; Source5/Export7 resolve Curves through an explicit checked decoder,
prepared execution route and `prepared-curves-rgba8-v1` resource contract.

Prepared Curves tables belong to the admitted caller job and share its cumulative
work counter. Rendering only looks up already-prepared programs. Missing keys
refuse before a warm CPU/display/GPU cache can return stale appearance; a frozen
owner cannot extend itself. The combined Levels/Exposure/Curves limit is 256
unique programs, compiled in that canonical family order. All enabled stored
intentions compile, including hidden, opacity-zero and identity cases. Reader
capability checks still include disabled intentions.

All 46 native prepared-render tests pass. Two required GPU tests pass on
actual Apple M1 Pro Metal; they qualify CPU-filtered appearance with GPU
presentation, not a Curves shader. All 19 historical baseline records match
before/after on their own native and actual Chrome Wasm platforms, including both
historical export contexts. Only the unpriced native factory bookkeeping size
changes from 72 to 80 bytes; the Wasm factory stays 48. This is not a latency,
speedup or process-memory claim.

The native shared-control test covers each of the five sliders, synchronous and
asynchronous private preview, Cancel, Apply with one undo step, unchanged source
pixels, fresh native save/open, Undo and Redo. Eight Studio tests and two real CLI
plus two real stdio MCP tests pass. All five controls are localized in seven
languages. CLI/MCP image export requires explicit bake consent and leaves the
editable source unchanged. Main Web's selected filter/DDS/MSP tests pass 51 cases;
native and actual-target Wasm strict checks pass for Main and both workers.

Receipts remain under `docs/validation/curves-integration/`,
`curves-planning/owner-source/`, `curves-actual-tests/` and `curves-main/`.

## Actual production-browser checkpoint

The fresh Main, Source5 render worker, Export7 codec worker, Astro build and asset
checks all pass. All 3,458 captured producer inputs and 27 production assets remain
identical through the final browser closeout. Raw Source5 passes 17 cases and 123
checks, including identity, inverse, constants, free outputs, disabled intent,
legacy and Exposure compatibility, requirement unions, quota/tamper/stale refusal
and old closed profiles.

The actual Studio flow passes 14 checks using all five real controls. Each private
prefix frame matches the independent reference; the final constant-37 preview
also matches every screenshot pixel in the source-established canvas region.
Cancel preserves exact saved bytes/state/title/clean status. Apply preserves the
complete stack, manifest and raw source; Undo returns the original PCND3, Redo
restores PCND6, and a fresh page opens the saved PCND6 with a bound committed BASE.

Actual Main DDS and MSP Export7 routes each pass explicit consent, revocation,
fresh recheck nonce, download and 16 fresh worker replays. Independent Pillow
decodes the complete two-pixel planes to inverse RGB `[191,239,127]` for DDS and
opaque black for MSP. Original PCND bytes, title and dirty state stay unchanged.
No guessed positive packet, fake import or host replacement stands in for Rust
Wasm execution. One Studio parameter-layout calibration is retained separately; the first
semantic run of each of the four gates passes.

The aggregate receipt is
`docs/validation/curves-catalog-browser/browser-final-freeze.json`, SHA256
`1d7a068dfa58441345e508dbb0c174484e21de68696f84eee648f39514f182ee`.
All original jobs are terminal and owned browsers/servers are closed. These are
finite local results, not new-head CI, physical-device, performance, original-app
or all-132-filter qualification. A subsequent Curves-specific test passes real
cold-history compaction, spill, restoration and Undo/Redo, preserving all five
parameter bytes, mixed filter intent, raw pixels and unique mask/metadata payload
accounting. All 26 history tests and strict checks pass. The two test-only source
additions after the browser freeze are recorded separately in
`docs/validation/curves-main/full-regression/promotion.json`; no runtime code
changes. Full package regression then passes 405 Web tests (404 unit plus one
preview integration, three existing ignored) and 232 CLI/MCP tests (15 existing
ignored across 44 groups). Full history passes 26 tests without ignored tests;
history all-target strict and final workspace formatting checks also pass. The
serialized original regression process exits successfully without a product fix
or test rerun. All seven CI jobs pass for integrated Curves commit `fbfd4737` in
[run 41199286392](https://github.com/replicraft/PaintCraft/actions/runs/41199286392).
The final CI receipt and separate push addendum are retained under
`docs/validation/curves-integration/ci-41199286392/`; later standalone Vibrance
changes have their own qualification and CI scope.
