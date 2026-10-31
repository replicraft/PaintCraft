# Persistent spatial filters

Box Blur is implemented as the seventh editable filter across the model,
renderer, automation, PCND persistence and Native/Web host controls. Local tests,
required offscreen Metal and native/Wasm strict checks pass. Actual production
Chrome qualifies render Worker3, the scoped Box Studio flow and MAIN DDS/MSP export5.
These are finite appearance, consent and replay proofs; earlier six-point
production evidence retains its original scope. The broader 132-entry catalog, Gaussian,
styles and layer masks remain separate work.

## Exact Box policy

`box-blur-v1` has a required integer `radius` in 0–32, default 2 for new Studio
drafts. It operates on straight encoded RGBA8 inside the true finite layer
source extent. Output coverage never grows beyond that extent. Taps clamp at
the true source boundary, never a tile, ROI, stripe or intermediate-buffer edge.

Radius0 copies every RGBA byte, including hidden RGB. For positive radius the
fixed square has `N=(2r+1)^2` samples with clamped multiplicities. With
`Sa=sum(A)` and `Sc=sum(C*A)` per RGB channel, alpha is `round(Sa/N)` and RGB is
`round(Sc/Sa)` if `Sa>0`, otherwise transparent black. Round nearest with half
ties upward. Horizontal sums remain exact until the final vertical result;
there is no per-axis rounded average. Positive `Sa` can yield rounded alpha 0
with nonzero hidden RGB, which is retained. Raw source pixels are unchanged.

The pinned Palette implementation rounds premultiplied intermediates after each
separable pass. This independently specified PaintCraft contract may differ;
it claims neither Palette/Photoshop byte parity nor Gaussian behavior. The
[foundation](BOX_BLUR_FOUNDATION.md) and independent Python/Fraction specimens
specify the scalar policy.

### Mask once after the complete candidate

For incoming pixel `p`, complete rounded candidate `q`, and coverage `m`, let
`W=(255-m)*p.alpha+m*q.alpha`. Coverage 0 returns `p` exactly;255 returns `q` exactly.
For intermediate coverage, alpha is `round(W/255)` and RGB is
`round(((255-m)*p.rgb*p.alpha+m*q.rgb*q.alpha)/W)` when `W>0`.
When `W==0`, interpolate hidden RGB directly with denominator 255.

A missing or disabled mask means full coverage. An enabled mask supplies zero
outside its finite source-local domain. It mixes against this stage's incoming
prefix, not the original raw source, and does not restrict neighborhood reads.
Each stage rounds before the next stage. The alpha-preserving point coverage
helper is deliberately not used for this alpha-changing operation.

## Checked point boundary and regional evaluation

The general descriptor includes Box, but `evaluate_pixel` returns checked
`Result<Rgba>` and refuses Box with `CapabilityUnsupported`, including radius 0.
`evaluate_row` validates its entire descriptor list before any callback or output
write. There is no source-pixel fallback, panic or accidental point identity.
Old six-point serialization, hashing and arithmetic remain unchanged.

The renderer's `LayerAppearancePlan` captures geometry, ordered stack identity,
source-local output and per-stage input regions. Enabled stages are walked
backward: point stages keep the region; Box grows by radius and clips to the
true finite source. Forward execution evaluates each required prefix once with
bounded private buffers, including incoming pixels needed for mask mixing.
`BoxBlurPlan` quotes input/output, `u64` horizontal scratch and sample visits.
Raster reads include implicit defaults; evaluation is not limited to resident
tiles. Neither whole-canvas materialization nor a cloned history is required.

`required_input` and `box_affected_output` are distinct dependency and damage
operations, even where both are clipped radius expansions. Multiple stages
accumulate halos. The workspace accounts for region tables, actual capacities,
prefix/output buffers and maximum scratch; work is charged across shared clones
of a job admission. Refusal precedes extraction/allocation when the quote cannot
fit. Cancellation occurs between bounded phases; failed private output is
never publication authority.

