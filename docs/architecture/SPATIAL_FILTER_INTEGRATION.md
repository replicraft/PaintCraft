# Persistent spatial filters: integration plan

Status: **planning, not an exposed persistent filter**. The independently
specified Box Blur scalar foundation is qualified: validated `BoxBlurV1`, checked
`BoxBlurPlan`, caller-owned buffers, exact finite-source clamping, spatial mask
mixing and independent arithmetic/partition/cancellation controls. This does not
add Box Blur to the persistent `Descriptor`, Studio, automation or a file format.
The shipped six [point profiles](POINT_FILTER_PROFILES.md) remain unchanged.

The next integration must connect every appearance consumer, admission boundary,
closed schema and save/reopen route. A point-only error or temporary capability
refusal is acceptable during development; silently displaying raw pixels or
persisting an unsupported intention is not completion.

## Decisions and unresolved details

The current direction is:

- Add a persistent Box descriptor only with the complete spatial route below.
- Change the general-descriptor point API to a checked `Result`; spatial kinds
  are never approximated by a point kernel, including Box radius zero.
- Select **PCND5 / `paintcraft.document/v5`** whenever a document stores a spatial
  intention, including a disabled instance or radius zero.
- Declare a new spatial required feature. `layer-spatial-filters.v1` is the
  proposed name; existing `layer-point-filters.v1` is not silently widened to
  cover neighborhood reads or alpha-changing mask composition.
- Keep existing PCND1–4 framing, feature declarations, algorithm registry and
  canonical bytes unchanged. Preserve all older closed automation profiles.
- Advance Worker protocols/admission or explicitly refuse spatial sources and
  operations. An old point-only workspace quote cannot authorize spatial work.

The preferred PCND5 probe uses a version-derived grammar capability list,
requiring point, mask and spatial support unconditionally. It needs no new flags
or stack scanner merely to discover actual mask presence. The exact prelude,
instance layout and feature order are **not frozen** until binary fixtures settle
the wire contract. The schema numbers below are proposed
conditional profiles; their closed documents must be reviewed before acceptance.
This plan is not a wire specification or evidence of deployed reader support.

## Exact Box policy

The foundation's policy has integer radius 0–32, default 2. Inputs and outputs
are straight encoded RGBA8. Output stays inside the finite layer source extent;
Box does not grow layer coverage beyond those bounds. Out-of-bounds taps clamp
at that true extent, never at a tile, requested ROI or scratch-buffer edge.

Radius zero copies every RGBA byte, including hidden RGB. For positive radius,
use the fixed square of `(2r+1)^2` samples, including clamped multiplicities. Let
`Sa = sum(A)` and `Sc = sum(C*A)` for each encoded RGB channel. The candidate is:

- `A' = round(Sa/N)`.
- `C' = round(Sc/Sa)` when `Sa > 0`; otherwise the candidate is transparent black.

Round nearest with ties upward. Horizontal scratch contains exact sums; do not
round horizontal averages before the vertical sum. Candidate RGB can be nonzero
when the rounded output alpha is zero; that valid hidden result is not discarded
merely because alpha rounded down. Raw source tiles remain unchanged.

The pinned Palette Box rounds premultiplied intermediates after each separable
pass. PaintCraft's exact final rational policy is independently specified and
can differ. It does not claim Palette byte parity, Photoshop parity or Gaussian
behavior. Gaussian's approximation/support rules are a separate profile.

### Alpha-changing mask application

Use the new spatial coverage rule once per stage, after the complete candidate.
The existing point coverage helper preserves incoming alpha and ignores candidate
alpha; it must not be reused for Box.

For incoming `p`, candidate `q`, mask byte `m`, and
`W = (255-m)*p.alpha + m*q.alpha`:

- Coverage 0 returns `p` exactly; coverage 255 returns `q` exactly.
- For intermediate coverage, output alpha is `round(W/255)`.
- If `W > 0`, output RGB is
  `round(((255-m)*p.rgb*p.alpha + m*q.rgb*q.alpha)/W)`.
