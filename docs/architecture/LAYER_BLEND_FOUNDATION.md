# Layer blend foundation

Multiply and Screen are being integrated as stored, editable layer properties.
The current slices cover numerical behavior, typed editing, persistence, history
and scoped serial CPU and GPU composition. Native Studio now exposes staged blend
controls; browser blend execution remains gated. The native selector checkpoint
below records its separate qualification scope.

## Numerical contract

`paintcraft_core::types::LayerBlendMode` defines Normal, Multiply and Screen.
Normal calls the existing source-over implementation. New modes use checked-range
integer arithmetic over straight encoded RGBA8, preserving the exact backdrop
when source alpha is zero. Independent rational fixtures cover translucent
backdrops, hidden RGB, extrema and rounding. This establishes the numerical
foundation, not application or proprietary-renderer parity.

The eventual compositor applies filters, the own-layer mask and layer opacity
before ordered layer blending. Native composition precedes composite LOD
reduction; display premultiplication follows it. GPU and browser execution need
separate qualification of this sequence.

## Stored intent

Layer metadata, compact staging and hot/cold history retain the mode independently
of visibility and opacity. Typed `layer.set_blend_mode` belongs to the closed
`paintcraft.batch/layer-appearance-v2` profile. Clear Canvas restores Normal.

Native PCND8 records store the mode and declare `layer-blend-modes.v1`.
All-Normal documents retain their previous minimal PCND encoding; removing the
last non-Normal mode returns to that branch. Unknown tags and nonminimal
all-Normal PCND8 streams refuse. Layers, filters, masks and source metadata remain
part of the same persistent document.

Actual engine receipts disclose stored modes through result/v6 and CLI
file-command/v6. These schemas do not expand older frozen discovery bundles.
Older inspection, MCP and foreign-copy routes remain closed to unsupported modes.

## Local evidence and limits

The metadata integration records are under
`docs/validation/layer-blend-metadata-integration/`. Independent PCND fixtures,
real Session receipts, malformed-schema controls and original failed attempts are
retained separately. Receipt qualification covers 72 real results and 72 actual
CLI envelopes across 36 feature combinations; schema constraints and the stricter
producer sorting contract are checked separately.

The same-target before/after probes observe no size change in the measured native
Layer/Document/SavedLayer/Packed or CPU/GPU cache controls. A real Wasm probe also
observes unchanged public Layer/Document sizes and the fresh-decoder retained quote
for its finite two-pixel specimen. These are actual layout observations, not
universal memory, RSS, latency or private Wasm-layout claims.

Rendering and frontend adoption remain separate gates. Unsupported consumers
must reject stored modes before installing a replacement document or producing a
visual copy. No provisional blend execution price or higher resource cap is
activated by this metadata slice. Full Palette blend/style parity remains open.

## Scoped CPU composition

Serial native rendering now evaluates Multiply and Screen with the existing
caller workspace. The fixed opaque-backdrop reduction is exact; translucent
backdrops retain the general integer implementation. Filters precede the own-layer
mask, then opacity and ordered blending; complete native composition precedes LOD.
Normal numerical kernels remain unchanged.

New modeled work uses 32 extra source-action units per proved-opaque sample and
256 for the general call, plus bounded context setup. These are policy units,
not instruction counts or measured timings. No new counter, pool, cap or mask-query
revision is introduced. Captured base/Noise ownership remains paired. Public nested
requests recheck their actual ROI and remaining budget; cache misses and dirty
patches stay inside the requested footprint.

Local gates pass 262 Render tests across 17 targets, including 54 required-adapter
GPU tests on Apple M1 Pro Metal. The new GPU test proves refusal, not blend arithmetic
parity. Independent literal checks, true partial dirty patches, exact-budget calls,
cancellation, nested reentry and native/Wasm strict checks pass. Original compile
and test-fixture failures remain under
`docs/validation/layer-blend-cpu-integration/`. Active control sizes are observed on
the native target; private Wasm layouts remain unclaimed. The separate
[CPU measurement record](../performance/CPU_LAYER_BLENDS.md) reports the finite
host workloads, raw evidence and unresolved comparisons.

## Scoped GPU composition

