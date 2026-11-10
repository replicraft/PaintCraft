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

### Unbound metadata forecasts

Separate pure program/call forecasts now compute canonical-key metadata debt
without constructing a Caller, execution root, prepared tables or renderer
context. Any actual Base or Noise TLS `Some` refuses before callbacks/helpers,
including zero-work or matching owners; callback boundaries recheck absence.
Every stored Noise descriptor, including disabled/default policies, explicitly
requires later paired admission. Read-only getters observe the actual installed
Noise owner and newly allocated prepared retention; neither creates authority,
repairs a mismatched pair, or grants bytes/work from a quote.

Eight new native groups and 16 existing service groups pass, with native/workspace
and Main/both-Worker Wasm strict checks, formatting and source-length gates.
Actual Node Wasm execution passes 65 finite controls: the unchanged original 57
service/style/borrowed-History cases plus eight forecast/ownership cases. The
original failed native counter-type compilation and its explicit `u64` test-only
correction remain preserved. Immutable receipts are
`docs/validation/layer-blend-candidates/source8-worker8-d0/native-attempt-2/final.json`
and `docs/validation/layer-blend-candidates/source8-worker8-d0-wasm/node-attempt-1/final.json`.
Actual source hashes match before/after the Wasm build and execution. These
forecasts price their declared metadata helpers, not all inherited renderer
layer-table traversal. The separate rooted adapters below address that traversal
boundary; D1 and Worker8/Main service/transport activation remain separate gates.

At the D0 checkpoint, its source closure also passes a fresh 39-case portable snapshot
run, separately from the 65 service/style/History/forecast cases. Its immutable
receipts are `docs/validation/layer-blend-candidates/source8-snapshot-wasm/build-attempt-2/build-final.json`
and `docs/validation/layer-blend-candidates/source8-snapshot-wasm/node-attempt-2/final.json`;
source hashes remain unchanged. The original attempt-1 evidence and general-reader
work/heap exclusions above remain intact.

### Owned immutable History source

The opt-in History read-source primitive now moves an existing timeline into an
immutable shared carrier, retaining actual Vec/Memo/Error storage credit with it.
Capture performs O(1) backing/control assessment and returns the exact original
owner on refusal. Owned cooperative forks keep that source through real Future
Pending/cancellation/drop, and opaque handoff retires unused reservation factories
while assigned storage remains funded. Already-funded backing and new carrier
controls may retain their distinct original realms without fallback or double charge.

Native History 63 and Editor 51 tests pass with current strict checks. The native
private tests cover retained table/error capacities, pinned Arc layout and causal
last-owner/account order; their native-only concurrency case is not Wasm evidence.
A separate actual Node Wasm run passes 77 finite cases: the previous 65 plus 12
public-API owned-source/future/factory controls using independently fixed limits.
Its immutable receipt is
`docs/validation/layer-blend-candidates/history-read-source-l1-wasm-effective/node-attempt-1/final.json`.
Source hashes match before/after execution. The external Wasm harness does not
claim execution of private layout/tombstone hooks or native concurrency. Session
holders, funded mutation/publication, asynchronous retirement, persistent broker
credit transfer and browser mask finalization remain separate integration gates.

### Rooted rendering adapters

Separate Source8 rooted adapters validate root identity/uniqueness and descriptor
requirements without full layer-table bucket scans. Quotes prepay a distinct
2,048-unit/root quote pass in addition to context proof and canonical-key capture.
Actual calls prepay their own facts construction and each prepared/spatial/ordered
composition pass at its helper boundary. A private immutable witness is charged
before retention on the same original scope: 24 bytes on native64 and 16 on Wasm32.
The actual Base/Noise byte allowance tightens by that charge; witness and temporary
bindings retire before credit. Existing public routes and their admission remain
unchanged; no TLS bypass marker or new execution root is installed.

Seven new native groups, 16 existing service groups and strict checks pass; expanded
missing-root/value-identity cases also pass. Actual Node Wasm execution passes 84
finite cases: the previous 77 plus seven public rooted controls. These include
independently fixed 33,522-unit exact/one-unit-short work, context-plus-witness byte
boundaries, paid quote prefixes, sparse/tombstoned versus compact layer-map traces,
known Noise pixels and source guards. The initial malformed test guard incorrectly
indexed an intentionally missing root; its source-only correction uses full field/
map equality, with original candidate revisions preserved before execution.
The immutable receipt is
`docs/validation/layer-blend-candidates/source8-worker8-rooted-wasm-effective/node-attempt-1/final.json`.
Actual source hashes remain unchanged. The module starts at 2 MiB and finishes at
3,407,872 bytes within its unchanged nonshared 16 MiB maximum. These are finite
portable adapter checks, not Worker8 service/transport activation or a latency,
RSS or full inherited raster-policy proof. D1 still needs its actual construction,
callback multiplicities, authenticated adoption and workspace-window ownership.

The separate portable snapshot matrix also passes all 39 cases against this
rooted source closure. Its immutable receipt is
`docs/validation/layer-blend-candidates/source8-snapshot-wasm/node-attempt-3/final.json`
(build `7a655a73911814f0bcccc6c31e9031482daf2f2d7ab3a4c816731501fda35bff`).
The same general-reader heap/work and browser-activation exclusions apply.

Additional branch controls extend the actual Wasm matrix to 90 cases while
preserving the earlier 84 assertions. Independent pixels cover nonuniform Box Blur
edges and a cropped region, Vibrance with an own mask, and Blur followed by Invert
and an own mask. Cancellation and one-unit-short facts admission retain the paid
prefix on the same counter before retry. Two native groups and renderer strict
checks also pass. The immutable receipt is
`docs/validation/layer-blend-candidates/rooted-branch-wasm-effective-public/node-attempt-2/final.json`.
Diagnostic Option conversions and public caller capture were corrected; initial
compilation failures and original literals remain archived. These additions change
tests only and do not activate browser consumers.

### Rooted Normal portable snapshots

A separate admitted writer now covers Normal-only PCND1–7 with root-ordered
identity/uniqueness, stored blend eligibility, filter catalog and coverage passes.
It preserves portable bytes/default elision, disabled descriptors and masks,
metadata and codec choice. Hidden or zero-opacity nonNormal layers are refused.
Its structural root/get policy prices each lookup walk explicitly; it does not
claim physical HashMap or inherited raster-iterator probe-time bounds. Existing
writer bodies and callers remain unchanged.

