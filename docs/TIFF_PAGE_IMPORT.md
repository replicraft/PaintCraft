# Selected TIFF page import

Studio's selected-page conversion is separate from ordinary Open. Ordinary TIFF
Open continues to require a single admitted image. Both routes normalize valid
TIFF Orientation values 1 through 8 into the actual pixels exactly once;
orientations 5 through 8 swap the output dimensions. The conversion inventories
the complete captured source and requires an explicit page choice, including
page zero. A page with unsupported sample semantics stays visible and disabled;
it does not prevent choosing a qualified later page. Invalid directory chains,
cycles, out-of-range payloads and exhausted resource budgets reject the source.

The selected page becomes a new unsaved RGBA8 document. This does not introduce
TIFF page editing, foreign preserving Save or automatic conversion of every page
into a layer. Saving the resulting project uses PCND. Source identities remain
guarded in the importing session after that Save, including source renames and
hardlinks. This is not persistent TIFF source authority recovered by reopening a
PCND file.

## Pixel and metadata policy

The profile is `tiff-selected-page-rgba8-v1`. It admits qualified 1-bit gray
and 8/16-bit integer gray, RGB, gray-alpha and RGBA samples. Alpha is straight;
16-bit reduction explicitly takes the high byte. This is distinct from the
JPEG XL round-to-nearest conversion rule.
WhiteIsZero grayscale inversion affects gray values only, including gray values
behind zero alpha. Orientation is a checked whole-pixel permutation, including
hidden RGB; it does not premultiply alpha or change sample values. Associated
alpha, CMYK, floating point and BigTIFF require separate profiles and remain
refused by this route. Ordinary single-image RGBA TIFF uses the same checked
normalization; gray with alpha remains specific to selected-page conversion.

The selected-page source is bounded to 64 MiB, 1,024 pages, 4,194,304 selected pixels, 4,096
tags per page, 65,536 tags in total, 1 MiB of metadata per page and 4 MiB of
inventoried metadata in total. Each canvas side also obeys the shared
1,048,576-pixel editor limit. Native private document
preparation admits the checked tile-padded extent separately from the decoded
pixel count, with the canonical staged allowance and shared resident-byte budget.
Thus fitting the pixel limit does not guarantee available tile memory. The source
parser validates absolute strip, tile and tag offsets against the original file.
It does not extract a selected page as an isolated byte slice. A decoder input
view supplies the selected initial directory pointer while payload reads retain
their original addressing. Grayscale with alpha uses the selected direct TIFF
decoder because the common image adapter does not expose that sample layout.

Conversion reports every applicable loss before publication:

- `tiff-other-pages-not-imported` when the source contains more than one page.
- `depth-16-reduced-to-8` for 16-bit source samples, even if their low bytes are zero.
- `tiff-metadata-ignored` when admitted source metadata is omitted.
- `icc-profile-ignored` when the source ICC profile is omitted.

The sealed 256-byte V2 assessment (`PCTIFF02`) binds the complete source digest
and byte count, selected page and page count, original source dimensions and
Orientation, output digest, conversion policy and exact loss set. Its encoded
dimensions remain original source facts. Normalized output dimensions derive
from those facts, swapping axes for orientations 5 through 8, and the output
orientation is 1. Adoption checks the exact normalized dimensions and samples,
so the transform is never applied again. The policy is
`tiff-uint16-high-byte-oriented-rgba8-v2`. Its digest also supplies the imported
document identity. Using only the raw file digest would give different pages
the same document, state and layer IDs. Repeating the same choice is deterministic;
choosing another page creates distinct identities. V2 intentionally gives all
selected-page conversions new identities, including orientation 1. Older V1
reports are rejected rather than interpreted under the new contract; published
0.1.1 artifacts are unchanged.

## Frontend contract

Native Studio performs inventory and decoding through background preparation.
Browser Studio uses a dedicated TIFF Worker operation for both phases, retaining
the complete File and binding requests to source, page, owner and nonce. Worker
output must satisfy the binary assessment and independent frontend checks before
document adoption. Browser wire V2 binds the original inventory dimensions and
Orientation to the selected re-read, then requires exactly the normalized output
geometry. Request, runtime and Rust-facing service declarations use one shared
JavaScript protocol-version constant. Old wire V1 requests/results are refused. These are logical admission limits, not physical RSS bounds.

The shared chooser makes no implicit selection. A retained decoded preview and
the previous visible frame precede loss consent. A very thin preview may expand
one display axis to one logical point within the available area; its true
source dimensions stay labeled, and its editable samples are unchanged.
Cancellation, source changes, stale ownership or invalid results preserve the
current document and history.

