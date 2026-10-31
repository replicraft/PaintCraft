# Seeded noise numerical foundation

Status: the standalone numerical foundation passes native and actual Rust Wasm
qualification. This does not add an editable catalog profile. Studio currently
exposes twelve qualified profiles; noise still needs persistence, history,
rendering, host controls and automation integration.

## Intent and source coordinates

Pinned Palette revision `aefc84fe9233e66344147078de866860326d8cfa` provides
[Uniform and Gaussian noise](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L170).
Its layer evaluator supplies source-local pixel coordinates. This establishes
the intended controls and pattern anchoring, not floating-point byte parity.

`AddNoiseRgbV1` stores amount in integer hundredths from 0 through 100, a u16
seed, monochrome mode and `NoiseDistribution`. Defaults are amount 10, seed 1,
monochrome true and Uniform. Invalid amount fails rather than clamps. Its six
semantic bytes are unsigned amount u16 big-endian, seed u16 big-endian,
monochrome 0/1 and distribution 0/1 for Uniform/Gaussian. This is not yet a PCND
codec or permission to admit a new catalog descriptor.

`SourcePixel` is a distinct pair of unsigned integer source coordinates. Signed
conversion is checked. The caller separately validates the layer extent and
maps a document pixel to its source pixel; no implicit document-space conversion
or default position is allowed. Translation of a layer does not change its noise
pattern. Tile, viewport, thread and display coordinates must not enter the seed.

The evaluator consumes straight encoded RGB and preserves alpha exactly,
including hidden RGB at zero alpha. Amount zero is exact identity. Monochrome
reuses channel zero's noise field for all RGB components; color uses fields
zero, one and two. A future coverage consumer mixes the complete rounded RGB
candidate once through the existing A8 coverage contract.

## Deterministic fields

All operations below use wrapping u32 arithmetic:

```
state = 0xa511e9b3 ^ x*0x9e3779b9 ^ y*0x85ebca6b
        ^ seed*0xc2b2ae35 ^ channel*0x27d4eb2d
mix(n): n ^= n>>16; n *= 0x7feb352d; n ^= n>>15;
        n *= 0x846ca68b; n ^= n>>16
w1 = mix(state)
w2 = mix(w1)
```

The second Gaussian draw mutates the first draw's state. Independently
reseeding the second draw changes the policy.

For Uniform, let `D=2^32`, `k=2*w1+1-D`, amount `a`, and input byte `p`.
Evaluate `N=100*D*p+255*a*k` with checked i64 arithmetic, clamp to
`[0,255*100*D]`, and divide by `100*D`, rounding once to nearest with ties upward.
The absolute unclamped numerator is below 2^48; doubled rounding arithmetic is
below 2^49. There is no intermediate normalized floating-point rounding.

## Gaussian binary64 policy

Gaussian uses the complete Box-Muller transform. Its authority is this named,
pinned software-binary64 operation graph, with nearest-even primitive arithmetic:

```
u1 = (f64(w1) + 0.5) * 2^-32
u2 = (f64(w2) + 0.5) * 2^-32
l = libm::log(u1)
r = libm::generic::sqrt::<f64>((-2.0) * l)
theta = f64::from_bits(0x401921fb54442d18) * u2
z = r * libm::cos(theta)
delta = ((f64(a) / 100.0) * z) * 255.0
v = f64(p) + delta
byte = libm::round(clamp(v, 0.0, 255.0))
```

Preserve the indicated association. No f32 reduction, fused multiply-add,
fast-math reassociation, operating-system transcendental functions, sampled
distribution or central-limit substitute is permitted. The dyadic mappings are
exact; their domain is `[2^-33,1-2^-33]`. The radius is below 6.764 and the
unclamped byte magnitude below 1980. Rounding the clamped nonnegative value with
`libm::round` implements nearest with ties upward; `floor(v+0.5)` is unsuitable
because addition can round a value immediately below a half upward first.

The dependency is exactly libm 0.2.16 with default features disabled and
`unstable-public-internals` enabled. The public generic sqrt deliberately
bypasses architecture dispatch. Source review finds the visibility feature
does not alter existing consumers' numeric dispatch. Global `force-soft-floats`
is not requested. The unstable API and operation graph require a fresh source
audit and native/Wasm qualification before any dependency update. Source review
alone does not establish compiled cross-platform bit equality.

