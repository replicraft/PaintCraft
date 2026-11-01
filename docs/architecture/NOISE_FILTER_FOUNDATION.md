# Seeded noise numerical foundation

Status: the numerical foundation passes native and actual Rust Wasm qualification.
Noise now has typed catalog intent, PCND6 persistence, recorded history, scoped
CPU appearance evaluation and qualified CLI/MCP rendering. Native Studio controls
pass headless tests. The established cross-frontend baseline remains twelve
profiles while Source6 browser delivery and remaining copy routes are qualified.

## Intent and source coordinates

Pinned Palette revision `aefc84fe9233e66344147078de866860326d8cfa` provides
[Uniform and Gaussian noise](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L170).
Its layer evaluator supplies source-local pixel coordinates. This establishes
the intended controls and pattern anchoring, not floating-point byte parity.

`AddNoiseRgbV1` stores amount in integer hundredths from 0 through 100, a u16
seed, monochrome mode and `NoiseDistribution`. Defaults are amount 10, seed 1,
monochrome true and Uniform. Invalid amount fails rather than clamps. Its six
semantic bytes are unsigned amount u16 big-endian, seed u16 big-endian,
monochrome 0/1 and distribution 0/1 for Uniform/Gaussian. The strict PCND6
`params-v1` codec now preserves this intent, including disabled and zero-amount
instances. Decoding intent does not grant renderer execution work.

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


## Integration checkpoint, 2026-11-01

The typed descriptor, command schema, history reconstruction and PCND raw/compact
round trips are implemented. The paired renderer admission preserves separate
base/Noise limits and cumulative ownership; a disabled or neutral Noise intent
cannot increase the base allowance. Coordinate-changing raster operations refuse
stored Noise before mutation until source-coordinate remapping is supported.

The frozen old-twelve comparison is byte-exact for all 59 native and 59 actual
Wasm records, with all eight authentic Main contexts unchanged and no excluded
layout, callback or accounting fields. Local receipt
`docs/validation/noise-planning/baseline/post-final-freeze.json`, SHA256
`3f5482cf41aba79fefefa9fe7d1b8d6acbf2ae8b8c28e7dacc5e67d47af7d3c2`,
pins that completed source boundary.

Actual CLI/MCP tests pass ten default/1080p complete-frame hashes previously
observed from the qualified numerical kernel, plus independent tiny Uniform and
Gaussian literals. Persistence, preview/export, bake refusal, no-op updates,
undo/redo, source preservation and reopen are exercised. The full CLI suite passes
249 tests, with 15 existing ignores; strict checks and eleven schema tests pass.
These results are in `docs/validation/noise-cli-runtime/final-freeze.json`, SHA256
`e5c7279822b6bded581df60746eb1995f702bad86282f4f28907d28eb12da736`.
Full-frame hash observations are host-integration references, not a second
independent mathematical oracle or benchmark.

The dedicated Source6 Worker passes 57 native tests and strict checks. Headless
Native App/Studio passes 764/27 tests with 29 existing App ignores and required
Metal access. That proof precedes subsequent print/export preparation changes:
`docs/validation/noise-worker-native/final-freeze.json`, SHA256
`219423e0aad851430f631ec6a08c76a534f5acda3c21aed859b008571e22cccb`.
Native ordinary printing and flat-image export now prepare appearance and encode
copies in background jobs. The full App suite passes 768 tests with 29 existing
ignores; four focused tests and strict checks pass after a borrow-only lint fix.
The receipt `docs/validation/noise-native-output/final-freeze.json`, SHA256
`212db21f25f968d20410915cd836e630f9292b5525766203f28385a2d37115aa`,
keeps the pre-lint full run and post-lint focused/strict boundary separate.