- If `W == 0`, interpolate hidden RGB directly:
  `round(((255-m)*p.rgb + m*q.rgb)/255)`.

A missing or disabled mask means full coverage. An enabled mask is sampled at
the output's original source-local coordinate and supplies zero outside its
finite domain. The mask does not restrict the candidate's neighborhood input.
Mask coverage is not applied again after the stack or folded into blur radius.
Each stage rounds before the next stage reads it; point→Box and Box→point are
ordered, distinct operations.

## Safe evaluator transition

The general `Descriptor` currently contains six point kinds, and the public
`evaluate_pixel` returns an infallible `Rgba`. Adding Box must not add a fallback
arm returning the original pixel, panic, `unreachable!` or an unchecked unwrap.

The smallest safe transition keeps existing flat descriptor construction and
changes `evaluate_pixel(&[Descriptor], Rgba)` to `Result<Rgba>`. The private point
kernel also accepts only supported point descriptors or returns a typed
`CapabilityUnsupported` error. `evaluate_row` validates all descriptors before
callbacks or output writes, so a later Box cannot partially evaluate a point
prefix before rejection. Radius zero is still a spatial kind for this admission
boundary; its mathematical identity does not grant point-only authority.

A genuine separate six-kind `PointDescriptor` plus checked conversion from the
general descriptor is a possible stronger type boundary. A wrapper around the
general enum alone does not remove the unsupported-match problem. Nesting all
existing variants under `Descriptor::Point` is unnecessary migration for this
first slice. Either approach must preserve the six point contracts and old wire
identities exactly.

The principal production callers are the renderer's appearance `mix` and uniform
proof, plus Effects row evaluation. Tests/oracles must adapt to checked success;
renderer consumers must use spatial evaluation rather than merely propagate an
unsupported error indefinitely after the UI exposes Box.

## Ordered region staging

Follow the normative [effect-region contract](../temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/EFFECT_REGIONS.md).
`required_input(output)` and `affected_output(changed_input)` are distinct public
concepts even when both are clipped expansions for this particular Box kernel.

A proposed render-owned `AppearancePlan` captures layer geometry, ordered stack
identity, requested source-local output, per-stage regions and a checked peak/work
quote. Walk enabled stages backward: a point stage retains its region; a Box
stage grows it by radius and clips at the true finite source. Account for the
incoming output pixels needed by post-candidate mask mixing. Then walk stages
forward, evaluating each required prefix region exactly once.

The pure `BoxBlurPlan` already reports its required input, output, pixel counts,
scratch elements, simultaneous storage bytes and sample visits. The adapter
supplies its exact tightly packed input and disjoint output/scratch slices. Read
raw pixels through `RasterLayer::row_spans` / `read_span`, including implicit
default pixels. No whole-canvas image or cloned history is needed. Avoid recursive
per-pixel evaluation of upstream filters: multiple spatial stages would otherwise
multiply work exponentially.

Charge the stage-region table, private prefix buffers and maximum horizontal-sum
scratch, including actual capacities. Two alternating prefix buffers and output
stripes are a viable first strategy; the planner must include live mask-mix inputs
and avoid overwriting a source region still needed by a later sample. A long
stack's cumulative halo can exceed available admission. Refuse before allocation
instead of ignoring stages or materializing an unbounded intermediate. Streaming
rings may reduce that peak later, without changing the scalar contract.

## Appearance, cache and resource gates