This policy does not promise ideal-real correctly rounded Gaussian output,
Palette's f32 bytes, modified foreign FPU modes or proprietary application parity.

## Bounded execution and acceptance

Rows require an explicit source origin and equal source/output lengths. Checked
end coordinates permit the final pixel at u32::MAX but reject crossing the u32
domain. Geometry, work multiplication and quota checks precede callbacks or
writes. Initial, per-at-most-64-pixel chunk and final cancellation checks bound
cooperative work. On late cancellation, discard private output even if partly
or fully written. Source remains immutable; a row API does not publish a document.

Reviewed fixed work prices are 512 units per Uniform tuple and 4,096 per Gaussian
tuple, including reachable pinned software math and the colored worst case.
Conservative source-action envelopes are 320 and 3,008 respectively; the expanded
ledger is `docs/validation/noise-foundation/source/WORK_MODEL.md`. Neutral and
monochrome rows retain their distribution's price. These are modeled source
actions, not generated instructions, cycle counts or measured speed. No renderer
or worker execution registration is activated by this standalone foundation.

The independent mathematical oracle was frozen before product implementation:
4,380 candidates and 26,280 A8 controls, including 60 forced hash-boundary cases.
Uniform uses exact Fraction arithmetic. Gaussian uses independently authored
high-precision logarithm/square-root and Machin-pi/cosine references, separately
recording ideal two-pi and exact binary64-TAU mathematical expectations. Agreement
at two precisions is finite evidence, not a certified global error interval or
software-libm intermediate-bit authority. The frozen source receipt is
`docs/validation/noise-oracle/source-ready.json`, SHA256
`da7945dd1eeffa763df9d8c0e66c2e211bbe8c6dbd2cf9d19bf37abeb6b62651`.

Native qualification passes 91 Core tests, seven focused noise groups, all 79
Effects release-library tests and both crates' all-target strict Clippy checks.
The actual scalar and row APIs match all 4,380 independent candidate expectations
and 26,280 mask expectations. Explicit source partitions, the final u32 pixel,
invalid geometry, work denial before callbacks/writes, empty-row checks, alpha,
hidden RGB and cancellation prefixes 0/0/64/128/130 pass. Private discard, guard
preservation and subsequent reuse pass for both modes and mono/color settings.
All 127 local graph, configuration and toolchain inputs match before and after
every stage. The original first native process exits zero. Receipt:
`docs/validation/noise-foundation/native/attempt-20261031T174612Z-4996c58e/final-freeze.json`,
SHA256 `7f49679a232348ad2af732fb8d88f2c2cdfbfc83ce039ccf1ae615633fdd6d3c`.

The generic software sqrt API compiles. The selected native Effects consumer
enables only libm's `unstable-public-internals` feature; the full-workspace
metadata union also contains existing consumers' `arch/default` and is not
misrepresented as this narrow compiled graph. Source writing initially stopped
at ENOSPC before any product edits or compiler; preimages were saved only after
headroom recovered. This environment failure is distinct from the later passing
first native test run.

Actual Chrome executes the real Rust Wasm scalar and one-pixel row APIs for all
4,380 specimens and Rust coverage mixing for all 26,280 masks. All outputs match
the independent mathematical fixture. The chosen native/Wasm records match
exactly: 43 little-endian u64 fields per specimen, SHA256
`da5390ceffb54212c6867a935d89a0be377370fbeca7c9468b391c4b3a4a73ae`.
The 1,752 active Gaussian records additionally contain twelve named stages per
channel from a separately written reference graph using the selected pinned
library. Those intermediate bits are implementation observations, not private
product tracing or independent mathematical expectations.