The native GPU compositor now evaluates Multiply and Screen through a separate
private admission scope. CPU admission cannot authorize a GPU request. Each entry
checks the actual region and remaining caller budget, retaining the same captured
base/Noise pair. Integer WGSL implements the opaque and general formulas with
unchanged Normal arithmetic. The two mode controls occupy existing uniform padding;
layout assertions retain the 32-byte structure and verify its little-endian bytes.

Filters and the own-layer mask are evaluated before shader opacity and blending.
Composition precedes LOD reduction and display premultiplication. Mode-only edits
reuse eligible source uploads while recomposing the output. Spatially filtered
sources retain their existing cache exclusion; this is not an all-source upload
reuse claim.

The first serial qualification attempt passes 145 renderer unit tests and all 62
required-adapter GPU tests on Apple M1 Pro Metal, including eight new numerical,
ordering, cache and admission groups. Independent literal vectors cover straight
and display output, mask/opacity ordering, Box Blur prefixes and partial-edge LOD.
Native GPU all-target and actual Main/both-Worker Wasm strict lint checks pass,
along with formatting and source-length checks. The source hashes and complete
eight-step gate are retained under
`docs/validation/layer-blend-gpu-integration/final-freeze.json`.

A later full renderer regression passes 270 tests across 17 nonempty targets with
the adapter required. Its first sandboxed attempt passes 134 unit tests but fails
11 adapter-dependent tests because Metal cannot be enumerated there; the same
source passes outside that restriction. Both attempts remain recorded separately
under the same integration directory.

This is native Metal execution evidence; Wasm lint does not establish browser GPU
execution, other GPU adapters, runtime performance or RSS. At this GPU checkpoint,
browser adoption, native blend controls and foreign-copy consent were still open.
The later native selector checkpoint does not relax browser guards or closed
clipboard/foreign consumers.

## Native host admission prerequisite

Stored non-Normal modes now select the desktop's existing workspace policy,
including hidden and zero-opacity intentions. The synchronous helper captures an
installed caller lazily instead of replacing its scope and work counter. Stored
Noise keeps priority and retains its paired base/Noise admission.

Fifteen selected native workspace tests pass, including actual CPU and required
Metal rendering through the synchronous helper, exact 544-unit parent exhaustion,
unchanged source/cache on denial and disabled-Noise pair continuity. The first
source-only proposal missed the helper's pre-existing counter reset; independent
review caught it before execution, and the amended candidate tests the actual
consumer helper. Candidate revisions and execution logs remain under
`docs/validation/layer-blend-native-admission-integration/` and its adjacent
candidate directory. This prerequisite does not remove open/install/recovery
guards or activate blend controls, clipboard or foreign exports.

## Native document viewing and recovery

Native Classic and Studio now open, install, save and recover typed PCND8
documents. Studio opens a new owner while retaining prior tabs and history;
Classic retains its existing explicit recovery discard decision. Pending recovery
claims keep the original checkpoint until a new durable checkpoint protects the
recovered document. Stale completion and full Studio tab capacity preserve the
current owner and recoverable source.

Actual host tests cover synchronous/background CPU display, Metal texture
readback, a genuine GPU storage-limit fallback and later GPU ROI recovery. Image
sampling uses the composed result; layer sampling remains raw. Flatten records one
Undo and restores modes, filters, masks and source pixels on Undo/Redo. Document
and isolated-layer thumbnail jobs and print PNG preparation are exercised without
invoking a physical printer. Failed rendering keeps the established explicit
clear/error presentation rather than publishing partial or stale pixels.

Real Studio filter controls also pass eight combinations of synchronous/background
execution, Multiply/Screen and editing either the top layer or opaque-proof root.
Independent literals verify Preview, Cancel, Apply, one-step Undo/Redo and PCND8
save/reopen. Filtering the proof root recomputes its classification instead of
reusing the cheaper opaque estimate. Source raster and private-preview history
remain unchanged until Apply.

The full native library suite passes 805 tests with 29 existing ignored tests and
the GPU adapter required. Native all-target strict lint, formatting, source caps
and independent fixture checks pass. The first full run retained one obsolete
recovery-refusal failure; its replacement tests actual consent, tab ownership and
checkpoint handoff. Original failures, amendments and actual gate logs are under
`docs/validation/layer-blend-native-view-integration/`.

