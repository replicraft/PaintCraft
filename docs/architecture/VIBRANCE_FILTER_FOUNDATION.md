# Vibrance numerical foundation

Status: the standalone type and evaluator pass native and actual Rust Wasm
qualification. No Descriptor, catalog schema, persistence codec, execution
registry, worker or Studio capability is added. The public supported-filter
count remains ten. This is the next HSL-family foundation in the Palette plan.

## Intent and exact policy

Pinned Palette commit `aefc84fe9233e66344147078de866860326d8cfa` applies
`s' = clamp(s * (1 + a * (1-s)), 0, 1)` to HSL saturation while preserving hue,
lightness and alpha. Its [point evaluator](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L255)
establishes intent, not an exact byte oracle. PaintCraft independently implements
the rational policy below and does not claim proprietary skin-tone protection
or upstream floating-point byte equality.

The standalone type `VibranceRgbV1` admits `amount_hundredths` in -100 through
100, default zero. Invalid values fail rather than clamp. Its semantic bytes are
the signed two's-complement integer in big-endian order. This type and the
prospective numerical ID `vibrance-rgb-v1` grant no transport capability.

Operate on the immutable incoming straight encoded-RGB tuple. Let:

```text
hi = max(R,G,B); lo = min(R,G,B)
S = hi + lo; d = hi - lo
q = 255 - abs(S - 255)
D = 100*q
F = D + amount_hundredths*(q-d)
N(c) = S*D + (2*c-S)*F
candidate(c) = (N(c)+D) // (2*D)
```

Amount zero and achromatic input return exact incoming bytes. Otherwise
`0 < d <= q`, so the denominator is positive. HSL saturation is `d/q`;
the multiplier `F/D` scales each channel's displacement from midpoint `S/2`.
For nonpositive amount, `0 <= s' <= s`; for positive amount,
`s' <= 2s-s² <= 1`. Thus the saturation clamp is mathematically inactive on
the legal domain, the exact RGB candidate is within 0..255, and the formula
performs one nearest-byte rounding with exact halves upward.

Fully saturated colors remain unchanged for every amount. Amount -100 squares
HSL saturation; it does not mean complete desaturation. Compute every channel
from the original tuple before returning a candidate. Copy alpha unchanged,
including transforming hidden RGB at alpha zero. Halo is zero; original ICC
remains provenance rather than an active working profile.

Use the existing point-mask policy once after candidate rounding. Intermediate
coverage blends the rounded candidate and incoming RGB as a separate rounding
stage; never blend parameters, fuse unrounded HSL into coverage, or apply twice.

## Checked arithmetic and resources

All non-byte arithmetic uses checked i64 operations. A separate bound experiment
enumerates all 32,896 min/max pairs and 130,560 chromatic endpoint corners.
The formula is bilinear in channel/amount for fixed min/max, so its corner bounds
cover intermediate channels and amounts. Its largest recorded intermediate is
13,030,500; the conservative evaluation-order bound is below 2^25. This is a
capacity proof, separate from independent numerical expectations and speed.

`paintcraft_effects::vibrance` exposes `quote(length)`, `Quote`, `Limits`,
`WorkUsed`, `evaluate_pixel` and `evaluate_row`. No LUT, retained index, heap
arithmetic, floating point, unsafe operation or new budget realm is required.
The zero caller-scratch quote excludes bounded scalar stack bookkeeping and
caller input/output; it is not zero physical memory or a process RSS bound.
Typed-error diagnostics and callbacks may allocate.

Quote and length multiplication are checked before callbacks or writes. Charge
a fixed conservative 64 modeled units per tuple, including identity/grayscale.
The numerical path uses at most 49 counted arithmetic/conversion/component
actions; this model does not count CPU instructions, cycles or callback work.
The row API checks initially, before each chunk of at most 64 pixels, and finally.
Preflight/initial refusal preserves output. Later cancellation can leave private
output partially or fully written; callers must discard it. This is not the LUT
compilers' final-copy-only publication contract. Source is never mutated.

## Independent fixtures and qualification boundary

The independent Python Fraction oracle explicitly converts RGB to HSL, adjusts
saturation, and reconstructs hue-sector RGB. It does not import the simplified
product formula. Its 5,929 RGB samples, 768 gray controls, 46 exact half ties and
4,860 RGBA/mask controls have SHA256
`447b959c646862afa28bb1212d809794b5a65636777ac108863c7e04bdec8dec`.
Fixtures and reproduction instructions live under
`crates/services/paintcraft-effects/tests/vibrance-fixtures/`.

Registration, canonical persistence, cumulative host admission, ordered-stack
rendering, UI and automation are separate integration stages. A whole-RGB
transform cannot be represented by the existing per-channel lookup tables;
adding another prepared-LUT family would be incorrect.

## Native and actual Wasm checkpoint

The full Core suite passes 83 tests. Five focused Vibrance debug tests and all
65 Effects release tests pass, including the existing filters. Core/Effects
all-target strict checks pass. The first native pipeline passes without a source
fix, with all 117 actual/candidate/oracle inputs unchanged. The native receipt is
`docs/validation/vibrance-foundation/native/final-freeze.json`, SHA256
`4e26655ff85baa54b5e7a33826e0cfb950116c9c24e4d7c5dd264f499015da9a`.

Actual Chrome executes the real Rust Wasm and generated wasm-bindgen 0.2.129 glue.
All 5,929 RGB samples, 768 grays, 46 exact half ties and 4,860 Rust RGBA/mask
records match the independent oracle; 6,347 partitioned row calls also pass.
There are 170 causal hidden-RGB cases. Three geometry and two low-work refusals
invoke no callbacks. A 130-pixel row checks cancellation at all five checkpoints,
leaving exactly 0, 0, 64, 128 or 130 private pixels written. Every refused private
result is discarded; successful reuse, immutable source and output-tail guards
pass. Final cancellation's complete private output is not published.

The actual module is 50,989 bytes; its only import is the real generated externref
initializer. Nonshared memory starts at two MiB, has a declared 16-MiB maximum,
and remains two MiB during this finite run. This is not process RSS or a speed
result. No fake import, host arithmetic replacement or alternate backend supplies
the Rust results.

Both original build and browser processes pass on the first attempt, with all
117 native-qualified inputs, seven harness sources and 95 build inputs unchanged
through final closeout. The receipt is
`docs/validation/vibrance-foundation/wasm/final-freeze.json`, SHA256
`e2b6c18ed95853bc1710f663be5251f5c21756411634de7e93c4ce1950bf46d6`.
All jobs are terminal and owned browsers/servers are closed. Editable filter,
frontend/export delivery, performance, new-head CI and upstream parity remain
separate gates; this checkpoint does not add an eleventh supported filter.