Actual Source6 qualification-constructor delivery passes 26 complete native and
Chrome Worker frames plus 26 matching-generation cancellation cases. Complete
native/Wasm output hashes match, including independent tiny literals; source,
owner, generation, reuse and cleanup checks pass. On the tested M1 Pro with
Chrome 156, the longest synchronous service call is 12.2 ms and the longest
cancellation acknowledgement is 6.5 ms. Default-size service jobs take
609.9–1051.7 ms and 1080p jobs 2545.7–4284.2 ms. These finite observations include
the qualification harness's per-step timer yields and output verification,
exclude Main serialization/UI transport, and do not establish statistical or
global bounds. The production pump instead uses MessageChannel; these completion
totals do not measure its scheduling or app-render latency. All proposed service targets
pass separately from functional checks. Receipt
`docs/validation/noise-source6-qualification/final-freeze.json`, SHA256
`cb8cfc2ab3ca93efcd21f6852ffe9b2be57a0712d7fdea834f7825b597c5e8a8`,
pins all 2,820 inputs and actual emitted artifacts. Runtime memory high-water was
not measured.

Main tool-preview headless regression passes 413 web Rust tests with three
existing ignores, strict native checks and all 274 JavaScript tests. An earlier
fixture incorrectly assumed source-coordinate selection movement was supported;
the corrected test verifies the explicit bake refusal preserves source, gesture
and the last good display. Both failed attempts remain recorded in
`docs/validation/noise-main-native/final-freeze.json`, SHA256
`f0948564d11effdaa20995a5c49e84c95d9f4675034fd313a46ab4515ee5f66c`.
Strict Main Wasm checks also pass with unchanged source inputs
(`docs/validation/noise-main-wasm/attempt-1/final.json`). The fresh production Main,
codec Worker, render Worker and Astro bundle pass with all inputs unchanged and
27 checked assets (`docs/validation/noise-web-production/attempt-1/final.json`,
SHA256 `124a5fce2056caecf9064e0ae5a6f5d6245148bf2333baa0c9fcdd8cc80cdbb0`).
Actual Studio browser UI subsequently passes 35 semantic checks on that frozen
production build, plus eight screenshot-calibrated coordinates: all four Noise
controls, private Apply/Cancel and preview-off publication, PCND6 save/reopen,
filter stack changes and minimal legacy header after final removal, real brush/
text previews and single Undo/Redo, authentic delayed view/source replies, and ten
finite old-profile samples. All 3,283 inputs and 52 artifacts remain unchanged.
Receipt `docs/validation/noise-studio-browser/browser-final-freeze.json`, SHA256
`b86d6a272d0bc1ac5c4349973f5adff89da11cb433927454ffa64d7f4800a368`,
retains earlier fixture/observer premise failures. This proof precedes subsequent
document-navigation and flat-output changes, which need fresh browser regression.
Ordinary browser copy routes and Export8 remain open. No GPU Noise kernel or
upstream floating-point byte parity is claimed.

The bounded PNG/JPEG/BMP adapter subsequently passes eleven actual production
Wasm FFI cases, including independent decoding, malformed selection retirement,
one-shot delivery, cancellation/reuse and paired-budget refusal. Its receipt is
`docs/validation/noise-flat-output-browser/final-freeze.json`, SHA256
`441504e7ed1a0a2abf6406d992ea6a2793d7ba1fba5c9b55f5181723d9f6b870`.
Actual Main attempt 95102 additionally passes all six positive delivery paths:
three downloads, the decoded print image, and whole/selected visible clipboard
copies with real OS clipboard readback. Only the OS printer invocation is mocked.
That attempt then fails a delayed-publication check and is retained as partial
evidence, not a passing suite: an obsolete job forwards cancellation on every
repaint, while the observer also incorrectly requires the latest cancel ACK.

Main cancellation now shares a once-only guard across cloned handles. Image
color picking with stored Noise also uses the existing single admitted worker,
requesting a level-zero raw ROI from the complete source document. It preserves
source coordinates and spatial-filter halos, averages straight RGBA with the
existing half-up rule, and checks current UI colors/options as well as tool and
document ownership before publication. Layer/raw and older-profile sampling keep
their existing paths. Superseded samples wait for acknowledged close; they do not
publish into the display cache or document history.