This viewing checkpoint admits native document viewing and existing editing
consumers. The later native selector checkpoint is separate. Browser PCND8
execution, foreign-bake consent and the old visible-clipboard route remain gated.

## Browser metadata groundwork

Separate Source8 metadata now defines registry contract 4, both mask and blend
registrations, framed admission origins, numerical counts, coverage ABI and the
ordered layer-mode identity. Eleven independent byte-frame scenarios verify the
new SHA domains. Zero own masks are valid metadata for a blend source; they do
not relax Source7's own-mask requirement. Metadata construction and hash binding
are not source authentication, decoded-storage attestation or execution grants.

Local gates pass 632 Formats library tests with 12 existing ignored tests and all
97 render-Worker library tests, plus native workspace all-target and Main/both-
Worker Wasm strict lint, formatting and source-length checks. The first Worker
run caught a new test's incorrect assumption about Source5's existing error code;
the corrected test verifies its exact registry refusal and a valid positive
control without changing the old endpoint. Sources and failures are retained in
`docs/validation/layer-blend-source8-metadata-integration/`.

Existing Source5/6/7 endpoints remain closed to registry 4 and blend execution.
This groundwork does not add a Worker8 endpoint, admitted portable PCND8 source,
CPU preparation service or Main consumer. Those still need their own ownership,
work-accounting and actual browser qualification.

## Native Studio layer-properties controls

Native Studio's Layer Properties dialog now stages Normal, Multiply and Screen
alongside name, visibility and opacity. Apply submits one atomic two-operation
batch through the existing activation FIFO, producing one Undo entry. An unchanged
Apply is a no-op. Cancel and Escape discard the draft without changing source
pixels or history; switching the layer, tab, document owner or publication retires
stale drafts rather than rebasing them.

The dialog waits for earlier accepted edits and text to resolve before capturing
its values. Acceptance and queue-head checks retain the actual owner, state,
publication epoch and target. Native activation retirement still reaches the
existing prepared-publication guard. Closing the last Studio tab disables the
real F4/menu action, and the private dispatch/dialog handlers also refuse the
placeholder document. The existing Classic/browser presentation entry remains
unchanged and exposes no blend selector. Seven locales include the new controls.

The full native library suite passes **817 tests with 29 existing ignored tests**
with the GPU adapter required. Native all-target strict lint and actual Main/both-
Worker Wasm library strict lint pass, as do formatting, translation coverage
(1,478 messages across seven locales), source-length limits and whitespace checks.
The first lint attempt failed writing compiler metadata because the disk filled;
the remaining-only retry passes without repeating the successful native suite.
Earlier harness-frame and test-budget corrections remain recorded separately.

An actual Apple M1 Pro Metal offscreen diagnostic renders eight English/German
mode and popup images. Four representative images were visually reviewed for
readable choices, the long German Screen label and complete Apply/Cancel controls.
This is modal-layout evidence at 900 × 850, not whole-application visual parity.
The temporary diagnostic source was restored byte-for-byte before the full suite.
All 17 product inputs match before and after qualification. Original failures,
source hashes, terminal results and visual artifacts are retained under
`docs/validation/layer-blend-native-controls-integration/` and
`docs/validation/native-controls-ui/`.

Actual before/after private native64 layout probes observe `QueuedEdit` and its
background queue slot growing from 2,304 to 2,368 bytes, and `PropertiesDraft`
(including its optional representation) from 88 to 152 bytes: **+64 bytes** in
each measured case. `ToolsAction` remains 136 bytes. These are unpriced native
queue/modal bookkeeping bytes, not newly reserved capacity, a Wasm/public-ABI
claim or a full native memory bound. The generic activation FIFO and retained
vector capacities are not capped by its 4,096 remembered-token history. Layer
names are limited to 1,024 characters when applying, which does not bound draft
String capacity. The existing preparation estimate and Worker workspace do not
account for this host bookkeeping. The finite audit and layout receipts remain
under `docs/validation/layer-blend-native-controls-bookkeeping/` and
`docs/validation/layer-blend-native-controls-layout-probe/`.

