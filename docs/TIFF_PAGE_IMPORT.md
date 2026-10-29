# Selected TIFF page import

Studio's selected-page conversion is separate from ordinary Open. Ordinary TIFF
Open continues to require a single admitted image. The conversion inventories
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
behind zero alpha. Associated alpha, CMYK, floating point, nonidentity orientation
and BigTIFF require separate profiles and remain refused by this route.

The source is bounded to 64 MiB, 1,024 pages, 4,194,304 selected pixels, 4,096
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

The sealed 256-byte assessment binds the complete source digest and byte count,
selected page and page count, source descriptor, output geometry and digest,
conversion policy and exact loss set. Its digest also supplies the imported
document identity. Using only the raw file digest would give different pages
the same document, state and layer IDs. Repeating the same choice is deterministic;
choosing another page creates distinct identities.

## Frontend contract

Native Studio performs inventory and decoding through background preparation.
Browser Studio uses a dedicated TIFF Worker operation for both phases, retaining
the complete File and binding requests to source, page, owner and nonce. Worker
output must satisfy the binary assessment and independent frontend checks before
document adoption. These are logical admission limits, not physical RSS bounds.

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

The current local evidence is scoped to this conversion profile:

- The backend has 18 selected-page controls, with the complete formats library
  suite at 406 passed and 11 existing ignored tests. All-target strict checks pass.
  Thirteen independently generated libtiff fixtures contain 24 pages. Their raw
  sample oracles cover both byte orders, gray/RGB/straight alpha, GrayA8/16 with
  both gray photometrics, strips/tiles and qualified compression, small/larger ICC
  metadata lookup paths, and unsupported first pages followed by valid pages.
- The compiled CLI and real stdio MCP have five passing end-to-end groups across
  19 explicitly listed source/page profiles. They compare raw saved raster samples,
  including hidden RGB, rather than flattened display pixels. They also check
  exact report fields and consent, source aliases after Save/rename, destination
  preservation, dirty/history preservation and cancellation. The CLI binary unit
  suite has 44 passed and two existing ignored tests; all-target strict checks pass.
- Native Studio has 11 passing headless App controls and App/UI all-target strict
  checks. They exercise actual chooser/consent controls in all seven languages,
  independent GrayA16/ICC preview/document agreement, no implicit page zero,
  deterministic distinct page identities, source/stale/cancel guards, Undo,
  separate retained allocation lifetimes and resident-denial preservation.
  Both 65,537×1 and 1×65,537 pages exercise padded tile admission and the visible
  thin-preview/prior-frame rule. Existing DCX controls continue to pass.
- Production Chrome 156.0.8078.49 on the local Apple Silicon host passed all
  13 source files: 23 classic-TIFF pages inventoried, 20 qualified selected pages
  matched to independent libtiff samples, and one completed BigTIFF refusal.
  Real Studio chooser, unsupported-first/later-page selection, actual preview
  and consent, both cancellation stages, changed-source re-read and
  hang/stale/trap/malformed-result preservation passed. A separate native Rust
  reader checked all 12 downloaded PCND artifacts, including hidden RGB/alpha
  and distinct/repeated page identities. Other engines and physical devices
  remain unqualified; this is not a general browser-memory guarantee.

Raw logs and frozen source receipts are retained under ignored
`docs/validation/tiff-pages/`, `docs/validation/tiff-selected-page/` and
`docs/validation/tiff-pages-2026-10-29/`. The production browser source/artifact
receipt is `docs/validation/tiff-pages-2026-10-29/final-receipt.json`. The fixture
source, manifest and independent provenance are checked in under
`crates/services/paintcraft-formats/tests/tiff-fixtures/`.

## Remaining TIFF work

This slice does not complete the format pack's full TIFF requirements. BigTIFF,
associated alpha, CMYK/floating-point and additional sample/color/orientation
or planar-storage profiles, broader metadata/profile retention or transformation,
high-depth editing, page editing and foreign preserving Save remain separate work. Unsupported
structures fail closed rather than importing an arbitrary first page. Native
headless and independent decoder evidence do not establish original-application
compatibility, physical-device behavior, cross-browser execution or six-target
release qualification for this new profile.