CPU composition, display reduction and GPU source upload use this common
appearance. Candidate evaluation precedes downsampling. GPU presentation uploads
CPU-evaluated regional appearances; this is not a GPU blur shader. Spatial paths
avoid unsafe source-only uniform/transparent shortcuts and cache reuse that
would ignore halo dependencies. Sparse implicit defaults and mask-induced
nonuniformity are covered, including negative origins and tile partitions.

## Host workspace and lifetime

Native hosts use one process-shared 64 MiB secondary workspace realm and a
1,000,000,000-unit per-job work allowance. Canvas/thumbnail/editing and export
callbacks install admission on the actual executing thread, including worker,
GPU and CPU fallback paths. Parent asynchronous reservations add 64 MiB once for
a stored or candidate spatial job. The App's shared 3 GiB worker ceiling remains
unchanged; the editing executor's local ceiling is explicitly 2 GiB+64 MiB, while
point editing reservations remain 2 GiB. Synchronous paths use the same process
realm, not fresh per-frame budgets. Final output, source and GPU storage retain
their separate existing charges. These are logical bounds, not process RSS.

Visible merged copy/picker, flatten, print/thumbnail and generic/specialty export
callbacks route evaluated appearance; explicitly raw-layer sampling remains raw.
Native host export composition does not pass thread-local authority into a
separate installed codec process. No physical printer/clipboard or changed
native codec-worker profile qualification follows from these headless tests.

The persistent render Worker advances its protocol and admits spatial source,
workspace and work facts before executing a candidate. Source PCND is frozen and
decoded once per client; sliders reuse it. The MAIN profile still bounds source
wire, linear memory and output. DDS/MSP protocol 5 carries a separately checked
spatial context for PCND5; older protocol 4 does not silently gain Box support.
Local Worker/codec controls and actual built-browser Worker3, Box Studio and
MAIN DDS/MSP export5 checks pass for the finite cases recorded below. JavaScript
cancellation is observed between synchronous Wasm patches,
not during a running call; no strict maximum latency is claimed.

## Closed automation profiles

| Surface | Condition | Older profile preserved |
|---|---|---|
| `paintcraft.batch/v5` | Box Add/Update | Original kinds/scalars v2; masks v3; four newer point Add/Update v4 |
| CLI `paintcraft.inspect/v4` | Any stored Box, including disabled/radius 0 | Existing v1/v2/v3 selection otherwise |
| MCP `paintcraft.mcp-inspect/v4` | Same condition; separate MCP projection | Existing unversioned/v2/v3 otherwise |
| Result/file-command v4 | Final document stores spatial intentions | Minimal v1/v2/v3 after the last Box is removed |

Batch v5 alone does not force output v4. Actual spatial `document_features` are
`layer-point-filters.v1`, optional `layer-filter-masks.v1` iff a mask is stored,
`layer-spatial-filters.v1`, then optional `source-metadata.v1`. Older closed
schemas remain unchanged. Updates, toggles and duplicates preserve masks; one
atomic publication preserves source, aliases, history and metadata admission.
All-known losses never authorize baking. Captured Bake and source-metadata Omit
remain separate authorities bound to the actual frozen copy.

## PCND5 grammar versus stored inventory

PCND5 / `paintcraft.document/v5` is selected iff any stored Box exists, including
disabled/radius 0. The header requires raster/finite/flat, point, mask and spatial
grammar capabilities unconditionally. A maskless PCND5 still needs a reader that
understands optional-mask framing; it does not manufacture a stored-mask result
feature. The bounded probe derives capabilities from the version without a
stack scanner or extra presence flag.

PCND5 reuses optional source and per-instance optional-mask framing and adds
algorithm tag 7 plus radius byte 0–32. Point tags 1–6 keep exact encodings.
Unknown tag 7 in PCND3/4 remains a valid refusal specimen. A v5 file without a
spatial intention is noncanonical. Removing the last Box selects exact v4 for
remaining masks, v3 for point-only stacks, or v1/v2 for empty stacks. The existing
8 MiB metadata cap, aggregate mask counts/work and explicit read admission remain
in force. Header capabilities never grant allocation or render authority.

## Current qualification

