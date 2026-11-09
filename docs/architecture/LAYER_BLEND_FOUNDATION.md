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

Actual Wasm execution is pending. These are caller-guarded primitives, not a closed
Source8 reader: pixel decoding, complete source/candidate/frame ownership, Worker
activation and final-owner cleanup remain separate gates. No physical memory or
latency qualification follows from the structural control/work declarations.