The combined changes pass all 431 web Rust tests (three existing ignores), strict
native all-target and Wasm checks, with all 3,297 inputs unchanged:
`docs/validation/noise-picker-native/attempt-1/final.json`. The fresh production
build also passes, retaining 53 artifacts:
`docs/validation/noise-picker-production/attempt-1/final.json`, SHA256
`1e0386de358994953813d99797ef095c5d26d7dea1e32a4248a20f60ee552c3c`.
Its Main Wasm is new; codec and render Worker bytes match the preceding build.
Actual Main replay on that new build now passes all eighteen output cases: six
positive routes and twelve delayed-publication fences across cancellation, owner
replacement, a new filter draft and a real accepted clipboard paste. Every fence
observes exactly one forwarded cancellation and its matching ACK before releasing
the held genuine result. All 3,297 inputs, 53 artifacts and harness files remain
unchanged; browser/server cleanup completes without page errors. The aggregate
receipt is `docs/validation/noise-main-flat-output-browser/browser-final-freeze.json`,
SHA256 `e73aae35e92290347ee82e20b1992bbe824ab13dcb38098e3d073a14ee3d5704`.
It retains five preceding failed attempts and their distinct harness/product
causes. The print image is genuinely decoded, but OS printer invocation is mocked;
clipboard success includes real browser write/read and independently decoded bytes.
At this output checkpoint, actual picker UI qualification remained pending;
the later finite picker result is recorded below. These output checks do not
establish Export8, browser flattening or other specialty-copy support.

The next local slice corrects a pre-existing web Studio bug: canvas input was
overriding its sampling controls with Classic's Image/One/Previous policy.
Classic retains that policy; Studio now uses its saved source, size and after-tool
choices. Ctrl captures an Image override in the gesture and immediately restores
the persistent preference, so releasing Ctrl cannot invalidate an asynchronous
sample. Headless canvas and normalized pen-fallback tests check causal pixels.

Browser flattening now requests whole-canvas Source6 BASE pixels, waits for source
close ACK, and supplies them to the captured editor `Preparation::flatten`.
Atomic publication retains selection and source metadata; one Undo restores the
complete original layers, raw pixels and filter intentions. Packed Main bytes
have their own reservation in the existing clipboard realm, retained through
ingestion, hashing and history preparation. There is no Main Noise-render fallback
or new worker grant. Ingestion remains synchronous; later finite browser
observations and their limits are recorded below.

These two changes pass all 442 web Rust tests (three existing ignores), strict
native all-target and Wasm checks, with all 3,300 inputs unchanged:
`docs/validation/noise-flatten-native/attempt-2/final.json`. The preceding attempt
retains two test-helper compile errors, corrected without changing production
behavior. The fresh production build passes with 53 artifacts and 27 checked
assets: `docs/validation/noise-flatten-production/attempt-1/final.json`, SHA256
`7533e3cac44706ea0a65c47094c4d5875e6427b75ce07b18c0921c3438c95604`.
All preceding production artifact bytes are preserved before rebuilding.
At this build checkpoint, actual Studio sampling and flattening qualification
remained pending; later results are recorded below. The eighteen-case output
receipt above belongs to its earlier explicitly pinned build.

