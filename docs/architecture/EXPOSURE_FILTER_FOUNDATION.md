# Exposure numerical foundation

Status: standalone parameter and compiler work is in progress. Exposure is not
an editable PaintCraft filter yet. Existing eight-profile catalog schemas,
PCND6 capabilities and Worker profiles remain closed. A parameter type or oracle
alone does not establish feature support.

## Intent and sources

The pinned Palette implementation applies Exposure in linear-light sRGB:
[document intent](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L824),
[EV control](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs#L499),
and [point evaluation](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L234).
The transfer constants are documented in [W3C CSS Color 4](https://www.w3.org/TR/2026/CRD-css-color-4-20261029/#color-conversion-code).
PaintCraft independently implements the exact mathematical policy below; it
does not promise equality with Palette's floating-point results or Photoshop.

The proposed numerical profile is `exposure-srgb-v1`, with one required integer
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

Later integration must connect typed descriptor construction, semantic identity,
history, a new immutable resolved schema bundle, PCND6 profile/codec resolution,
prepared CPU/GPU entry points, native/Web controls, explicit copy consent and
CLI/MCP inspection/export. Older eight-profile schemas and Worker capabilities
must remain closed until explicitly revised and qualified. This document grants
none of those capabilities.