| Area | Required change |
|---|---|
| CPU composition | Evaluate a layer's requested appearance region once per admitted partition, then apply layer opacity and ordinary ordered source-over. Replace raw-span-only point assumptions for spatial stacks. |
| Sparse/uniform proof | Prove the entire upstream halo and preceding mask effects uniform. A locally transparent source ROI may receive alpha from adjacent visible samples. Do not skip it before spatial evaluation. A wholly transparent, zero-resident finite source remains zero-alpha for Box. |
| Display/LOD | Evaluate native-resolution candidates and compose before the existing display reduction. Blurring a pre-downsampled source is a different algorithm. |
| GPU source upload | CPU-evaluate the admitted regional appearance before upload. Composition can remain GPU-backed; this does not establish a GPU blur shader. |
| Composite/display/source caches | Source versions must cover required halos; damage must expand through later spatial stages. First safe integration may bypass caching and incremental patches for spatial stacks using existing uncached paths. Point-only cache behavior stays unchanged. |
| Parallel renderer | Output writes remain disjoint, but every active partition's halo/scratch must be included in the shared peak; independent Rayon locals are not free. |
| Runtime | Reserve the real request peak before submission; retain it while running/ready/cancelled until results are consumed. Keep existing owner/state/epoch/generation publication checks. |
| Export and visual consumers | Flat/specialty exports, visible picker/copy, thumbnails, print and preview must share evaluated appearance. Explicit raw-layer sampling remains raw. |

Current Native render reservations include output, resident tiles and existing
preview/cache allowances, not an arbitrary spatial chain's scratch. Add the
actual plan quote before `Executor::submit`. Caller-owned `BudgetScope` accounts
or a reserved scope can manage independent storage ownership without pretending
that a quote itself admits memory. Retain GPU submission leases through observed
completion. None of these declarations is a process-wide RSS/GPU-memory bound.

The browser persistent render Worker currently computes one synchronous patch of
up to 32×32 display samples per step. At nonzero LOD, the native footprint plus
halo can be much larger. Quote and admit that actual footprint and work before
accepting a candidate; existing 4 MP output, 64 MiB source wire and 256 MiB linear
memory ceilings do not imply enough spatial scratch. Reuse the immutable decoded
source between parameter updates; do not serialize/decode PCND per slider edit.
Native callbacks can cancel between bounded phases. JS cancellation is observed
between synchronous Wasm calls; strict maximum latency needs resumable work, not
just an async wrapper. Old protocol versions must refuse unsupported spatial work.

## Closed automation and output profiles

Proposed lowest-required-version rules:

| Surface | Spatial condition | Legacy behavior |
|---|---|---|
| `paintcraft.batch/v5` | Box Add/Update | Old six Add/Update keep their v2/v4 requirements; mask ops v3; scalar stack actions v2. Closed v1–4 reject Box Add/Update. |
| CLI `paintcraft.inspect/v4` | Any stored spatial kind, including disabled/radius zero | Six-kind inspection still selects existing v1/v2/v3 exactly. |
| MCP `paintcraft.mcp-inspect/v4` | Same stored-kind condition, distinct MCP projection | Existing unversioned/v2/v3 shapes remain unchanged. |
| Result/file-command v4 | Final document requires the new spatial feature list | If no spatial intentions remain, select original v1/v2/v3 from final masks and aliases. A v5 request alone does not force v4 output. |

Existing result/v3 and file-command/v3 are closed, including their feature lists.
A new spatial feature cannot be inserted into those profiles. Define the v4
schemas and exact canonical feature order before emission. Stored disabled and
radius-zero spatial intentions still require support and must not be omitted
from inspection or preservation checks. These actual document features remain distinct from the PCND5 header
capability list: spatial is present iff stored spatial intentions exist; mask is
present iff a mask is stored, including disabled/zero fields. Exact canonical
ordering and inherited point/source declarations still need schema review.

Descriptor hashing must include Box's versioned ID and radius without changing
old six-kind digests. Updates/toggle/duplicate preserve masks; atomic preparation,
metadata save viability, aliases, Undo/Redo and source-local transforms retain
existing guards. The extra descriptor bytes still count against the unchanged
8 MiB metadata cap. All-known loss lists never grant baking authority; existing
captured Bake and source-metadata Omit remain separate policies.

## PCND5 grammar capabilities and probing