The local suites pass 64 Core, 25 Effects, 25 Document and 16 Commands tests;
58 PCND controls plus two recovery controls; and 123 renderer tests, including
38 required offscreen Metal controls. Three workspace-helper controls pass
separately. Actual CLI/stdio qualification includes two workspace groups, five
Box groups, 120 schema checks over 26 genuine outputs, nine legacy groups, one
future-version control and strict checks.

Four Studio tests and 11 new Native groups pass, alongside 18 older filter
lifecycle controls. They cover private preview/Cancel/Apply, real CPU sync/FIFO,
Metal texture/readback, clipboard/export callbacks, PCND5 save/reopen and
single-layer Flatten/Undo. Flatten removes intentions while preserving literal
canvas pixels and returns PCND1 when no newer semantics remain. App/Studio/UI
strict and all seven locales pass. The Native evidence is retained in
`docs/validation/native-box-ui/final-freeze.json` and
`single-layer-final-freeze.json` in that directory.

Web passes 391 tests with three existing ignored; Worker passes 17, and native/Wasm
strict passes. DDS/MSP protocol 5 passes five Formats groups, two codec-entry
groups, four older library groups covering 23 tests, four registry controls and
native/Wasm strict. Node passes 197 tests. Source preimages, the intermediate
graph-overlap compile failure, test fixture corrections and the sandbox
no-adapter failure remain retained alongside corrected passes.

## Production browser qualification

The production build passes all five steps (MAIN, render Worker3, codec Worker5,
Astro and asset checks), with 3305 input records and 27 frozen assets. The raw
Worker, Studio and DDS/MSP receipts bind the same 27 asset hashes.

- Raw Chrome Worker3: 17 independent source specimens and 104 finite controls,
  including source-once reuse, typed refusal and acknowledged retirement. This
  does not establish a maximum scheduling or cancellation latency.
- Visible Box Studio: seven checks cover actual Add, default radius 2 and changed
  radius 1, causal screenshot markers and exact bound Worker planes, byte-identical
  Cancel, exact stack/raw-source Apply, Undo/Redo and fresh PCND5 reopen/save.
  Apply legitimately promotes the admitted private texture; fresh reopen verifies
  strict committed BASE binding. Existing-instance Edit, duplicate and preview-off
  are not newly qualified by this flow.
- MAIN DDS export5: actual capture, Bake revocation/recheck and download pass.
  Nine fresh Worker replays include one positive and eight negative mutations;
  Pillow 10.3.0 independently decodes the positive file to the exact fixture plane.
  The source remains unchanged.
- MAIN MSP export5 also passes real consent/revocation/recheck/download,
  fresh Worker byte identity and nine replays (one positive, eight negative).
  Pillow independently decodes its 2×1 opaque-white plane exactly; this uniform
  fixture does not establish a causal color-change marker. Both MAIN copy flows
  preserve source/title/dirty state, retire revoked intent without download and
  use a fresh nonce after recheck. Fresh Worker artifacts match downloaded bytes.

Exact retained receipts are:

- `docs/validation/box-production/attempt-2/final-freeze.json`
- `docs/validation/box-production/raw-worker/attempt-HcVyrK/final.json`
- `docs/validation/box-production/ui/attempt-lCvdlH/production.json`
- `docs/validation/box-production/export5/attempt-TmfN66/final.json`
- `docs/validation/box-production/export5/attempt-1jHnpq/final.json`
- `docs/validation/box-production/current-assets-recheck.json`

The current Formats library regression passes 543 tests with 12 existing ignored;
ten relevant integration targets separately pass 21 tests with one existing
ignored. These are not a new full all-targets total. Historical six-point counts
retain their original scope. Calibration and failed attempts remain preserved.
No full 132-catalog, GPU blur shader, performance, external-application or
all-target compatibility claim follows from these local and browser controls.

[CI run 41153720980](https://github.com/replicraft/PaintCraft/actions/runs/41153720980)
passes all seven jobs for `326f7f851955b00bdce8b962246debec32df8461`, including
the production web build, Ubuntu/macOS workspace checks, three native filesystem
targets and X11 clipboard checks. Its structured final receipt is retained under
`docs/validation/box-production/ci-41153720980-final.json`. CI does not replace
the separately scoped browser/Metal evidence or physical-device and
original-application qualification.