CLI `import --page` and MCP `import_image` with `page` apply the same Studio
profile and require every actual loss in their acknowledgment lists. Their
conversion result includes the source assessment. Classic profiles cannot acquire
this conversion by supplying its variant name.

## Qualification status

The orientation follow-up is locally qualified on macOS Apple Silicon:

- Backend checks pass 22 controls. The new independent coordinate corpus has
  25 authored classic-TIFF files and 32 pages: all eight orientations, asymmetric
  3 × 2 samples, both byte orders, straight RGBA8 with hidden RGB and selected
  GrayA16 using the existing high-byte rule. Python writes raw IFDs/scanlines;
  a separate libtiff scanline reader verifies the original samples and tags.
  Ordinary single-image RGBA8 cases pass in both byte orders. Multipage ordinary
  Open and ordinary GrayA remain refused.
- Three actual host Worker controls pass, including all eight selected-page
  orientations, exact normalized pixels and rejection of a forged inventory
  Orientation. Seven host Web controls pass, with the two external-artifact
  checks run separately. The existing 54-case independent JPEG group also passes
  after sharing the checked pixel-permutation helper.
- Native Studio passes three new headless consumer groups, including eight real
  selected-page reviews and two ordinary image cases. Actual CLI and stdio MCP
  pass two groups over 14 profiles, checking original/normalized report fields,
  saved raw samples, deterministic policy-bound identities and source guards.
  Matching native/CLI/MCP artifacts agree. For the shared standalone GrayA16
  orientations 2 and 7, the actual native, Web, CLI and MCP PCND artifacts are
  byte-identical. All-target strict checks pass.
- Production Chrome 156.0.8078.49 passes the new 25-file/32-page Worker corpus
  and all eight real GrayA16 Studio chooser, preview, consent, Save and reopen
  workflows. Sixteen downloaded orientation PCND artifacts match the independent
  raw-pixel oracles and retain their identities on reopen. The same run repeats
  the initial 13-file corpus: 23 classic pages inventoried, 20 qualified decodes
  and a completed BigTIFF refusal. Its chooser, consent, cancellation,
  changed-source and hang/stale/trap/malformed preservation controls pass again.
  A separate native Rust reader checks all 28 downloaded PCND artifacts.
- Full JavaScript checks pass 94 tests. Formats/Web/Worker strict checks,
  WebAssembly builds, Biome, formatting, source limits and built asset checks pass.
  The Worker ceiling remains 4,096 WebAssembly pages (256 MiB linear memory),
  not a physical RSS, GPU-memory or synchronous execution-stop guarantee.

The frozen orientation receipts are
[`tiff-orientation/final-freeze.json`](validation/tiff-orientation/final-freeze.json)
and the separate native/CLI/MCP
[`tiff-orientation-consumers/final-freeze-v2.json`](validation/tiff-orientation-consumers/final-freeze-v2.json).
The separate [four-transport artifact comparison](validation/tiff-orientation-four-transport-comparison.json)
binds those two matching sources without implying all profiles ran in every
transport. The complete successful production run is retained at
[`attempt-3wCBVq/production.json`](validation/tiff-orientation/production/attempt-3wCBVq/production.json).
Two earlier production attempts exposed separate JavaScript version-gate defects;
their failures and the corrected full run are preserved, rather than counted as
successful UI qualification. These ignored local receipts do not establish a
new release or green main-branch CI.

The initial selected-page evidence remains historical under ignored
`docs/validation/tiff-pages/`, `docs/validation/tiff-selected-page/` and
`docs/validation/tiff-pages-2026-10-29/`, including its original production
[`final-receipt.json`](validation/tiff-pages-2026-10-29/final-receipt.json).
Independent fixtures and reproducible source are tracked under
[`tiff-fixtures`](../crates/services/paintcraft-formats/tests/tiff-fixtures/README.md)
and [`tiff-orientation-fixtures`](../crates/services/paintcraft-formats/tests/tiff-orientation-fixtures/README.md).

## Remaining TIFF work

This slice does not complete the format pack's full TIFF requirements. BigTIFF,
associated alpha, CMYK/floating-point, additional sample/color
or planar-storage profiles, broader metadata/profile retention or transformation,
high-depth editing, page editing and foreign preserving Save remain separate work. Unsupported
structures fail closed rather than importing an arbitrary first page. Native
headless and independent decoder evidence do not establish original-application
compatibility, physical-device behavior, cross-browser execution or six-target
release qualification for this new profile.