Current PCND probing verifies framing/hash and reads the prelude. Its
`required_features` list describes versioned grammar capabilities, not a complete
inventory of stored per-layer intentions. Point/mask capability is derived from
container version. Optional source metadata is discovered from its existing
prelude presence field; no full per-instance mask scanner is necessary.

The preferred PCND5 design therefore requires point, mask and spatial grammar
capabilities unconditionally. A reader must understand the spatial algorithms
and optional per-instance mask framing, including a stream in which all masks
are absent. The bounded header probe can report these capabilities from version
5 without allocating tiles or walking all stack records. No new header flags or
scanner are needed solely to discover actual mask inventory.

Two distinct feature declarations must remain explicit:

1. **Header grammar capabilities:** PCND5 requires point, mask and spatial support
   even when no actual point instance or mask is stored.
2. **Stored document meaning:** result/inspection feature lists include the new
   spatial feature iff a spatial intention is stored and the mask feature iff an
   actual mask is stored, including disabled and zero coverage. A header mask
   capability does not manufacture a stored-mask result feature.

The full reader still validates every algorithm, parameter and mask and enforces
canonical container selection. Existing mask-aware read admission remains
required for any decoded payload; header capability is not allocation authority.
This simpler direction replaces a proposed conditional-mask header flag/scanner.
It is a planning decision, not a frozen binary layout: precise framing, feature
order and independent PCND5 fixtures still need review. Additional flags should
be added only for another actual wire requirement, not to conflate grammar
capability with document inventory.

The existing v4 optional-mask framing is a reuse candidate, not authorization to
emit an all-maskless v4 document: old v4 canonical rules remain unchanged. PCND5
would be conditional on at least one stored spatial intention. Removing the last
spatial kind should return to exact PCND4 for stored masks, PCND3 for nonempty
point-only stacks, or PCND1/2 for empty stacks. Probe and full read must agree on
that conditional canonical meaning.

A numeric algorithm tag for Box has not been frozen. If tag 7 is selected for
PCND5, **unknown tag 7 in PCND3/4 remains a legitimate refusal specimen**. There
is no need to rewrite those legacy negatives as accepted merely because another
container version knows that numeric value. Readers must gate registry entries
by container/profile, including disabled instances. Existing PCND1–4 fixtures and
bytes remain authoritative. Update selected compact preflight, raw writer, full
reader, source inspection, recovery and all host read-admission paths together.

## Completion gates and ownership

Suggested implementation groups, with no changes authorized by this document:

- Core/Effects: persistent descriptor/parser/hash support only after the checked
  point boundary and qualified scalar Box policy are available.
- Render: region plan/workspace, CPU/parallel/display/GPU upload adoption,
  conservative cache handling and shared quote APIs.
- Commands/Editor/CLI/MCP: closed input/inspection/result profiles, actual atomic
  operations and unchanged no-op/source/history/failure semantics.
- Formats: finalized PCND5 capability probe/layout/registry/preflight/read admission,
  independent wire fixtures and specialty export workspace/capability guards.
- Native/Web/UI/Workers: actual captured private parameter drafts, admitted latest
  rendering, all protocol/version quotes, explicit copy consent and presentation.

Independent gates include radius 0/1/32, tiny and nonsquare sources, negative and
extreme valid origins, true edges versus partition boundaries, split sizes
1/2/3/128/256, implicit defaults, transparent hidden RGB and alpha-changing masks
at 0/128/255. Cover point→Box versus Box→point, halo-only edits invalidating
cached output, CPU/required Metal presentation, native versus reduced display,
resource peaks/cancellation/stale publication, exact source preservation,
metadata-limit atomic refusal and all PCND/CLI/MCP/UI save/reopen routes.

Do not expose a persistent kind as complete from the pure kernel alone. The
132-entry upstream catalog, styles, Gaussian and other spatial/global families
remain separate work; this plan neither implements them nor establishes parity.
