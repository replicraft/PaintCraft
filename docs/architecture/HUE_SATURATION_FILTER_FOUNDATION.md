# Hue/Saturation numerical foundation

Status: standalone source implementation and independent finite reference are
reviewed and locally qualified on native and actual Rust Wasm. This document
does not register a twelfth editable filter, persistence codec or worker
capability. The eleven existing profiles remain the public feature count.

## Intent and parameters

Pinned Palette revision `aefc84fe9233e66344147078de866860326d8cfa` stores hue,
saturation and lightness adjustments. Its
[point evaluator](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L240)
adds hue, multiplies HSL saturation and adds HSL lightness. This establishes
intent, not floating-point byte parity. PaintCraft's policy uses exact integer
units corresponding to the upstream UI steps; upstream API values can also be
fractional and are not represented by this bounded policy.

`HueSaturationRgbV1` admits hue degrees -180 through 180, saturation hundredths
-100 through 100, and lightness hundredths -100 through 100. All default to zero.
Invalid parameters fail rather than clamp. The semantic identity is six bytes:
H, S and L in that order, each signed i16 big-endian. Hue -180 and +180 retain
distinct intentions even though their evaluated colors are equal.

The evaluator consumes the original straight encoded RGB tuple. It preserves
alpha exactly, including transforming hidden RGB at alpha zero. Source ICC
remains provenance; this policy does not activate color management. It computes
normalized HSL changes exactly:

```
h' = (h + H/360) mod 1
s' = clamp(s * (1 + S/100), 0, 1)
l' = clamp(l + L/100, 0, 1)
```

Only final RGB channels round to nearest, with ties upward. All-zero parameters
are exact byte identity. Saturation -100 yields the rounded original HSL
lightness as gray; lightness -100/+100 yields black/white. A saved coverage mask
would mix the rounded candidate once afterward through the existing point
coverage policy. This standalone evaluator does not own masks or publication.

## Checked integer formulation

Let `hi=max(R,G,B)`, `lo=min(R,G,B)`, `T=hi+lo`, `d=hi-lo` and
`q=255-abs(T-255)`. Set:

```
u = clamp(50*T + 255*L, 0, 25500)
m = 25500 - abs(2*u - 25500)
Ds = 100*q
Ns = min(d*(100+S), Ds)
```

Achromatic input has no hue division and outputs `(u+50)/100` in each channel.
On the chromatic branch `q>0` and `d>0`. With R/G/B maximum priority, define
`n=G-B`, `B-R+2*d` or `R-G+4*d`, respectively. Then:

```
Dh = 60*d
W = (60*n + H*d).rem_euclid(6*Dh)
k = W / Dh
f = W % Dh
z = f when k is even, otherwise Dh-f
```

Sector coefficients for R/G/B are `[Dh,z,0]`, `[z,Dh,0]`, `[0,Dh,z]`,
`[0,z,Dh]`, `[z,0,Dh]` and `[Dh,0,z]`. For each coefficient `p`:

```
D = 200*Ds*Dh
N = 2*u*Ds*Dh + m*Ns*(2*p-Dh)
out = (N + D/2) / D
```

Euclidean hue wrap is required for negative adjustments. No intermediate
rounding or per-channel LUT can replace the coupled RGB calculation.

The denominator is positive and at most 78,030,000,000. The sum of independent
absolute numerator bounds is at most 29,846,475,000,000, below 2^45. HSL geometry
further bounds `N` to `[0,255*D]`; the rounding numerator is at most
19,936,665,000,000. Checked i64 arithmetic therefore suffices without floating
point or an allocated table. Impossible overflow or range failure is a typed
error, never byte truncation. The source-only bound experiment covers 391,680
vertices over 32,640 positive denominator pairs; its multi-affine corner proof
is a capacity argument, not exhaustive numerical conformance.

## Execution and qualification gates

The standalone row contract uses caller-owned output, no LUT or heap
arithmetic, and no caller scratch buffer. Bounded scalar stack bookkeeping,
typed-error diagnostics and callback allocations are excluded from that scratch
figure. The price is 128 modeled units per tuple, including neutral, gray and
clamped cases. Review counts at most 113 source numerical, conversion, selection
and component movement actions on the full chromatic path. Checked Result
plumbing, function calls, row orchestration and compiler lowering are outside
this model; it does not bound generated instructions, cycles or measured time.

Geometry, checked work multiplication and quota validation precede callbacks or
writes. Initial, per-at-most-64-tuple-chunk and final cancellation checks bound
cooperative work. Cancellation may leave private output partly or fully written;
the caller must discard it. Source remains immutable. No row API alone promises
atomic document publication.

The independent Fraction HSL reference was frozen before product implementation:
2,720 RGB cases, 266 causal half-channel ties, 3,240 literal RGBA/mask controls and
15 strict invalid-parameter controls. It imports only the tracked independent
Vibrance fixture's RGB/HSL reference, not the integer formulation above. Its
original oracle SHA256 is
`490cc0da56a18139abbed89b3866e449824a857ef816135e3d70c38244f25e72`.

Native qualification passes all 87 Core tests, five Hue/Saturation groups, all
71 Effects release tests and both crates' all-target strict Clippy checks. All
115 captured local graph, manifest, configuration and toolchain inputs match
before and after every step. The original process exits zero; no native failure
or retry was needed. Receipt:
`docs/validation/hsl-foundation/native/attempt-20261031T150445Z-6e108169/final-freeze.json`,
SHA256 `8acc0891ea6340e8f1af3b771dd2936efa8f60712059c579c25defa2d7e48dbf`.

Actual Rust Wasm in installed Chrome passes all 2,720 RGB records, 3,000 row
calls, 266 half ties, seven pinned literals and 3,240 RGBA/mask controls. Both
ordered result hashes match the independent reference; 322 hidden-RGB controls
are causal. Three geometry and two quota refusals occur before callbacks or
writes. Cancellation at every phase of a 130-tuple row produces the expected
private prefixes of 0, 0, 64, 128 and 130 tuples; guards, source preservation,
discard and subsequent reuse all pass.

The 51,774-byte module uses real wasm-bindgen 0.2.129 initialization, its sole
externref-table initialization import and all eighteen required qualification
exports. Nonshared memory starts at 2 MiB, has a declared maximum of 16 MiB and
stays at 2 MiB through this finite run. This is standalone kernel qualification,
not a Main/Worker protocol, editable Web Studio feature or physical RSS bound.

The first artifact inventory failed because its test helper still required
Vibrance export names. Rust compilation and bindgen had succeeded. Correcting
only that ignored validator allowed inspection of the SAME compiled bytes;
no product arithmetic, oracle or compilation changed. The original failing
process and executed helpers remain recorded rather than being called a clean
first pass. Chrome then passed on its first execution.

Wasm closeout:
`docs/validation/hsl-foundation/wasm/final-freeze.json`, SHA256
`cea3d0d95442fc11eac1d102f6e35cc892443dcc95a3f983851b58e9b058648f`.
It binds all 115 native inputs and 105 Wasm qualification inputs unchanged,
alongside native receipt `8acc0891` and the compiled module SHA256
`f57e5028ad0c243ebd740e19bbc26be646919f1793839628a131042a7238292b`.
Descriptor, codec, immutable schema, history, priced renderer, private Studio
editing and actual browser export integration follow as separate acceptance
gates. See [Palette integration](PALETTE_INTEGRATION.md) for the wider inventory.