The actual native Formats run passes 652 tests (12 existing tests ignored), and
strict workspace/Main/Worker checks pass. The separate actual Node Wasm matrix
passes 75 finite cases: preserved 39 Source8 snapshot assertions plus 20 independent
Normal profile1–7 wire literals and 16 admission, debt/retry, cancellation, strict
eligibility and retained/tombstoned-map controls. Inputs and distinct encoded
outputs are authenticated; readers use fixed explicit metadata/coverage admission.
Returned scratch quotes never establish grants. This is not a full general-reader
heap/work, allocator/RSS, timing, browser UI, Worker8 or persistent Main-credit claim.

Immutable evidence:
`docs/validation/layer-blend-candidates/normal-rooted-snapshot-wasm/node-attempt-1/final.json`
(SHA256 `29d2057b303119b1907e9aa843defbbda1bf0b73da185d71990204b18b41c9ac`,
sourceUnchanged=true); actual build receipt SHA256
`03dabca518c53824ee49317fa29227f3de16b913d9392e33c77922e59f046c12`.
Initial native fixture failures are preserved. Two test-only corrections added
explicit coverage admission to four fixture reads, then corrected the canvas-pixel
bound to 1024 for the separate 257×3 native codec fixture. Both exact deltas are in
`docs/validation/layer-blend-candidates/normal-rooted-snapshot/root-corrections/`;
no product behavior, independent wire or expected work was changed. At that
checkpoint, count/fixed wrappers and browser credit integration remained separate
prerequisites; the later wrapper qualification is recorded below.

### Explicit reserved decode factories

A restricted Raster decoder admission now takes the original decoded reservation
and every factory explicitly. Its gauge and control declarations remain attached
to surviving tiles after the decoder is dropped. Cancellation and failed writes
retain that ownership; new tiles acquire their control owner before insertion.
Existing Admission constructors retain their behavior. The private Allocation
tail changes Tile size, and metadata weight follows that actual size.

Eight focused native tests, the backend suites, 305 renderer tests with required
Metal, and 820 desktop tests pass. The desktop suite retains 29 existing ignores;
Formats retains 12. Native/workspace and Main/Worker Wasm strict checks pass. Nine
actual Node Wasm public-API controls pass, using independently fixed 64/63-byte
control boundaries. Native-only factory inspection and concurrency remain separate
evidence. The immutable runtime receipt is
`docs/validation/layer-blend-candidates/source8-raster-decode-factories-wasm/node-attempt-1/final.json`.
The 75-case portable snapshot matrix also passes again against this Raster change
at `docs/validation/layer-blend-candidates/normal-rooted-snapshot-wasm/node-attempt-2/final.json`.

This constructor funds its fixed gauge/control allocations and decoded pixels.
Containing Tile headers, index storage and the complete reader source lifetime
still require their own census and ownership declarations. This does not establish
full reader heap admission, Worker8 activation, persistent Main credit or latency.

### Counted fixed-size Normal snapshots

The Normal-only rooted writer now has an additive count/fixed-output API. A count
fact borrows the original immutable document, scope and callbacks; encoding accepts
no replacements. The full container count includes its SHA trailer and refuses
above 64 MiB. Encoding prepays buffer initialization, retains the output declaration
with its bytes, then runs the complete writer again on the same work ledger.
Partial failures retain paid work, and callers can retry without replacing that
ledger. There is no public detached Vec or account escape.

The native Formats suite passes 675 tests with 12 existing ignores. Documentation
checks include three compile-fail ownership examples. Strict workspace and
Main/bothWorker Wasm linting, formatting and source length checks pass. Twenty
independent wire literals, portable codec parity, boundary, cancellation and
retained-debt controls cover the new API. The first strict run caught missing
Debug implementations; bounded manual implementations corrected only formatting
behavior, with the failed attempt and preimages preserved.

The immutable native receipt is
`docs/validation/browser-mask-next/source-count-prerequisite/shared-formats-counted-candidate/native-attempt-2/final.json`,
with source hashes in `qualified-freeze.json` beside that attempt.

Actual Node Wasm execution also passes 108 finite cases: the previous 75 snapshot
checks, 20 independent counted/fixed wire cases and 13 public ownership, refusal,
cancellation and retry controls. The original scopes and cumulative work counters
remain in use. Independently fixed Wasm control sizes are 56/40/40/40 bytes; no
returned quote creates a grant. Output ownership survives dropping its count fact
and original scope. The module ends at 3,407,872 bytes under the unchanged 16 MiB
maximum. The immutable runtime receipt is
`docs/validation/layer-blend-candidates/shared-formats-counted-wasm/node-attempt-2/final.json`;
its build receipt SHA256 is
`7315be5b6ab4c7353a7207c5ca7548e709db71c7a3f4d055e6eb4ce400f8cb58`.

The first runtime attempt is preserved. Its generic peak assertion confused
admitted storage with scope demand, which includes refused pending requests.
The test-only amendment asserts exact rejected demand at the three one-byte-short
boundaries, retains the other ceilings, and checks that only the original fact
remains live after the blocker and retry output retire. No product code, grant,
work expectation or wire fixture changed. The two frozen amendments and root
promotion receipt are retained alongside the original evidence.

This API does not establish full reader heap
admission, physical raster-table scan bounds, persistent Main credit, Worker8
activation, allocator/RSS bounds or latency improvements.

### Retained raster table bounds

Raster tile indices now retain allocation and bucket provenance through every
structural mutation, including reserved decoding, paste, replay, capture and
compact cloning. A read-only O(1) quote reports retained allocation/bucket uppers,
live entries and the named allocation-plus-bucket scan allowance. Deletion and
allocated-empty cloning preserve the retained extent; successful nontrivial
reserve reconciles the actual pinned deletion-free capacity. Occupied replacement
avoids std HashMap insertion's implicit reserve-before-lookup behavior. There is
no raw mutable map or Entry escape and no new dependency.

The tracked controls change RasterLayer's inline size to 80 bytes on native64
and 64 on Wasm32. Layer and its map entries consequently grow; Document, Tile
and packed SavedLayer inline sizes do not change from this wrapper alone. The
legacy metadata weight follows the actual RasterLayer size but remains a logical
diagnostic. The new quote creates no budget, source lease or allocation authority.

