# Vibrance numerical foundation

Status: standalone arithmetic and editable integration are locally qualified on
native and actual Rust Wasm. Vibrance is the eleventh Studio filter, with PCND6,
CLI/MCP, private parameter editing and browser preview/export support. Full
package regression and all seven integrated CI jobs pass for commit `b00bcce2`. Earlier
standalone evidence below keeps its original scope.

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
numerical ID `vibrance-rgb-v1` identify its policy. Execution and transport
registration are explicit boundaries described below.

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
rendering, UI and automation have separate integration evidence below. A whole-RGB
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
frontend/export delivery, performance, integrated-head CI and upstream parity are
not established by this standalone checkpoint. All seven
[foundation CI jobs](https://github.com/replicraft/PaintCraft/actions/runs/41201386321)
pass for `cdcdfcac`; later editable integration has separate scope.

## Editable integration and pricing

Catalog JSON is exactly
`{"kind":"vibrance-rgb-v1","amount_hundredths":0}`. Every required parameter is
an integer; missing/extra fields, coercion and out-of-domain values fail. PCND6
stores exactly two signed big-endian parameter bytes using profile-scoped
`params-v1`. Identity and disabled instances retain intent. The immutable
eleven-profile schema bundle is
`5b017aee79e713dd9d43a12a95211be727c66e3c16a08ae80e6b1bdbe1ac340b`;
all eighteen older eight/nine/ten-profile schema files remain unchanged.
All 26 genuine new CLI/MCP outputs validate against the new bundle and fail each
older bundle. Historical Curves26, Exposure26 and Levels6 outputs validate against
both their original bundle and the new bundle.

The explicit registry entry uses direct execution and resource contract
`direct-vibrance-rgba8-work64-v1`, revision1. Catalog-only means explicit
construction and transport; it does not mean LUT compilation. No new prepared
family, captured key array, compile work or retained table is added. Existing
Levels/Exposure/Curves owners can coexist with direct arithmetic on the same
caller admission. Legacy point facades refuse unpriced Vibrance before callbacks
or output writes.

Spatial plans include 64 additional units per Vibrance stage output pixel using
the existing reverse halo walk, and bulk-charge the complete plan once. Point
appearance reserves candidate work per uniform attempt or at most 64-pixel chunk,
using the captured parent's cumulative counter. Identity and mask-zero slots
remain conservatively charged. Source5 includes the two-pass direct bound in
admission, removes it from the initial generic bulk debit, and spends it during
evaluation. Export7 already uses actual appearance debits and receives no such
subtraction. Old work limits and memory ceilings are unchanged.

Public CPU/GPU/cache entries require explicit admission before cache lookup.
Vibrance cache misses render only the priced ROI. Optional display-level uniform
proofs are skipped for relevant Vibrance content, retaining the normal appearance
proof and dense fallback within the quoted bound. A causal two-pixel case now
accepts quote256 and spends192; before correction it needed limit384 and spent320.
A one-pixel cached ROI accepts quote128 and spends64; below-bound limits refuse
before spending. Dirty patches, off-ROI layers, LOD, cold/warm caches and actual
Metal scaled display have separate controls. These are abstract work units,
not latency or speedup measurements.

All 27 prior ten-profile records match every raw field on native and actual
Chrome Wasm separately, with no allowed data/layout delta. All four historical
Main export contexts match their complete 128 bytes in Wasm. Factory storage
stays native80/Wasm48 and final reservations retire to zero. The final receipt is
`docs/validation/vibrance-integration/final-post/final-freeze.json`, SHA256
`79c707c7677bf331a35154509c1d73955b7968996a1fd7b0b647d27333967fd7`.

## Canonical snapshot facts and closed workers

Generic7 export facts describe the actual immutable PCND bytes. Logical decoded
filter count replaces live Vec capacity in the payload quote. An authenticated
borrowed metadata walk counts every serialized tile record without inflating
pixels, including explicit all-default records retained by the reader. Inner
PCND authentication and whole-source SHA share one body traversal. Producer
`snapshot_contexts` replaces captured live residency before source-length
finalization and the parser-phase maximum; Header7 and Main use that returned
region/program/hash set. Workers continue validating actual decoded residency.
This avoids both compact default-tile omission mismatches and underpricing an
explicit default-valued raw record. Live/history capacity accounting remains
unchanged, as do legacy4/5/6 export paths.

The borrowed scanner passes writer-produced versions1..6, including their
different source-metadata presence rules, plus integrity, geometry, cancellation
and encoded-payload skip tests. Explicit raw default residency1 succeeds with
authentic claims; underclaiming zero refuses. Source2/3 private operations now
reject all catalog schemas before staging, fixing an inherited quote-only
Exposure/Curves acceptance gap. Source4 retains its fixed list. Export4/5/6
remain closed; Worker5's exact PCND5 framing refusal keeps its InputSchema code.
No outer protocol version or resource cap changes.

## Actual native and production-browser delivery

Local integration passes Core84, legacy Effects boundary4, Commands36 and
History27 tests. Cold compaction/spill/restore preserves signed parameter bytes,
raw source, mixed intent and unique mask/metadata accounting. Full renderer191
includes 46 required GPU tests; three focused controls record actual Apple M1 Pro
Metal. Native/Wasm strict checks pass. Studio9 and a native control lifecycle test
cover synchronous/asynchronous preview, Cancel, one-step Apply, fresh PCND6 open
and Undo/Redo. Real CLI2/MCP2 flows verify exact `[62,4,140]`, explicit bake consent,
no artifact on refusal and unchanged editable source. These are finite groups,
not physical-tablet or original-application qualification.

Fresh production Main, Source5 worker, Export7 worker, Astro and asset checks all
pass. All 3,491 captured producer files, 27 assets and 47 executed successful
harness inputs remain exact through browser closeout. Source5 passes 17 cases and
131 controls. Studio records six lifecycle groups using its real signed slider
and causal two-color screenshots. Actual Main DDS/MSP exports each pass consent,
revocation, fresh nonce, download and sixteen fresh worker replays. Independent
Pillow decodes opaque DDS `[62,4,140]` and MSP black; MSP's Vibrance100 followed by
Threshold30 would be white if Vibrance were skipped.

The aggregate browser receipt is
`docs/validation/vibrance-catalog-browser/browser-final-freeze.json`, SHA256
`1a7401798157b5f691d7451a9d927268da107f213827a0fcd3ead870eb5cfb62`.
First failures, calibration and source preimages remain recorded separately,
including the inherited Curves negative-forgery premise that needed adjustment
for zero prepared programs. All browser/server jobs are closed. Full package regression passes 405 web tests
(404 unit and one preview integration, three ignored) and 236 CLI/MCP tests
(fifteen ignored), with both original package processes exiting successfully.
All 4,264 captured regression inputs and 27 production assets match before and
after both phases. The final postcheck also matches all 3,491 producer inputs;
sixteen ignored producer files were supplemented from the earlier production
preimage and are disclosed as a later observation rather than an initial capture.

The full regression receipt is
`docs/validation/vibrance-integration/full-regression/attempt-20261031T144223Z-f49ed7ce/closeout.json`,
SHA256 `504994062bcc55681c6832922f9138d23813fce182543b70a8286178567e2eff`.
All seven [integrated CI jobs](https://github.com/replicraft/PaintCraft/actions/runs/41212265055)
pass for exact commit `b00bcce2eff3d28313323986b0dfe2a1b9c01ded`. The original
watch exits zero and the final API independently reports success for all seven.
The push/CI addendum is
`docs/validation/vibrance-integration/ci-41212265055/push-ci-addendum.json`, SHA256
`1f9c336d58835403ae3dec3af54e67a4fc033c8143d49cc240d46464a663e45c`.
Later Hue/Saturation work does not inherit this result. The broader Palette
inventory, layer styles, layer masks and performance remain separate work.