CI for checkpoint `c378f626` subsequently exposes a debug-only fixture problem:
fourteen headless tests drop unapplied font texture deltas. The macOS job fails;
Ubuntu is cancelled, and the other five jobs pass. The failed run and sources are
retained in `docs/validation/noise-ci-41306090921/final-freeze.json`, SHA256
`396c473f2573bd2b1989fd8ae52e51a20568a42628884263c6860acdae7a62ca`.
Three test initializers now explicitly clear unused deltas as pinned epaint
requires for intentional non-rendering consumers. No assertion or product hook
is bypassed. Full **debug-profile** Web tests pass 442 with three existing ignores,
followed by native all-target strict checks and workspace formatting:
`docs/validation/noise-debug-fixtures/attempt-1/final.json`, SHA256
`74590a3df9ed5db192d5c67dcd587246577c976bfb2233eff556ec143d27f668`.
A fresh production build keeps all 53 artifacts byte-identical to the preceding
build while pinning the corrected 3,300 source inputs:
`docs/validation/noise-debug-production/attempt-1/final.json`, SHA256
`bc548f349fdf8434133b8f5b2375f644fd5b7584d47e15acb3787c16e2e800c4`.
All seven [CI jobs for exact `6641216f`](https://github.com/replicraft/PaintCraft/actions/runs/41309633465)
subsequently pass. Original watch54419 exits0 and the final API confirms seven
successes; the failed checkpoint remains separately preserved. Receipt
`docs/validation/noise-ci-41309633465/final-freeze.json`, SHA256
`b306d3e6edc80460af730c521c1e7b6914492b98c666240bde0cbe8b3873360e`,
pins this result. Local and CI evidence retain their own exact source scopes.

Native ICO, CUR and ICNS preparation and accepted encoding now share one captured
background job; retained preview handles keep their source reservation after
cancellation or source replacement. The full App suite passes 772 tests with 29
existing ignores, and strict checks pass. The first compile failure and its two
integration corrections are retained in
`docs/validation/noise-native-assets/final-freeze.json`, SHA256
`cc11a9fd5c6ef5f2327fad091554689bf4fcf7466f2d2ad0e4d14d7664ae3149`.
A subsequent test-only addition passes an actual staged-file test covering revoked
bake consent, cancellation cleanup and validated publication, with unchanged
source/history and final reservation release; strict checks pass again:
`docs/validation/noise-native-asset-write/attempt-1/final.json`.
These headless tests do not claim OS picker interaction or opaque encoder
allocator/RSS accounting.

## Latest finite consumer checkpoint

The actual Image picker subsequently passes four positive and three stale-result
cases, plus eight accepted-pixel Undo/Redo controls: fifteen semantic records;
twenty coordinate calibrations are separate. It verifies source-local Noise and
Box-halo sampling, raw-layer behavior, after-tool choices and changed owner/tool/
Controls-color rejection after real cancellation and source-close acknowledgments.
Sampling preserves the project; real follow-up pixel edits make the chosen color
observable. Receipt `docs/validation/noise-picker-browser/browser-final-freeze.json`,
SHA256 `4af45c8c5da044d6c8816e03232970d1aedf0721beac5582fdbcf1bbbe8ad17e`,
pins its 3,300 producer inputs, 53 artifacts and 27 assets. It does not establish
all picker sizes, geometry, physical pen behavior or later-build equivalence.

DDS8 and the shared document-derived Flatten eligibility rule now pass a separate
local checkpoint. Seven selected packages and integrations pass 1,942 tests with
32 ignored; Native App passes 774 with 29 ignored. Node passes 287 and Biome checks
110 files. Native workspace strict, Main/both-Worker Wasm strict and formatting
pass. Those runtime stages are bound through explicit later source projections,
retaining all seven native attempts and the initial nine Node fixture failures.
The default-feature Render GPU target executed **zero tests**. A separate explicit
`--features gpu` run passes 126 library and 50 GPU integration tests on actual
Apple M1 Pro Metal, with no ignores. Repeated Renderer groups are not additive
unique coverage. Receipt
`docs/validation/noise-dds-eligibility-native/final-freeze.json`, SHA256
`8e047e46027cbe85bc650c2c2e1c8b567a21e1a8ca34990e8125a938aaa528ec`,
records these scopes. Older Export4–7 profiles remain closed to Noise. No GPU
Noise kernel is claimed.

Actual DDS8 subsequently passes 43 Worker checks: nine independent fixtures through
direct FFI, maintained runtime and isolated Worker service give 27 positives,
with 16 refusal, resource and lifecycle controls. Successful completion releases
its declared service lease without quarantine. The final genuine in-flight abort
instead retains the 640 MiB declaration and one pending job, quarantined until
the isolated page/realm closes: Worker termination is not a join, and this is not
an immediate zero-byte-release or RSS claim. The qualification producer's maximum
64 MiB memory is distinct from the actual codec's unchanged 256 MiB limit. The
first helper environment failure, artificial 16 MiB helper snapshot refusal,
diagnostic36519 and corrected build3 remain preserved in the Worker aggregate.

The actual bundled Main then passes three cases with 18 checkpoints: Uniform
with two mips, Gaussian with two mips, and Uniform with untouched default
one-level settings. Genuine 704-byte requests and 864-byte responses bind source,
owner, settings, nonce and complete context echoes. Explicit bake prepares a
preview; revoking consent prevents download, and rechecking uses a fresh nonce.
Real DDS downloads equal the captured payload and independent decoded top/mip
pixels. Cancellation witnesses actual Worker termination before release of the
unchanged late reply, with no download. Exact saved source, title, dirty state and
observable history remain unchanged. Calibration-only and two failed harness
attempts are retained separately from these final passing cases.

`docs/validation/noise-dds8-integration/complete-consumer-final-freeze.json`, SHA256
`d9a4ce1a87d7750cddadde0789f8e296a3c5464cf48e3efbb3861c21145e6083`,
binds the immutable Worker aggregate to Main session7799's passing receipt, the
same a9af producer, 3,326 current inputs, 53 artifacts and 27 dist assets, and
successful browser/server cleanup. This is finite DDS8 evidence, not MSP/JXR,
a broader thirteen-profile all-consumer baseline or a performance guarantee.

The first actual Flatten attempt passes the single filtered-layer Image-menu
case: independent source/pixels, whole-canvas Source6 binding and close ordering,
metadata-preserving minimal PCND2, Undo/Redo and fresh reopen. It then fails the
multilayer Command+Shift+F case because Web shortcut dispatch used the Classic
frontend. This first attempt is a **partial result**, not a passing suite: receipt
`docs/validation/noise-flatten-browser/first-attempt-closeout.json`, SHA256
`3fc19deb195c960605f2526e0ba2838f5f0e9d07c6a7800882831543e73ffcf3`.
A narrow two-path shortcut fix has two passing tests. The fresh production build
passes with all 3,326 inputs unchanged and 53 artifacts, including 27 dist assets:
`docs/validation/noise-shortcut-production/attempt-1/final.json`, SHA256
`a9af8616306d35d985fb9414bc6ce4fca6b696d763b561e533c464ae21ccb7fd`.
The corrected producer subsequently passes six small actual Chrome cases:
single-filtered-layer Image-menu flattening, multilayer Command+Shift+F, genuine
held-result Escape/source-owner/private-draft fences and a 128×64 diagnostic.
Each source is compared in full against its independently parsed fixture before
Main authority is accepted, including hidden pixels, filter IDs and source
metadata. Positives check exact whole-canvas RGBA, metadata-preserving minimal
PCND2, single Undo/Redo and fresh reopen. Cancelled/stale replies require real
retirement and source-close ACKs and cannot publish a flatten history entry.

A separately authorized 2048×2048/MAX_PIXELS run passes the complete 16 MiB raw
Worker reply and saved Main raster SHA against two streaming independent
references, plus five literal probes. It uses active Uniform Noise10/color with
seed65535 and no Box, a 3,408-byte implicit-default source and a nonuniform output
that requires real raster tiles. Canvas and limits are unchanged. The first
boundary attempt's observer incorrectly rejected a legitimate 1024²/level1 fit
BASE; that error is preserved separately from product refusal. The corrected
observer admits only that exact fit shape and the exact native 2048²/level0 action.

This instrumented run observes 136.6 ms OPEN-to-raw, 7.7 ms raw-to-close-ACK and
91.4 ms close-ACK-to-first-rAF-observed-dirty. An actual Main long task lasts
84 ms within the publication window. Save is measured separately at 633 ms.
These are service and observation intervals, not isolated kernel or exact
synchronous-ingest CPU durations; observer hash snapshots can affect timing.
Main ingestion/hash/history remain synchronous. No 50 ms, cancellation-latency,
RSS or general performance guarantee follows, nor a Gaussian or arbitrary-stack
4MP result.

Aggregate `docs/validation/noise-flatten-browser/browser-final-freeze.json`, SHA256
`396a03282245f02640b09cf83a84760890aef3314f88a7ad27a81f9a97da0d94`,
binds both successful original sessions, both retained first failures and the
current 3,326 producer inputs, 53 artifacts and 27 dist assets without byte drift.
Browser/server cleanup passes. The broad cross-frontend baseline remains twelve
while Noise's specific routes retain their separately recorded qualification.