Eight focused groups pass, followed by Document 45, Editor 51, Formats 675,
History 63 and Raster 128 native tests, 305 renderer tests with required GPU,
and 820 desktop tests. Formats retains 12 existing ignores and desktop 29.
Workspace/Main/Worker strict checks, formatting and source limits pass. Actual
Node Wasm passes 117 finite cases: all previous 108 assertions and nine public
tracker transitions. The module ends at 3,473,408 bytes under its unchanged
16 MiB maximum. The immutable runtime receipt is
`docs/validation/layer-blend-candidates/source8-raster-table-wasm/node-attempt-1/final.json`;
its build receipt SHA256 is
`9672e49aa8a6c15d3a91729b41d8039d6648eb50224527bd938ef0fbc9643f78`.

The native compile failure from one unmigrated private fixture and the subsequent
fixture reservation failure are preserved. Corrections route that fixture through
tracked insertion and reserve four simultaneously held tiles upfront within its
unchanged pixel ceiling. The Wasm harness also preserves a pre-execution metric
slot correction, separating clone quotes from demand peaks. Product grants,
wire/pixel expectations and old writer callback prices remain unchanged.

History registration and compact metadata cloning still need their separately
reviewed scan and ownership supplements. Rooted writer work is amended below.
This tracker does not
retroactively fund old storage, establish physical probe/allocator timing or RSS,
or activate browser mask/blend consumers.

### Borrowed portable Source8 plans

Authenticated container8 inputs now expose a separate borrowed structural plan
for portable block encodings 1/2/4/5. Two metered passes validate framing, catalog
and filter intentions, mask structure, unique layer/block identities and bounded
record counts. The plan retains exact 56/24-byte layer/block records under its
original scope. Cursors borrow the same authenticated source and retain its
original check/spend callbacks, with a shared cumulative work ceiling. No caller
can construct a plan from a raw slice or substitute helper callbacks or scopes.

The Formats native suite passes 685 tests with 12 existing ignores. Six
compile-fail documentation checks pass, including three new plan ownership
examples. Thirty independent wire cases and ten test groups cover cursor replay,
limits, cancellation, paid retries, original scope refusal and bounded Debug.
Workspace and Main/Worker strict checks, formatting and source limits pass.
The native receipt is
`docs/validation/layer-blend-candidates/source8-portable-plan-r1/native-attempt-1/final.json`;
`qualified-freeze.json` records all nine effective source hashes and the bounded
Debug amendment.

Actual Node Wasm passes 161 finite cases: the previous 117 unchanged cases plus
30 independent structural wires and 14 public plan controls. Independently fixed
Wasm declarations of 1,528 bytes for the plan, 7,260 for scan controls and 56/24
for records match the actual module. Exact 8,924-byte scope admission and its
one-byte-short refusal preserve original scope and work; no quote creates a grant.
The module ends at 3,473,408 bytes under its unchanged 16 MiB maximum, and source
hashes remain unchanged. The immutable runtime receipt is
`docs/validation/layer-blend-candidates/source8-portable-plan-r1-wasm/node-attempt-1/final.json`;
its build receipt SHA256 is
`43b1a055f45ec0c192d2ea6ae98905b31b45847b74fcd9266ca11ac8c72cced2`.

This is structural planning, not pixel or ICC validation. It creates no decoded
Document, root, reservation, materializer or publication authority. Native Zstd
remains supported by the separate general reader; this portable transport refuses
its code3 before decoder initialization. Original-realm materializers, complete
source/index/header ownership and browser activation remain separate work.

### Rooted snapshot collector work amendment

Rooted Normal and Source8 writers now explicitly prepay their retained tile-table
scan, copied tuple/strong-handle lifecycle and a conservative heapsort move upper
before collection begins. The scalar quote costs 64 work units. A fresh one-tile
table adds 210 units in total, a retained sparse one-tile table 938, and a fresh
two-tile table 452. Zero-entry tables pay only 64 and skip iterator/index creation,
including allocated-empty tables. These are declared structural policy units,
not measured CPU timings. Both complete counted passes pay the supplement; output
initialization remains a separate debit. Original counters/scopes and wire bytes
are preserved. Ordinary writer, block collector and sort bodies stay unchanged.

Formats passes 691 native tests with 12 existing ignores, six documentation
checks and strict workspace/Main/Worker checks. Actual Node Wasm passes 173
finite cases, including twelve new public scan, wire-parity, scope and paid-retry
controls. This explicitly requalifies the changed rooted callback prices rather
than relabeling earlier receipts. The module ends at 3,473,408 bytes under the
unchanged 16 MiB maximum, with source hashes unchanged. The runtime receipt is
`docs/validation/layer-blend-candidates/source8-rooted-collector-wasm/node-attempt-1/final.json`;
the build receipt SHA256 is
`bd53101e5b22025cb5284362ce1d5b1976d1ec2a1e70583b3e16ddd197668454`.

The preserved initial failures required only a missing import and a type alias
in the new test helper. Trailer-cancellation controls explicitly distinguish the
new 64-unit scalar debit from the later SHA trailer debit, keeping the same paid
work counter. Compact staging, history registration, D1 forecasts and complete
reader ownership remain separate gates. No physical hash-probe, allocator/RSS,
latency or browser activation claim follows from this work amendment.

### Original-reservation metadata and fresh decode indices

Core now exposes the defining full lease-control Arc allocation bound. Document
adds a strict borrowed source-metadata recipe whose materializer captures the
original ICC reservation, control scope, tail guard and callbacks once. Header,
descriptor, tag and digest validation finish before retained allocation. Both
canonical-header constructions, hashing, comparison, initialization and copying
have separate prepaid phases. Materialization uses that reservation directly,
requires exact Vec capacity and creates no second capacity lease. Resolution-only
attachments still retain their Inner and lease controls.

Raster also has a new-empty-only decode-index constructor. It checks an already
assigned metadata declaration before reserving the actual record count and
reconciles the tracked allocation. It accepts no existing table or caller-supplied
bucket bound. Existing constructors, fields, drops and general readers stay exact.
These primitives require the caller to retain the complete metadata/header union;
they do not independently provide the closed Source8 carrier.

