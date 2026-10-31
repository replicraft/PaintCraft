# Levels numerical foundation

Status: implementation in progress. Levels is not an editable PaintCraft filter
yet. The existing seven descriptors, project profiles and automation schemas
remain unchanged. This contract prepares the next complete filter rather than
presenting a linear remap as full Levels.

## Source intent and parameters

The pinned Palette implementation stores five parameters, including gamma:
[document fields](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L851),
[controls and domains](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs#L645),
and [point evaluation](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L156).
The supplied [author inventory](../temp/IMPLEMENTED_FUNCTIONS.md) identifies the
feature but does not define its numerical behavior. PaintCraft independently
implements the following versioned encoded-RGB8 contract; byte equality with
Palette's floating-point implementation is not promised.

| Parameter | Inclusive integer domain | Default |
| --- | --- | --- |
| `input_black` | 0 to 254 | 0 |
| `input_white` | 1 to 255, strictly above input black | 255 |
| `gamma_hundredths` | 10 to 999 | 100 |
| `output_black` | 0 to 255 | 0 |
| `output_white` | 0 to 255 | 255 |

Output endpoints may be equal or inverted. Invalid input endpoints are refused;
they are never swapped or clamped. Gamma is the exact rational `g/100`, with no
NaN, infinity or binary-float parameter state. Future descriptor parsing must
require the complete parameter set and reject unknown keys and nonintegers.
The standalone parameter type does not register a descriptor or wire profile.

## Exact channel result

For incoming byte `v`, let:

```text
d = input_white - input_black
q = clamp(v - input_black, 0, d)
u = (q/d)^(100/g)
y = output_black + (output_white - output_black) * u
result = floor(y + 1/2)
```

Only the final result is rounded, with ties upward. Every valid result is already
in 0 to 255. Channels are independent; there is no histogram analysis, luma
conversion, active ICC transform or intermediate normalization quantization.
Alpha stays byte-identical and hidden RGB is transformed. A filter mask blends
the full rounded candidate once against this stage's incoming tuple through
the existing point-coverage policy. Ordered stages retain separate rounding.

The defaults are exact identity. Equal output endpoints produce constant RGB
while retaining alpha. Inverted default endpoints with gamma 100 exactly match
Invert's pixels, but remain a separate editable intention. Descriptor identity
must retain all parameters even when two settings produce the same lookup table.

At gamma 100, the result is the exact rational remap:

```text
N = (d - q) * output_black + q * output_white
result = floor((2*N + d) / (2*d))
```

That case is a useful fast path, not permission to ignore nonneutral gamma.

## Bounded lookup construction

For a rational threshold `a/b` in 0 to 1, compare `u` with that threshold using:

```text
q^100 * b^g  compared with  d^100 * a^g
```

Reduce both exponents by `gcd(100, g)` first. Exact equality identifies half ties;
inverted output endpoints reverse the inequality without reversing tie policy.
An independent oracle can binary-search output half thresholds. The production
builder may instead sweep monotone input/output boundaries and reuse powers.
It must prove the same byte, rather than use agreement with host `powf` as proof.

The unreduced product has fewer than 9792 bits because `q,d <= 255`, threshold
numerators/denominators are at most 510, and `g <= 999`. Fixed-capacity integer
scratch can therefore bound every intermediate. Concrete scratch layout,
operation accounting and construction limits must be checked with the actual
implementation. Scratch byte quotes are local ABI facts, not a project format.

The kernel builds a 256-byte RGB lookup table into caller-owned private storage.
It must have no heap allocation, per-pixel transcendental calculation or global
cache. Validate limits before work; check cancellation throughout construction
and immediately before publishing the completed table. Failure must leave the
caller's published output table unchanged. Scratch may contain discarded work.
Measure construction cost over the parameter domain before choosing runtime
limits or claiming that the implementation is fast.

## Prepared execution and remaining integration

Lookup application is cheap; repeated construction per pixel or per rendered
tile is not an acceptable integration. A future prepared point program must
construct the table once per admitted parameter set, retain its resource charge
for its lifetime, and reuse it across a frozen render job. Any cross-job cache
needs explicit runtime ownership, eviction, complete parameter/profile keys and
tests that release charges. A derived table is not authoritative document state.

The remaining gates are:

1. Validated parameters and an independent reproducible integer oracle.
2. A bounded exact builder, full-domain boundary controls, cancellation and
   output preservation, native/Wasm checks and quantified construction cost.
3. Prepared point execution with admitted table lifetime and row application.
4. A catalog-capable persistence and automation boundary. PCND3/4 stay closed to
   tags 1 to 6; PCND5 retains its stored-Box invariant. Levels cannot silently
   enter either grammar. Future algorithm IDs, parameter framing and required
   capabilities need independent specimens and early unknown-profile refusal.
5. Document/history/commands, all appearance consumers and explicit flat-copy
   consent, including default/disabled intentions and final-document downgrades.
6. Real Studio controls, private Apply/Cancel, Undo/Redo, save/reopen, CLI/MCP,
   CPU/Metal and production-browser qualification.

No catalog layout or future outer version is frozen by this numerical document.
The wider Palette catalog, layer masks and layer styles remain separate work.
