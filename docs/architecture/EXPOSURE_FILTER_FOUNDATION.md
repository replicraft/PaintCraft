# Exposure numerical foundation

Status: native and actual Wasm execution qualify the exact compiler. Typed
catalog commands, PCND6 persistence, hot/cold history, prepared CPU filtering and
Metal presentation,
Studio controls, native edit controls, CLI and stdio MCP are locally qualified.
The new nine-profile schema bundle is additive; the prior eight-profile bundle and
old browser preview/export capabilities remain closed. Fresh Source5 and Export7
production-browser qualification passes the finite workflows below. Exposure is
the ninth editable Studio profile. These checks do not establish Palette byte
parity or full catalog support.

## Intent and sources

The pinned Palette implementation applies Exposure in linear-light sRGB:
[document intent](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L824),
[EV control](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs#L499),
and [point evaluation](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L234).
The transfer constants are documented in [W3C CSS Color 4](https://www.w3.org/TR/2026/CRD-css-color-4-20261029/#color-conversion-code).
PaintCraft independently implements the exact mathematical policy below; it
does not promise equality with Palette's floating-point results or Photoshop.

The numerical profile is `exposure-srgb-v1`, with one required integer
`stops_hundredths` in -800 through 800, default zero. One stop doubles linear
light. Floats, missing fields, unknown keys, clamping and inferred defaults are
not future wire conveniences. Canonical semantic parameters are the signed
two's-complement value in two big-endian bytes, independent of transport.

## Exact channel policy

For an input byte `v`, set `x=v/255` and `s=stops_hundredths`:

```text
D(x) = x / 12.92                       if x <= 0.04045
       ((x + 0.055) / 1.055)^(12/5)   otherwise
l = clamp(D(x) * 2^(s/100), 0, 1)
E(l) = 12.92*l                        if l <= 0.0031308
       1.055*l^(5/12) - 0.055        otherwise
result = floor(255 * E(l) + 1/2)
```

Constants are exact decimals; only the final byte is rounded, with ties upward.
EV zero is explicitly exact RGBA identity. Black remains black. Channels are
independent, alpha remains byte-identical and hidden RGB is transformed.
Retained source ICC is provenance, not an activated working profile. Each saved
filter mask mixes the full rounded candidate once using the established encoded
point-coverage policy; ordered stages keep separate rounding. Halo is zero.

## Integer classification

For each output boundary `t=(2*j+1)/510`, `j=0..254`, compare
`D(v/255)*2^(s/100) >= D(t)`. The byte result is the number of crossed boundaries.
The half-byte boundaries nearest the sRGB knee are 9.5/255 and 10.5/255. Neither
falls in the tiny discontinuity between the conventional decode/encode knees;
the inverse-boundary comparison therefore gives the same rounded byte as the
piecewise policy above. Black is handled separately.

Raising positive sides to 100 removes rational exponents. A linear branch uses
a rational base to power 100; a nonlinear branch uses power 240. Shift `2^s` to
the left or right according to its sign. Reduced exact bases are:

| Sample | Linear branch | Nonlinear branch |
| --- | --- | --- |
| Input `v` | `5*v/16473`, `v<=10` | `(40*v+561)/10761` |
| Boundary `j` | `5*(2*j+1)/32946`, `j<=9` | `(40*j+581)/10761` |

Common branch denominators cancel; divide the exponent and signed shift by
their greatest common divisor where valid. Mixed branches retain crossed
denominator powers. Fixed-limb capacity and operation quotes must be proved
against the actual chosen implementation, including every temporary. No
floating-point approximation decides an exact half threshold.

## Standalone APIs and native checkpoint

`paintcraft_effects::exposure` supplies the explicit u32 reference backend;
`exposure::u64` supplies a separate u64 backend. Each has its own `Scratch`,
`Limits`, `Quote` and `WorkUsed`, with `quote` and `build_lut`. Neither selects a
backend automatically. `evaluate_pixel` performs lookup only and preserves alpha.

The shared private integer engine retains Levels' public APIs, quote arithmetic,
callback order and capacity diagnostics. Exposure owns its own budget and error
context. Storage is five fixed 10,240-bit integers plus the private LUT; there
are no heap arithmetic buffers, implicit caches, unsafe casts or dynamic backend
dispatch. The conservative mixed-branch bound is 5,760 bits. The quote permits
180 u32 or 91 u64 limbs, including rounded operands and signed exposure scaling.
The denominator 32,946 requires 16 bits, not 15.

There are at most three monotone branch regimes. Conservative counting permits
516 powers and 514 products/shifts/comparisons, including transition reloads.
The exact maximum quoted work is 25,259,104 u32 units or 10,299,760 u64 units,
both attained at EV +/-7.99. These are distinct abstract accounting units, not
CPU instruction counts, timing, RSS or a conversion between backend costs.

Both native backends match all 1,601 independent LUT hashes and the complete
409,856-byte aggregate, ten literal half ties, 420 literal RGBA/mask controls and
26,880 all-alpha cases. The full Effects release suite passes 51 tests, including
existing Levels and spatial/point regressions. Four debug boundary tests cover
geometry, preflight, early/mid/final cancellation, reuse, small private capacity,
independently recorded primitive charges and output preservation. All-target
Effects strict lint checks pass. Native scratch size is 6,696 bytes per backend;
this is caller-owned compiler storage, not a process memory bound.

The independent review and native logs live under
`docs/validation/exposure-foundation/compiler/`. Native test elapsed times are
diagnostic observations, not repeated benchmark or frontend performance evidence.
Backend measurements and actual frontend delivery remain separate gates.

## Actual Wasm and integration checkpoint

Actual Chrome execution of the real Rust Wasm modules covers both explicit
backends over all 1,601 LUTs, ten exact half ties, 420 literal RGBA/mask controls
and 26,880 all-alpha cases. Existing Levels controls pass in the same harness.
Cancellation, zero-callback preflight and invalid/readiness/batch checks also pass.
Each module retains its two-MiB nonshared memory during this finite run. Native
scratch size is not a Wasm size assertion. The immutable compiler evidence is in
`docs/validation/exposure-foundation/wasm/final-freeze.json`.

Native integration passes independent document identities, cold-history restoration,
PCND positive/negative fixtures and exact CPU plus actual Metal presentation checks.
The native editor control test exercises private preview, Cancel, Apply, Undo/Redo
and save/reopen in both synchronous and worker modes. Actual CLI and stdio MCP each
pass two groups; all 26 captured catalog outputs validate against the new nine-profile
bundle and are rejected by the frozen eight-profile bundle. Six separately retained
genuine Levels outputs validate against both bundles. See the finite closeout in
`docs/validation/exposure-catalog-cli/finite-closeout/final-ledger.json`.

These compiler, native-control and automation scopes are separate from the fresh
production-browser evidence below. Physical tablet behavior, repeated performance
measurements and the unimplemented Palette catalog remain outside this checkpoint.

## Production-browser checkpoint

A fresh main/Source5/Export7 production build passes all five build/asset checks.
All 3,435 producer inputs and 27 assets remain byte-identical throughout the proof.
Actual Chrome Source5 passes 15 cases and 97 checks. The visible Studio workflow
passes six lifecycle evidence groups: identity and causal Exposure previews,
Cancel with byte-identical source, Apply, exact Undo/Redo and fresh PCND6 reopen.
This does not newly qualify every existing-instance, duplicate or preview-off UI path.

Actual Main-produced DDS and MSP Export7 routes each pass 16 fresh Worker replays
(one positive and 15 negative). Real dialogs exercise consent, revocation without
download, recheck with a fresh nonce and the authorized download. Independent
Pillow decoding verifies the exact 2×1 planes: DDS `[90,26,176,255]` and MSP
`[0,0,0,255]`. PCND source bytes, title and dirty state remain unchanged.

The aggregate receipt is
`docs/validation/exposure-catalog-browser/browser-final-freeze.json`, SHA256
`55360d27089fdf6c03e629bcb32a5f20c137d4091c8750b0e903c49a39dda4d8`.
The initial loopback permission failure and all calibration attempts are preserved.
All original processes are terminal and browser/server resources are closed.
The full web package passes 404 tests with three existing ignored tests. The full
CLI/MCP package passes 228 tests with 15 existing ignored tests across 44 groups.
Native all-target and Wasm strict checks and all 267 Node tests pass.
New-head remote CI is a separate gate; the earlier green CI applies to pre-Exposure
commit `c94be65c`.

## Admission and acceptance

Compile one private 256-byte table for each unique complete admitted policy.
Retain its charge for the frozen job, evaluate pixels by lookup, and publish the
table only after the final cancellation check. Scratch, cumulative work and
fixed-width overflow must refuse before any public output changes. A shared
integer engine must preserve all existing Levels results, work/callback bounds
and API behavior. Backend choices need measured native/Wasm evidence.

The independent oracle covers all 256 bytes at all 1,601 settings: 409,856
channel records, per-table and aggregate hashes, linear/nonlinear knee cases,
clipping, exact half ties, all alpha values and mask coverage controls. Compiler
preflight, cancellation, final-check failure, reuse and source/output preservation
are separate gates. Native and Wasm execution must both match the oracle.

Implemented integration connects typed descriptor construction, semantic identity,
history, a new immutable resolved schema bundle, PCND6 profile/codec resolution,
prepared CPU/GPU entry points, native/Web controls, explicit copy consent and
CLI/MCP inspection/export. Older eight-profile schemas and Worker capabilities
remain closed; Source5 and Export7 resolve explicit priced implementations. New
registrations require their own numerical, resource and consumer qualification.