Native validation passes Core 104, Document 55, Editor 51, Formats 691, History
63 and Raster 132 tests, with 12 existing Formats ignores. Eight documentation
checks and strict workspace/Main/Worker, formatting and source-limit checks pass.
Fourteen independent metadata wires include valid nonzero tags and invalid tag
extents. Pure layout, original-capacity refusal, paid callback errors/retries,
alias lifetime and fresh-index controls pass. The native receipt is
`docs/validation/layer-blend-candidates/source8-r2-core-icc-index/native-attempt-2/final.json`;
`qualified-freeze.json` binds all 13 effective source paths. The initial strict
failure and test-only lexical-scope correction remain preserved.

Actual Node Wasm qualification now passes 207 finite cases: the previous 173
plus 14 metadata wires and 20 public reservation, cancellation, paid retry,
alias-lifetime and fresh-index controls. The build uses the six actual Cargo
packages and generated wasm-bindgen glue. Source hashes stay unchanged, and
linear memory finishes at 3,473,408 bytes under the unchanged 16 MiB maximum.
The runtime receipt is
`docs/validation/layer-blend-candidates/source8-r2-core-icc-index-wasm/node-attempt-1/final.json`;
the build receipt SHA256 is
`cdbb0f61fa7368374942131f5e17e17590c7937f9f59c4bf73c3ba47b8d11caa`.
This is actual Wasm execution in Node, with no browser UI, Worker activation,
timing target or physical memory claim. Exact A8/pixel materializers, closed
source assembly and candidate/frame ownership remain separate work.

### Compact staging resource revision 2

Compact filter staging now prepays retained source-table scans, strong-handle
operations and local seed retirement, plus the existing generic executor's
clones, target vectors and intermediate retirement. Its seven-field quote has a
separate control declaration funded before quotation. Main preparation includes
the concurrent quote in its fixed peak. Worker state releases frames and staged
candidates before the immutable base. These changes keep the existing generic
executor and fixed budgets; the old Raster compact helper remains unchanged.

The shared appearance resource revision is now 2 in Rust and JavaScript, with
coordinated Source7/Source8 framing vectors. Old revision 1 is explicitly refused.
Other registration revisions and PCND wire versions stay unchanged. A fresh
one-tile Raster supplement is 330 work units, a retained sparse one-tile table
1058, two fresh entries 356, and an empty table 64. These are structural policy
units, not timing measurements. Direct independently retained outputs still need
their caller's final-owner cleanup policy; this is not a closed reader proof.

Native validation passes Editor 63, Formats 692, Raster 140 and Worker 100 tests,
with 12 existing Formats ignores. Five JavaScript framing tests, both independent
fixture generators, strict workspace/Main/Worker checks, formatting and source
limits pass. The receipt is
`docs/validation/browser-mask-next/source-raster-table-prerequisite/scan-supplements/compact-lifecycle-candidate/native-attempt-5/final.json`.
The adjacent `qualified-freeze.json` binds all 23 effective paths and correction
receipts. Failed attempts and preimages remain preserved: corrections address a
nested test-module path, an omitted one-entry retirement unit in test expectations,
fixture encoding/formatting and a cumulative-counter test that needed a captured
job rather than a standalone deferred quote. Production prices and fixed budgets
were not relaxed.

Actual Node Wasm execution now passes 232 finite cases: the preserved 207 plus
25 public compact resource2 controls. The build selects nine actual Cargo package
artifacts and Core's actual UUID dependency, then generates real wasm-bindgen
glue. All source hashes stay unchanged; final linear memory is 3,473,408 bytes
under the unchanged 16 MiB maximum. The runtime receipt is
`docs/validation/layer-blend-candidates/source8-compact-resource2-wasm/node-attempt-1/final.json`;
the build receipt SHA256 is
`5fee317a250958e7023e2fc863f7dce7fcae101c2a14d6e46030cc0370e1d90a`.
This separately qualifies the new prices, original counters/scopes, typed failures
and metadata semantics. It does not qualify arbitrary final output cleanup,
Worker/client or Main UI execution.

The unchanged production scripts also build Main, codec and render WebAssembly,
the Astro site and all asset checks at commit `82b87b3c`. The source stays unchanged
through all eight build/check steps, and 55 output files have recorded hashes in
`docs/validation/layer-blend-candidates/source8-compact-resource2-production/attempt-1/final.json`.
These are actual production artifacts, with browser execution still a separate gate.

Actual headless Chromium execution also passes against those preserved
`82b87b3c` production artifacts and their maintained Worker/client modules. It
retains the original 26 RGBA cases, two independently checked PNGs, 23 controls
and two twenty-generation view groups, then adds 14 RGBA checks, five resource2
controls and a three-generation prepare/cancel/render lifecycle. Old resource1
is refused before client source transfer. All source/artifact hashes stay exact,
and the page, browser and local server close. The receipt is
`docs/validation/source7-resource2-browser/browser-attempt-2/final-freeze.json`.

Both initial helper link failures remain preserved; immutable, byte-identical
hash-named library counterparts resolve dependency metadata without rebuilding
against later A8 code. The first browser failure was a new assertion comparing
JSON key insertion order; its explicit amendment compares every key/value pair
structurally while preserving all old assertions and literal outputs. This is
archived-production Worker/client qualification, not current A8 activation,
Main UI/History publication, OS printing/clipboard consent, physical memory or
latency qualification.

The follow-up full JavaScript unit suite passes all 350 tests, including mocked
Worker/client seams. Pinned Biome checks all 133 web files after a formatting-only
correction to the new revision-refusal test; CI exposed this check missing from
the earlier local gate. `biome-freeze.json` chains that correction to the native
freeze. These unit seams do not qualify execution against the actual Worker.

### Canonical source mask materialization

Raster now provides a consuming canonical A8 stream builder. It fills exact final
row/run storage directly, rejects noncanonical ordering and hashes the same bytes
as the existing generic mask path. Existing normalizers, storage fields and mask
semantics remain unchanged. Its fixed 4096-byte control declaration, exact retained
storage and caller-held full Storage Arc guard are separate obligations. Failures
consume the partial builder; the original work counter and typed error survive.

Formats binds coverage materialization to one authenticated portable Plan and its
original scope/guard. Item methods accept layer/filter indices, fetch actual views
internally and stream parsed values without intermediate parse vectors. Disabled
masks and disabled filter instances still materialize; present empty masks remain
distinct from absence. Repeated materialization creates separate retained storage,
and every actual callback updates the same Plan census without double billing.