This selector does not activate browser blend execution, asynchronous browser mask
editing, appearance-aware foreign-copy consent or the closed visible-clipboard
route. Those consumers require separate source ownership, admission and actual
runtime qualification. Full Palette blend/style parity remains open.

## Additional portable foundations

A separate admitted portable Source8 snapshot writer now preserves Multiply/Screen
intent and stored own masks. Root-ordered layer preparation avoids full layer-table
bucket scans; a pure advisory scratch quote requires a separately prepaid walk.
Independent wire fixtures cover accepted and malformed profiles, explicit-default
elision and unchanged old7 refusal boundaries. Native format tests pass. A separate
actual Node Wasm snapshot/probe run also passes 39 finite cases: 19 independent
wire literals and 20 storage, admission, cancellation and typed-token controls.
This result is separate from the 57-case service/style/History matrix below.

The caller-owned prepared CPU blend service now checks the actual original scope,
cumulative Base/Noise debits, ROI and resolved metadata minima before callbacks.
Distinct prepared owners retain both complete storage charges even with equal
keys. Private gateway validation grants no ambient or public bypass. A cooperative
History fork copies a borrowed source in bounded entry prefixes with original
admission, scan/clone/probe accounting and storage-before-account destruction.
It does not own a browser source lease or activate mask mutation.

Native suites pass Core 103, Document 45, Formats 642 (12 existing ignored),
History 48 and Editor 51 tests, plus 288 render tests across 17 nonempty targets
with the required GPU adapter. The initial new default-elision test used an old
inspector that correctly refused Source8; its correction uses the separate probe,
and the failed attempt remains preserved. Strict checks also pass after moving
an unchanged Noise getter before test items.

Actual Node Wasm execution through real Cargo-produced libraries and generated
bindgen glue passes 57 finite cases: 20 CPU-service authority/counter controls,
eight independent mixed numerical cases, 19 standalone style-vocabulary cases
and ten borrowed cooperative History cases. The scope includes the exact
17,696-unit no-filter allowance and one-unit denial, paid-prefix refusal,
original-owner minima, retained-owner union, cancellation and source guards.
Source/config/harness hashes match before and after execution. The actual module
remains nonshared at 2 MiB, with a 16 MiB maximum and only the real glue's externref
initializer import. This is actual portable primitive/service evidence, not
browser UI, Worker8/Main activation, GPU-Wasm, owned History source-lease,
full reader heap/RSS or latency qualification.

Native receipts are under `docs/validation/palette-next-foundations-integration/`;
the actual Wasm receipt is
`docs/validation/layer-blend-candidates/source8-cpu-service-wasm-history/node-attempt-1/final.json`.
The separate snapshot receipt is
`docs/validation/layer-blend-candidates/source8-snapshot-wasm/node-attempt-1/final.json`,
with its immutable `build-attempt-1/build-final.json` and archived actual sources
and libraries. Source hashes match before/after build and execution; real bindgen
glue runs the nonshared 2 MiB module with a 16 MiB maximum. Controls exercise
same-buffer authenticated-witness reuse, general-reader versus structural-probe
validity, default elision, old7 closure, paid-prefix/original-headroom refusal,
source preservation and cancellation. A clipped live A8 view projects canonical
meaning but refuses adoption above its wire-retention bound; actual fresh decoded
storage passes that bound.

This snapshot result does not price complete general-reader work or heap.
Authentication policy and helper callbacks share the declared cumulative counter;
the general reader exposes cancellation and its existing coverage/metadata and
staged-pixel admission, with decoded-tile storage observed in its separate default
realm. Returned metadata controls and buffers retain their original accounts until
destruction. Diagnostic JSON/reference clones and inherited metadata allocation
are excluded from a full heap bound. One explicit authentication and witness reuse
are observed; the private test-only hash-pass counter is not instrumented. No
browser/Worker8/Main execution, GPU, RSS or latency result follows.

The standalone style vocabulary adds no `Layer.styles` field or stored/rendered
Color Overlay semantics. Browser mask editing, Worker8/Main blend execution and
appearance-aware foreign/clipboard consent still require their own integration.