Both distributions pass geometry and quota refusal before callbacks/writes,
last-u32 and empty rows, all five cancellation phases, source/tail preservation,
private discard/reuse and 64+66 partition equality. Identity retains each
distribution's reviewed work price. The 61,962-byte module, SHA256
`35cd7a11c02ce3a0941b95342a322bd345ca2cf07cee36a582a20e054929ec6e`,
uses real wasm-bindgen 0.2.129 initialization, twenty-one wrapper exports and
the sole externref initialization import. Its nonshared memory starts at 2 MiB,
has a declared maximum of 16 MiB and remains at 2 MiB in this finite run.
Native and Wasm Cargo artifacts select only `unstable-public-internals` for libm.
Read-only native disassembly finds no fused multiply-add in the inspected log
and reference-graph ranges. This is not an all-target compiler proof or RSS bound.

The first Chrome attempt stopped at the first negative logarithm bit pattern:
the harness compared a signed JavaScript Wasm i64 return with an unsigned native
record. A separate diagnostic retained all 10,285 signed representation
differences and found zero normalized bit differences, while independently
finishing every product output check. Correcting only the harness comparison
then passes the acceptance run on the SAME module, glue, native records and
expected fixture. No math, product source or compiler output changed. The
original failure, diagnostic and successful acceptance remain separate records.

The native/Wasm build preserves 261 inputs. The final binding retains all 259
unchanged inputs and explicitly binds the two amended ignored browser-harness
files, rather than calling the original manifest unchanged. The earlier native
127-input proof remains exact. Final receipt:
`docs/validation/noise-foundation/qualification/final-freeze.json`, SHA256
`3027d1cd05f5a563c24e3d9576ffb04c74d26c19b0b9a9cdf1d4a1aa17717946`.
No timing or performance gain follows from these conformance tests. Main,
Source5/Export7, Studio, CLI/MCP and editable document support remain future gates.

Later catalog integration must prevent coordinate-dependent noise from using a
uniform-source collapse proof and must charge the captured caller before work.
Normal canvas sizes additionally require the separately versioned
[scoped workload contract](NOISE_WORKLOAD_CONTRACT.md); old Source5/Export7 limits
are not silently enlarged by registering the new numerical policy.

## CI and isolated parser linkage

The [standalone integration CI run](https://github.com/replicraft/PaintCraft/actions/runs/41227109331)
for exact commit `8b8cf23b963c7a672447aa96c2330bdbe9f21a37` finishes with six
successful jobs and one failed Ubuntu job. Ubuntu's workspace tests pass; its
subsequent isolated stable-parser check stops immediately because `fuzz/Cargo.lock`
lacks the new Effects-to-libm edge. Later parser commands are not treated as
executed. The original watch, failed-step log and final API remain preserved in
`docs/validation/noise-foundation/ci-41227109331/final-freeze.json`, SHA256
`d9e31168724da21b0eec92eec91527dc59721c98aad508f66e1a037cd75d6428`.

The corrective isolated lock adds only libm 0.2.16 with the product lock's exact
source/checksum and the Effects dependency edge. Local macOS qualification passes
20 Python tests, all-target strict Clippy, 25 Rust parser tests, formatting, replay
build, both historical/stock/PCND replay suites and final lock validation. All
2,684 captured inputs match before/after every phase. Receipt:
`docs/validation/noise-foundation/parser-lock-fix/attempt-20261031T184248Z-6cf084dd/final-freeze.json`,
SHA256 `cf18b5993d7468add1d393117abcee497440b7abbba23bc8e60ca8cff9d663c7`.
This verifies the isolated parser linkage and existing parser cases; it does not
claim persistent Noise codec fuzzing.
All seven [corrected-head CI jobs](https://github.com/replicraft/PaintCraft/actions/runs/41231000555)
pass for exact commit `a8d21c9c6b46d9f5c2fb32057739beacff03f9cc`.
Receipt SHA256 `150c3f3e3cb89ad8d3d2d8e59f3820def615746809d10b55a32092e6708e1d08`
is recorded under `docs/validation/noise-foundation/ci-41231000555/`. This result
excludes later benchmark additions and editable Noise integration.

The [native kernel baseline](../benchmarks/noise-kernel-2026-10-31/DECISION.md)
now records three statistical/process sessions for the default canvas and actual
1080p. These timings qualify the measured scalar workloads only; they do not
replace Wasm or real host responsiveness gates.