Native validation passes 698 Formats and 148 Raster tests, with 12 existing
Formats ignores, plus eight documentation checks. Strict workspace/Main/Worker,
formatting and source limits pass. Five independent filter wires cover enabled,
disabled, empty, repeated own/filter storage and a disabled filter instance with
its nonempty mask. The receipt is
`docs/validation/layer-blend-candidates/source8-r2-canonical-a8/native-attempt-4/final.json`;
the adjacent `qualified-freeze.json` binds all 14 effective paths. Failed attempts
and corrections remain preserved: test helper visibility, cursor borrow lifetime,
an old fixture's filter-root index and an equivalent array-size spelling for lint.
No assertion, fixed grant, layout or work price was relaxed.

Actual Node Wasm execution now passes 303 finite cases: the preserved 232 plus
71 canonical A8 and bound-Plan controls. Both first build and runtime attempts
pass, with source hashes unchanged and final linear memory 3,473,408 bytes under
the unchanged 16 MiB maximum. The new cases check exact independent digests,
target layouts, original control/storage/header limits, all seven one-run callback
cuts and 24 debit cuts, paid retries, strict geometry/canonical failures, disabled
filter instances, repeated storage, census equality and final alias lifetimes.
The runtime receipt is
`docs/validation/layer-blend-candidates/source8-r2-canonical-a8-wasm/node-attempt-1/final.json`;
the build receipt SHA256 is
`7e3a69332fe0ad461383ec396b2bf8743ba193c9a910b630edf45bdb6aacac3f`.
The harness preserves its pre-build cursor-binding correction separately.

These are caller-guarded primitives, not a closed Source8 reader: pixel decoding,
complete source/candidate/frame ownership, Worker activation and final-owner
cleanup remain separate gates. No browser, GPU, physical memory or latency
qualification follows from this finite Node execution.

### Bound retained-source layout census

Formats now computes retained-source requests through one read-only pass bound
to the authenticated Plan, original metadata scope and paid counter. Each layer
header is parsed once, followed by every stored filter and own mask in wire order.
One remaining coverage limit spans the whole document. Existing parser/getter
bodies and Plan fields stay unchanged; the result exposes scalar diagnostics and
retains its Plan borrow rather than accepting caller-supplied records or views.

The census separately reports fresh metadata/index storage, every physical Tile
and lease-control header, every nonempty mask backing, ICC attachment headers,
retained mask/ICC policy requests and maximum actual decoder scratch. Disabled
and empty stored intent is counted; equal occurrences are not deduplicated.
Document inline size belongs once inside the later containing allocation, and a
zero-tile reader must skip creating decoder controls. Counts do not grant capacity
or prove the eventual source/candidate/frame lifetime.

Native validation passes 707 Formats tests with 12 existing ignores, nine
documentation checks, strict workspace/Main/Worker checks, formatting and source
limits. Twelve independent wires cover retained layouts and global limits; tests
check all 24 independently derived callback prefixes, original-scope refusal,
helper overlap, paid retries and recipe retirement. The native receipt is
`docs/validation/layer-blend-candidates/source8-retained-layout-census/native-attempt-1/final.json`;
the adjacent `qualified-freeze.json` binds all seven effective paths. Preserved
pre-build amendments fix full-container fixture hashing, move final arithmetic
after its prepaid check and strengthen every callback-prefix assertion. They do
not relax prices, limits or existing parser behavior.

The one-layer empty-layout pass costs 13,188 declared work units. Its named control
declaration is 1,040 bytes on Native64 and 936 on Wasm32; these are structural
policy declarations, not RSS or latency measurements.

Actual Node WebAssembly validation now passes 359 cases: the previous 303 plus
56 public census controls. These check all twelve retained layouts, all 24
callback prefixes, fixed debit refusals with cumulative retries, document-wide
coverage limits, original-scope control/helper overlap and account retirement.
The module starts with 2 MiB of nonshared memory, retains its 16 MiB maximum and
finishes at 3,473,408 bytes. Source hashes remain unchanged during execution.
The build and execution receipts are
`docs/validation/layer-blend-candidates/source8-retained-layout-census-wasm/build-attempt-2/build-final.json`
and the adjacent `node-attempt-1/final.json`. The first build remains preserved;
its fixture JSON error-conversion failure was corrected only in the test harness.
Pixel decoding, original-realm funding and closed Source8 assembly still need
separate validation before Worker or Main publication. This finite Node run does
not establish browser integration or performance.

### Bound portable pixel materialization

Raster now decodes Raw, RLE, LZ4 and RLE+LZ4 into a consuming fresh layer builder.
Formats selects those payloads through the authenticated Plan's recorded blocks;
callers cannot substitute a block view or replace its meter. The portable path
explicitly refuses native-only block encoding 3. Existing general import paths
and parser/getter bodies remain unchanged.

The builder checks complete planned tile/index metadata funding before decoding,
retains original tile reservations and scratch accounts, and rejects duplicate,
outside, extra or missing blocks. Edge blocks retain the full physical tile
reservation. Owned full-tile decoder buffers transfer without a second pixel
allocation; partial tiles initialize their final storage once. Both buffers and
credits survive until their actual ownership handoff. The first insertion prepays
retirement of the planned sparse index, including late failure with few filled
entries. Failed admission retains its charged work and staged-pixel debt.

Native validation passes 710 Formats tests with 12 existing ignores, 161 Raster
tests and 12 documentation checks. Workspace and Main/Worker Wasm strict Clippy,
formatting, source lengths and whitespace checks pass with unchanged source
hashes. Independent byte/event fixtures cover all four codecs, hidden RGB,
physical padding, maximum RLE scratch, original reservation boundaries, typed
cancellation, cumulative retries and large sparse-index cleanup. The effective
23-path freeze is
`docs/validation/layer-blend-candidates/source8-r2-portable-pixels/qualified-freeze.json`;
its native receipt is `native-attempt-3/final.json`. Earlier failed runs and their
test-borrow and conservative Entry-envelope corrections remain preserved. The
Entry declaration is a 40-byte control envelope, not a guaranteed standard-library
ABI; the fixed 4,096-byte control slot and original work fees remain unchanged.

These standalone primitives require the caller to retain complete metadata
funding through every returned alias. They do not yet close Source8 document,
candidate and frame ownership. Original-realm assembly and Worker/Main
integration remain pending; no speedup or complete memory bound is claimed by
this correctness baseline.

