# Hue/Saturation numerical foundation

Status: the arithmetic foundation is qualified on native and actual Rust Wasm.
Editable integration passes native Studio, history, PCND6, CLI/MCP, priced CPU
rendering and Metal presentation. Fresh browser Source5, Studio and DDS/MSP
export workflows pass. Hue/Saturation is the twelfth editable Studio profile.
Full package regression passes; integrated-head CI remains a separate gate.

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
All seven [foundation CI jobs](https://github.com/replicraft/PaintCraft/actions/runs/41213743376)
pass for exact standalone commit `5428dc01408a5f74487b0a5c04e494851b133a4d`.
The original watch exits zero and the final API independently reports all seven
successful. Addendum SHA256
`8b3f751994589d8c39f549e816ca61877e3c8bf7b13229e7fc6b409ba365323c`
is recorded in `docs/validation/hsl-foundation/ci-41213743376/push-ci-addendum.json`.
This result excludes later editable integration.

## Editable integration and finite delivery evidence

The explicit catalog descriptor is `hue-saturation-rgb-v1`, with all three integer
fields required. Old unchecked/legacy constructors remain closed. The
[PCND catalog](PCND_FILTER_CATALOG.md) records its exact six-byte `params-v1`
codec; disabled and neutral intentions remain stored. Removing the last
catalog-only intention restores minimal legacy output. The new immutable
12-profile schema bundle is
`a152a29d66e211f4261deba1c47ecb5f191d925eccede1ecd2ae7c04006bc87a`;
all 24 historical eight/nine/ten/eleven-profile JSON files remain byte-identical.

The explicit execution registration is `DirectHueSaturation`, resource
`direct-hue-saturation-rgba8-work128-v1`, revision 1. The renderer retains only
policy parameters, without a LUT, preparation owner or new prepared family.
Per-descriptor quotes preserve Vibrance's 64 and Hue/Saturation's 128 modeled
units. Nonspatial uniform/chunk evaluation debits the captured caller before
mutation; mixed spatial plans spend their complete reverse-halo quote once.
Source5 excludes only the derived direct portion from its initial bulk debit.
Export7 has no such bulk precharge. No outer protocol or resource ceiling changes.

Shared direct predicates preserve optional display-proof avoidance and exact-ROI
cold-cache clipping for any active Vibrance/Hue/Saturation document. Actual
snapshot-byte facts, decoded stack counts, explicit default wire records and
source hashing retain their earlier authoritative behavior. Old Source2/3/4 and
Export4/5/6 remain closed, including disabled/default intentions and private edits.

Native qualification passes Core 89, Commands 31 unit plus eight integration,
Effects boundary five and History 28 tests, with all four packages' all-target
strict checks. Actual cold spill drops both source document Arcs and reconstructs
signed six-byte parameters, masks, metadata and unchanged raw raster payloads;
Undo/Redo preserves distinct -180/+180 intentions and releases owned budgets.
Commands' original library-only count is explicitly supplemented, not called a
full-package result by itself.

Broader qualification passes Renderer 203 tests, including all 49 required actual
GPU tests on Apple M1 Pro Metal, Worker 43, independent PCND two groups covering
13 positive/17 negative authenticated specimens, all seven registry groups and
closed export profiles. Focused public-cache and ordered tests cover work bounds,
halos, masks, same-parent authority, no extra prepared key and independent order
witnesses. GPU presentation uploads CPU-evaluated HSL pixels; these are not HSL
shader-kernel results. Native all-target strict checks and Worker Wasm strict pass.
Two initial test-only mistakes are preserved: a layerless registry fixture and a
complex generated tuple type; explicit layer setup and a generated type alias fix
them without changing production arithmetic or expected numeric records.

The complete old-eleven matrix matches every field before and after on native
and actual Wasm separately: 43 cases each, six captures, layouts, work, callbacks,
accounts, public caches and snapshot facts, with no exclusions. All six historical
Main-produced 128-byte export contexts match on actual Wasm. These are historical
packets, not newly constructed Main requests. Receipt
`docs/validation/hsl-planning/post-old11/final-freeze.json`, SHA256
`98d88598930ffc69acb68fb0ab10702090bfa90911b8d137b724d33066e83fdf`.

Actual native Studio uses all three sliders and verifies private output prefixes
`[128,96,16]`, `[144,103,0]`, `[47,34,0]` for opaque `[64,16,128]`, then Cancel,
one-step Apply, fresh PCND6 open and Undo/Redo in synchronous and asynchronous
execution. Studio's 11 test groups, real CLI two and stdio MCP two pass. All 36
fresh result/envelope/inspection captures validate under the new bundle and
refuse each historical bundle; 84 genuine historical captures validate under
their own and the new bundle. Host receipt
`docs/validation/hsl-catalog-browser/host-runtime-p1u89aui/final-freeze.json`, SHA256
`e181cae01fba62b0b4e77fb7c23034861322619d9d22359d72615240793af748`.

Fresh production Main, Source5, codec7, Astro and asset checks pass with all 3,574
inputs exact before and after every stage, including ignored public font metadata
from the first snapshot, and 27 frozen assets. Actual browser Source5 passes 24
cases/229 checks; Studio passes ten lifecycle records with real controls,
independent screenshot markers, bound worker planes, source/title/clean-state
preservation and fresh reopen. DDS/MSP7 browser exports each pass sixteen fresh worker replays, actual Main
consent/revocation/recheck with fresh nonces, download, and unchanged project
bytes/title/dirty state. Independent Pillow decoding confirms opaque DDS
`[47,34,0]` and MSP white. MSP applies H137/S73/L+10 to yield `[195,139,0]`, then
Threshold64: skipping HSL would produce black, so its white result is causal.

The aggregate browser receipt is
`docs/validation/hsl-catalog-browser/browser-final-freeze.json`, SHA256
`8b9e293e17da80bf5d3e76c72d95641c99e9b8079efc094ddb2a78a4b8720eac`.
All 3,574 producer files, 27 assets and 29 executed harness/reference inputs are
current and exact. Thirteen calibration attempts remain explicitly calibration,
not additional semantic passes. The first raw report's top-level zero check count
was corrected to the actual nested 229 without a runtime or source rerun.
All browser/server/worker processes are closed.

Full regression passes 405 web tests (404 unit plus one integration, three
existing ignored) and 240 CLI/MCP tests (fifteen existing ignored, 44 result
groups). The first pipeline's web stage passed; its later CLI installed-codec
stage failed when storage filled, leaving some closeout JSON truncated. The raw
log, original exit and truncated files remain preserved. Three failures report
StorageFull; the first JXR helper exit six retains its initially unproven cause.
After removing completed disposable test executables, the exact JXR case and
full CLI package pass without source changes. The earlier successful web stage
is retained rather than repeated or called a wholly successful original pipeline.

All 4,323 captured local inputs, 3,574 producer files, 27 assets and symlink facts
match around successful phases and at final recovery closeout. Earlier failed
post-observations are not fabricated: recovery observations are labeled later.
Receipt `docs/validation/hsl-integration/full-regression/final-ledger.json`, SHA256
`00a7dcf48ff4456a0096f7341e4fd5a3abb0d3e07e72601d4a9fa9e732fd2a7c`.
New integrated-head CI remains a separate pending gate at this checkpoint.

See [Palette integration](PALETTE_INTEGRATION.md) for the remaining inventory.