Actual Node WebAssembly execution now passes 469 cases, preserving the earlier
359 and adding 110 public pixel controls. These include independent physical-tile
byte hashes, all Raw/RLE callback and debit cuts, exact staged-pixel debt for each
of 59 failure/retry pairs, malformed payloads, original Q/M/scratch boundaries,
maximum RLE scratch, sparse-index retirement and Plan binding. Initial nonshared
memory remains 2 MiB with a 16 MiB maximum; final linear memory is 4 MiB. Test
caller diagnostics are separate from product declarations, so this is not a full
heap or RSS measurement.

The effective build and execution receipts are
`docs/validation/layer-blend-candidates/source8-portable-pixels-wasm-effective/build-attempt-2/build-final.json`
and `node-attempt-2/final.json`; source hashes remained unchanged. The original
source pack and its exact staged-debt assertion amendment are preserved separately.
The first execution failed because a test event omitted the partial initializer's
existing callback. Its source-derived role correction changes no work amount,
byte oracle, product code or capacity. The failed attempt remains preserved.

### Sequential intentions and required caller capture

Formats now provides a consuming whole-document intention stream bound to the
same Plan and coverage materializer. It reads each layer header once, then each
filter and own mask in order. One remaining coverage-limit state spans all
layers; errors consume the stream rather than leave a partially advanced cursor
available for reuse. Returned filters and masks preserve stored disabled/empty
intent and require the caller's complete original A/M lifetime. The scalar finish
receipt does not authorize document adoption.

The direct owned-parts filter constructor avoids an implicit intermediate clone.
Construction and complete tuple returns have explicit work charges, including
stream state and owning mask results. The native 256-filter fixture spends
6,476,460 declared stream work units without raising the original work cap.
This is a work-accounting result, not a timing or speedup measurement.

Render also exposes required-capture helpers for an existing Base caller or a
matching Base/Noise pair. They tighten the original ceilings, preserve prior
work and frozen bindings, and leave the installed contexts unchanged. Missing
or mismatched callers cannot select a fallback constructor. Private Source8
mode negotiation, original-realm provenance and complete carrier ownership still
need their own integration.

Local validation passes 18 context tests, 56 Document tests, 717 Formats tests
with 12 existing ignores, and 14 documentation checks. Strict workspace and
Main/Worker Wasm checks, formatting, source limits and parser-lock consistency
pass. The effective sixteen-path source freeze and native receipt are
`docs/validation/layer-blend-candidates/source8-r3-native/qualified-freeze.json`
and `native-attempt-1/final.json`. Source hashes remained unchanged throughout.
Preserved pre-build amendments correct fixture blend modes, retain normal-only
inputs as upstream refusals, price owning returns, and place production helpers
before tests without modifying old definitions.

Actual Node WebAssembly execution now passes 674 cases: the previous 469 plus
205 stream/capture controls. Independent target byte projections check layer
headers, ordered descriptors and masks, receipts and direct owned construction.
All 155 selected callback/debit cuts preserve exact typed errors and cumulative
retries. Scope boundaries, global limits, sequence errors, 256 distinct masks,
owner retirement and thirteen public caller-context cases pass. The unreachable
public Noise-only half-context remains covered by the native private-seam test;
no production hook was added to manufacture it.

The actual build and execution receipts are
`docs/validation/layer-blend-candidates/source8-stream-capture-wasm/build-attempt-1/build-final.json`
and `node-attempt-1/final.json`. Sources remain unchanged; nonshared initial
memory is 2 MiB, the maximum remains 16 MiB, and final linear memory is 4 MiB.
This primitive validation does not establish closed Source8 document assembly,
Worker/Main integration, full D1 accounting or physical memory/latency bounds.

### Prepared-owner payload demand

WorkspaceAdmission now exposes a read-only byte union for two actual complete
prepared payload owners, using the existing qualified ownership comparison.
A cloned owner counts once; distinct jobs with equal keys and the same work
counter count separately. The helper returns demand only and excludes existing
Arc bookkeeping; it neither admits capacity nor validates caller compatibility.
The synchronous Source8 entry needs this distinction when the original ambient
Noise binding differs from its selected Base binding.

Two native integration tests and 18 context regressions pass, including a real
matched pair with two distinct prepared bindings. Counters, budget usage and TLS
remain unchanged by the query. Strict workspace/Main/Worker checks, formatting,
source limits and parser-lock validation pass with unchanged source hashes. The
three-path freeze is
`docs/validation/layer-blend-candidates/source8-prepared-owner-union/qualified-freeze.json`.
Actual Node WebAssembly execution now passes 682 cases, preserving all 674
stream/capture cases and adding eight public owner-union controls. Independent
Wasm expectations require 260 retained payload bytes for one prepared job and
520 for distinct equal-key jobs on the same counters. Clone deduplication and
selected Base versus original ambient Noise bindings pass without changing
counters, budget usage or TLS. Sources remain unchanged; final linear memory is
4 MiB under the unchanged 2 MiB initial/16 MiB maximum configuration.
The receipts are
`docs/validation/layer-blend-candidates/source8-owner-union-wasm/build-attempt-1/build-final.json`
and `node-attempt-1/final.json`. This does not establish closed-reader ownership
or an arbitrary-many-owner union.

### Closed portable source inspection

The render worker now has a Rust-only Source8 inspection entry. It captures an
actual installed caller, requires exact original reservation capacities, and
assembles document fields, pixels, ordered filters, masks and source metadata.
It adopts the authenticated appearance metadata, returns descriptive scalars,
and retires the private document before returning. A failed Entry is terminal;
a later public call is a distinct operation on the same cumulative caller and
original factories. The existing Worker/JS protocols are unchanged.

The private owner keeps original factories alive through data destruction and
releases paired retention credit after the data. Entry controls remain 8192
bytes with the original 16384-unit control-work charge. Named overlapping values
occupy 8192 native or 5260 Wasm bytes, including the owned layout summary,
probe quotation controls and pre-frame reservation snapshot. Separate prepayment
covers the existing wire probe's
raw sort callbacks, descriptor resolution, parsing and hashes before it starts.
Existing helper bodies and checked callback fees remain unchanged. Plan-local
work is not reported as the whole caller's authentication, assembly or adoption
cost. These conservative work bounds are not performance measurements.

Native validation passes all 118 render-worker unit tests, four integration
groups and one compile-fail documentation test. The 71-wire matrix contains 39
valid sources and 32 refusals. Independent private assertions cover full physical
tile bytes, hidden/default pixels, layers, filter order, masks and ICC metadata.
Original-capacity boundaries, exact callback errors and scoped retirement also
pass. Strict workspace and Main/both-Worker Wasm checks, formatting, source limits
and parser-lock consistency pass. The 23-path native freeze is
`docs/validation/layer-blend-candidates/source8-r3-reader/qualified-freeze.json`.

The first private 256-mask test used the smaller ownership-test profile and
correctly refused. Its failed log and source remain preserved; the corrected
test uses the already-frozen whole-corpus profile without changing its grants
or domain assertions. Initial formatting/test-lint failures and their fixes are
also retained. Public full-pixel rendering, Worker8/Main activation,
exhaustive whole-reader fault injection and complete
D1/physical-memory qualification remain open. Arbitrary inherited or escaped
diagnostic heap is outside the static error shadow.

Actual Node WebAssembly execution now passes 780 cases: all previous 682 plus
71 whole-reader wires and 27 original-caller, reservation, error and ABI controls.
The reader returns the independently expected 144 scalar bytes for 39 valid
sources and rejects 32 negative wires. This validates scalar inspection and
retirement, not private pixel contents or browser publication. Sources stayed
unchanged; final linear memory is 4 MiB under the unchanged 2 MiB initial and
16 MiB maximum settings. The real ten-package Cargo build, genuine bindgen glue
and numeric execution are recorded separately in
`docs/validation/layer-blend-candidates/source8-r3-reader-wasm/build-attempt-5/build-final.json`
and `node-attempt-2/final.json`.

Earlier attempts retain the missing historical-receipt, standalone macro-context,
debug-name inventory and signed-JavaScript-sentinel failures. Six original
receipts were restored with their historical hashes; all inherited required
paths are checked. The standalone probe supplies explicitly labeled diagnostic
macro metadata, without claiming a Cargo package identity. Official debug-name
removal produces 3922967 bytes within the original 4 MiB file bound, with
unchanged executable sections and genuine glue. Its exact three imports and
reader signatures are checked by a separate inventory; the legacy parser stays
unchanged. The unsigned metric decoder preserves the original golden values.

### Closed portable source rendering

The Rust-only rendering entry now reads a private Source8 and returns an
immutable accounted RGBA image. It reuses the captured caller and original
reservations. The complete declared byte envelope is checked before frame
control admission; output bytes plus 112 bytes of retained control are admitted
before allocating the output. The image keeps its original output factory alive
after the document, frame controls and source host retire. Existing Worker/JS
routes remain unchanged.

Native validation passes all 126 worker unit cases, including original-allocation
diagnostic identity. Nine ambient/selected prepared-key combinations cover
matching, stale, distinct and cloned owners. Actual render callbacks capture
prepared aliases in both drop orders: source and frame storage retire first,
the original workspace factory survives its last capture, and image output
remains independently owned. Three integration groups check 22 independent complete
RGBA outputs, one off-canvas refusal and output ownership after host retirement.
Strict native all-target and Wasm library Clippy, formatting and the 800-line
source limit pass. The evidence is retained under
`docs/validation/layer-blend-candidates/source8-closed-frame-native`.

The first check failed with ENOSPC. Two subsequent pixel attempts exposed fixture
grammar errors: a mask run encoded its endpoint instead of width, and eight
prepared-filter catalogs were unsorted. Their original wires and failures remain
preserved. Data-only corrections preserve filter order and all expected pixels;
the production decoder is unchanged. Regenerable compiler libraries were removed
from completed conformance build caches, preserving source, executed Wasm, logs
and receipts.

Actual Node Wasm execution now passes 819 cases: all previous 780 plus 22
complete RGBA outputs, one off-canvas refusal and 16 public frame controls.
Original reservation, caller, output-factory, cancellation, diagnostic and
prepared-alias checks pass. The genuine ten-package build produces a 4041891-byte
module under the unchanged 4 MiB file bound. Initial memory remains 2 MiB,
maximum memory 16 MiB, and final linear memory is 4 MiB. Sources stayed unchanged.
The exact receipts are
`docs/validation/layer-blend-candidates/source8-closed-frame-wasm-effective/build-attempt-1/build-final.json`
and `node-attempt-1/final.json`. Source review corrected two driver errors before
compilation: historical source projection must not replace current archive
bytes, and relative path composition needs a Path prefix. Original drafts and
the separate amendment remain preserved; production code and limits did not change.

Browser publication and complete D1/physical-memory qualification remain open.
Inherited prepared bookkeeping and arbitrary diagnostic heap remain outside this
finite functional gate. This is correctness validation, not a performance claim.

### Display patch prerequisite

The Rust-only Source8 display entry now composes canonical native regions before
reducing them to display cells. One whole visible rectangle defines the global
power-of-two grid; an output patch selects at most 64 by 64 cells without moving
that grid. Partial canvas-edge cells use only actual canvas samples. Layer
composition happens before alpha-weighted encoded-RGB reduction.

The same captured caller and original reservations fund every native region in
one display operation. Positive-level accumulator storage belongs to original C;
original O covers both the retained output patch and the simultaneous native
region. Output ownership survives source retirement. Cancellation preserves its
diagnostic identity and paid work prefix; invalid geometry does not replace the
loaded source or renew its counter.

Current native validation passes 139 worker checks, including ten independent
complete display pixel oracles, exact/one-byte-short accumulator capacity, source
retirement and cancellation. Native and Wasm Clippy, formatting and the source
length guard pass. The first compilation exposed a missing test import; later
tests exposed a target-specific mutex-size assumption in the fixture. The test
now uses the existing defining decoder-control quote, rather than observed usage.
Original failures and the final source map remain under
`docs/validation/source8-display-current-audit`.

Pure Core budget/reservation/scope control quotes and the renderer's work-counter
quote now provide defining logical allocation sizes for the upcoming Worker8
factory constructor. They grant no capacity and leave existing constructors
unchanged. Core's 109 native tests and two renderer controls pass, including
unchanged reservation capacity and shared cumulative work. These quotes exclude
allocator overhead, physical RSS and separate lazy OS mutex allocations.

Actual Node Wasm execution now passes 872 cases: the preserved 838 assertions,
31 display cases and three public resource-quote controls. Ten complete display
pixel oracles and the original caller, cancellation, refusal and output-lifetime
controls pass. The public C blocker checks Entry/read admission; the exact
accumulator-capacity test remains native evidence. The genuine ten-package build
preserves all 164 dependency identities and 184 target/feature rows, with only
the reviewed local version mappings. Its module is 4,176,026 bytes under the
unchanged 4 MiB bound. Memory starts at 2 MiB, has a 16 MiB maximum and ends at
4 MiB. Sources stayed unchanged.

Build and runtime receipts are retained under
`docs/validation/layer-blend-candidates/source8-display-wasm-effective` in
`build-attempt-2/build-final.json` and `node-attempt-1/final.json`. Before Cargo,
the first attempt found one absent historical generated declaration; its exact
11,499 bytes were recovered from the authenticated prior source archive. Driver
import spelling and six numeric ABI type declarations were corrected in a
separate reviewed amendment. Original drafts and failures remain preserved.

This bounded primitive is not full default-canvas zoom readiness. Its current
positive-level sample term alone charges 373,248,000 work units for 1000 by 486
native samples, exceeding the unchanged 268,435,456 Base ceiling. Patching cannot
renew that counter. A more efficient accounted reducer and default-size tests
are required before adoption.

The connected Worker8 service, Main transport and texture publication remain
separate gates. Browser blend adoption remains disabled. These prerequisites
do not establish latency, physical-memory limits or completion of the browser
blending roadmap.

### Native-row reduction and materializer controls

The positive-level reducer now validates at most 64 native pixels as one row,
preflights every destination count, then updates exact alpha-weighted sums.
Malformed geometry or any sample overflow leaves the whole destination unchanged.
Its bounded controls remain inside the original C reservation. Row and group
work replaces repeated scalar validation; cancellation remains checked between
rows on the same cumulative counter.

Formats exposes separate pure defining quotes for coverage, document-stream,
pixel-wrapper and retained-layout controls. These facts grant no memory or
execution authority, and existing constructors retain their admission behavior.
The future service must still combine simultaneous phases and retained payloads.

Native regression, native/Wasm Clippy and formatting pass. Actual Node Wasm
execution passes 887 cases, including the previous assertions with reviewed row
accounting changes, ten materializer controls and five default-canvas controls.
All 4,394 recorded source hashes match. The module is 4,189,060 bytes, below the
unchanged 4 MiB bound; memory starts at 2 MiB, ends at 4 MiB and retains its
16 MiB maximum. Build and runtime receipts are under
`docs/validation/layer-blend-candidates/source8-display-rows-wasm-effective`,
with the root closeout in `root-qualified-887.json`.

A 1000 by 486 Normal-layer case now completes at 210,136,654 work units under
the unchanged 268,435,456 ceiling. A visible General blend still refuses on
that same bounded job, including paid retry behavior. This is progress toward
default-canvas support, not completion. The connected Worker8 service and Main
publication remain open. Kernel timings are mixed, including regressions; see
[the measurement decision](../benchmarks/display-native-rows-2026-11-10/DECISION.md).

The tracked worker suite now includes default-canvas regression cases for the
complete Normal output, the partial corner followed by a full render on one
job, original cancellation identity, paid retry refusal and General refusal.
All 142 worker checks pass. These native tests extend the earlier 887-case
Wasm evidence; they do not imply a new Wasm execution or browser adoption.

`CountedBlendSnapshot::plan_record_upper_bounds_checked` provides a paid
bootstrap for subsequent plan sizing. It retains the original immutable
document, scope and callbacks, walks roots with charged lookups, and sums
constant-time resident tile counts. The writer emits at most one record per
resident tile; hidden layers and repeated shared storage still count as
separate serialized occurrences. All-default resident tiles can be omitted,
so this is an upper bound rather than the authenticated wire count. Temporary
query controls use the original scope and retire before return.

These bounds cover record storage only. Main must still authenticate the same
encoded snapshot and combine simultaneous plan, scan, retained census and
probe scratch before declaring Worker roles. No new grant, counter or complete
source forecast is introduced by this query.

The worker also exposes compiled fixed reader controls through
`source8_reader_control_bytes`: the existing Entry C account, assembly C
controls and paired K carrier allocations. This allows Main to use defining
target sizes rather than infer private layouts or observe live usage. The
query creates no factory or work counter and grants no capacity. Source-sized
scratch, decoder controls and phase overlap remain additional requirements.
Native tests pin target literals and exercise actual Entry and closed-source
construction at exact and one-byte-short C/K capacities. All 145 worker checks,
native/Wasm Clippy, rustdoc and formatting pass. This is not a new Wasm runtime
execution or a connected browser service.

### Fresh transparent backdrop composition

The closed Source8 frame now uses a separate fresh-output renderer. A private
proof identifies the first contributing General layer in the requested region.
It can skip earlier Normal layers only when they have no resident tiles,
filters or mask and their default alpha is zero. The proof binds the immutable
document address and exact region; cached or caller-populated buffers cannot
enter this path.

After the existing filter, filter-mask, own-mask and opacity evaluation, this
first source can directly seed fresh transparent output. Zero-alpha sources
write zero RGBA, preserving the existing hidden-RGB policy. Later roots use the
original ordered compositor and each intermediate rounding boundary. General
kernel work is removed only for samples that use this copy. The containing
Source8 Normal copy work remains funded, and the General fee stays 256.

Proof work is paid before its root walk; control movement is paid before the
additional 2048-byte original W account. Public legacy/rooted and cached routes
retain their action and debit order through shared private helpers. No original
counter, factory or work ceiling is replaced.

Native default-canvas tests now pass complete Normal and General pixels under
the original 268,435,456 work limit. A real counted-writer round trip with an
empty Normal background and fractional-alpha Multiply layer also passes; its
prior ResourceLimit failure is preserved. Complete output and partial corners,
cancellation identity, cumulative retry refusal and output ownership pass.
All 256 renderer and 146 worker checks, native/Wasm Clippy, strict rustdoc and
formatting pass. Evidence is under
`docs/validation/source8-transparent-seed/root-native`.

Actual Wasm validation of this renderer change is pending. The earlier 887-case
receipt describes the row-only stage, including its then-current General
refusal. This new native result does not activate Worker8, Main publication or
browser blending, and makes no CPU-speed or full-heap claim.
