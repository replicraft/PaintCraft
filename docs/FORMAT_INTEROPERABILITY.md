# Format interoperability implementation

This ledger follows `docs/temp/paintcraft-format-interoperability-classic-studio-v3` alongside the canonical editor and PCND specifications. It records implemented operations as of 2026-10-25, not completion of the entire planning pack.

Codec implementation, enabled frontend operations and release qualification are separate gates. `paintcraft_formats::registry` controls discovery and admission; individual adapters enforce their narrower file profiles and resource limits. A listed format does not imply support for every variant, metadata field or original application version.

## Available workflows

Desktop and browser use the same frontend policy. CLI and MCP import/export operations also apply that policy, using an explicit frontend profile rather than inheriting every compiled codec. All foreign conversions require acknowledgment of their reported losses before publication. Failed, cancelled or stale conversions leave the existing document intact.

- PCND is the native layered project format and the preserving Save destination.
- Classic 7/10/11 open and export PNG, JPEG, BMP, still GIF and single-page TIFF. CLI Classic profiles can explicitly import the admitted PCX subset; the Classic 11 CLI profile also admits the limited PAINT conversion. Desktop/browser Import image is currently a Studio action, so these headless allowances do not imply additional Classic menus.
- Studio also opens TGA, still WebP and admitted single-surface DDS images, and opens the admitted historical PDN subset as a converted copy.
- Studio's explicit Import image workflow adds QOI, Netpbm, farbfeld, PCX, selected DCX pages, flat OpenRaster layers and the admitted PAINT subset. These extended imports do not silently expand ordinary Open or Recent filters.
- Studio offers **14 flat exports**: PNG, JPEG, BMP, GIF, TIFF, TGA, WebP, JPEG XL, DDS, AVIF, QOI, PAM, farbfeld and PCX with RGB24 or indexed palettes. PAM output always uses `.pam`, never a `.ppm`, `.pgm`, `.pbm` or generic `.pnm` filename.
- DCX requires a page choice. Desktop/browser stage a page chooser before decoding and loss confirmation; CLI `import --page` and MCP `import_image` use an explicit zero-based page index. There is no implicit page-zero import.

Studio's planned stock inventory remains exactly PDN, PNG, JPEG, JXL, JXR, BMP, GIF, TGA, DDS, TIFF, HEIF, WebP and AVIF. That inventory is not a claim that all those readers are enabled. PCND and qualified extended conversions are additional workflows. HEIF/HEIC writing and SVG export remain prohibited.

## Current delivery ledger

| Family | Enabled operations and qualified subset | Important limits or losses |
| --- | --- | --- |
| PCND | Native layered Open/Save with bounded container validation, compact sparse chunks and recovery integration | Retains project layers, transparency and canvas properties; foreign preserving writes are separate work |
| PNG, JPEG, BMP | Classic/Studio raster Open and flat Export | Bounded RGBA8 conversion; JPEG quality is adjustable and loses alpha; metadata/depth/hidden-RGB losses are reported as applicable |
| GIF | Classic/Studio still Open and palette Export | One full-logical-screen frame only; partial frames/animation refused; palette, alpha and hidden-RGB export losses are explicit |
| TIFF | Classic/Studio single-page integer grayscale/RGB/RGBA Open and RGBA8 Export | Precision/metadata losses explicit; multipage, CMYK, floating point, ambiguous/associated alpha and unqualified orientation variants refused; writer declares unassociated alpha |
| TGA | Studio integer raster Open and lossless RGBA8 Export | Admitted orientation, RLE and palette profiles only; palette/metadata losses explicit |
| WebP | Studio still Open and lossless RGBA8 Export | Animation and unqualified extensions refused; import metadata losses explicit |
| DDS | Studio single-surface Open and exact RGBA8 Export | Reader admits RGBA/BGRA/BC1/BC2/BC3 with explicit sRGB transfer and straight or opaque alpha only. Linear/unspecified transfer, associated/unknown alpha, mipmaps, arrays, cubes and volumes refused. Writer emits uncompressed DX10 RGBA8 sRGB straight alpha |
| JPEG XL | Studio lossless RGBA8 Export only | At least two pixels per side, at most 4,194,304 pixels and 64 MiB encoded output. Internal decoder evidence does not qualify hostile-input Open; no frontend reader enabled |
| AVIF | Studio lossy RGBA8 Export only, adjustable quality | Lossy even at quality 100; alpha may change and transparent RGB may be discarded. At most 1,048,576 pixels, 4096 per side and 16 MiB encoded output. No decoder enabled |
| QOI | Studio explicit Import and lossless RGBA8 Export | Strict complete raster/end marker and sRGB profile; linear declaration refused; exact alpha and hidden RGB retained by the codec |
| Netpbm | Studio explicit P1 through P7 Import; PAM P7 RGBA8 Export only | Admitted max values 1, 255 or 65535 and recognized grayscale/RGB/alpha tuples; bounded 64 KiB header. High-depth import reduces to 8 bits; metadata loss explicit |
| farbfeld | Studio explicit Import and lossless RGBA8 Export | Exact body length required; imports reduce 16-bit samples to 8 bits with explicit loss. Exports widen each 8-bit sample by multiplying by 257; this does not add high-depth editing |
| PCX | Classic/Studio explicit Import; Studio RGB24 flat Export | Version 5 RLE: admitted packed 1/2/4/8-bit, planar 1-bit and RGB24 profiles; minimal even row padding; strict row/plane bounds. Palette and metadata import loss explicit; older/ambiguous variants refused. RGB24 export composites alpha onto white with explicit loss consent. Exact indexed-8 writer API preserves supplied palette order/indices; desktop/browser controls offer RGB24, exact indexed colors or explicit 256-color conversion with a preview |
| DCX | Studio explicit selected-page Import only | Strict bounded page directory, at most 1023 pages; each page must satisfy PCX admission. Other pages are explicitly not imported; no writer |
| PDN | Studio Open or explicit Import as a converted layered copy | Historical PDN3 normal-blend, same-sized full-canvas bitmap layers only; names/order/visibility/opacity/pixels retained, metadata loss explicit. No preserving or fresh writer |
| PAINT | Studio and Classic 11 explicit Import as a converted layered copy | Observed MIF1 `iovl`/`unci` DEFLATE RGBA8 profile only; offsets/order retained, layers numbered/visible/full-opacity because source properties are unverified. Canvas fill becomes a layer, with depth loss when needed. No writer |
| ORA | Studio explicit Import as a converted layered copy | Flat Normal PNG layers with names, signed offsets, visibility, opacity and raw pixels; requires merged preview and thumbnail. Unsupported groups, blends, extensions, ZIP64 and resource variants refused. Fractional opacity is rounded to 8 bits and explicit print resolution is discarded with consent; fresh writer implemented internally but not enabled pending external-application qualification |
| JXR, HEIF | No enabled frontend operations | Selected/backend qualification remains; HEIF writing is prohibited |
| ICO/CUR | Studio explicit selected-entry Import on desktop/browser; CLI `--entry` and MCP `entry`. Fresh multi-entry export in Studio desktop/browser, CLI `export-icons` and MCP `export_icons` | Reports every admitted entry and cursor hotspot before selection. Fresh exports use explicit sizes, RGBA PNG or 32-bit bitmap entries, independent aspect-ratio fitting, and per-entry CUR hotspots. Desktop/browser exports require a displayed preview and explicit loss consent; foreign preserving save remains unavailable |
| PSD/PSB | Studio explicit Import on desktop/browser; CLI `import` and MCP `import_image` | RGB8 Normal raster-layer subset retains names, offsets, opacity, visibility and alpha. Metadata losses need explicit consent. Groups, masks, text, effects, clipping, high depth and foreign writing remain unavailable |
| SVG/SVGZ | Studio desktop/browser Import; CLI `import` with `--svg-width`/`--svg-height`; MCP `import_image` with `svg` options | Explicit size/DPI/fitting and rasterization consent. Studio previews use bundled fonts; CLI/MCP also accept additional outline fonts. No Classic access, ordinary Open or SVG export |
| EXR/HDR, JP2, MSP, ICNS | Not yet enabled | Registry inventory or work in progress is not usable format support |
| ASE | Studio Palettes dialog; CLI `palette inspect` / `palette export`; MCP `inspect_palette` / `export_palette` | Flat groups, exact names/order/models/types and finite f32 values. RGB/gray tool-color adoption is explicit; CMYK/LAB conversion, persistent catalogs and Adobe qualification remain |
| ABR | Asset workflow policy only | Brush import implementation remains |
| CSH | Internal checked resource reader/writer; no public workflow yet | Retains bounds, names, IDs and exact ordered path records. Studio/CLI/MCP integration and independent interoperability qualification remain |
| RAW, PDF, KRA, XCF, animation, advanced DDS | Future profiles | No inherited Classic support or preserving-write claims |

QOI, PAM and farbfeld writers admit at most 4,194,304 pixels and 64 MiB encoded output. Their bounded private output is discarded on failure or cancellation. These checks do not constitute a hard aggregate resident-memory or codec scratch-memory limit.

PDN/PAINT conversions limit source acquisition to 64 MiB, layers to 256 and aggregate decoded layer pixels to 4,194,304. Unknown graph/container variants fail closed. PDN uses inert typed graph parsing without executing .NET objects. PAINT brands alone do not prove a supported document. Converted sources never become preserving Save bindings; users save the new project as PCND. Native writer qualification still requires reopen in pinned original Windows applications, beyond PaintCraft self-roundtrips.

## Loss handling and publication

Flat exports compose visible layers into a single RGBA8 image and report flattening, canvas-fill, metadata, precision, palette, alpha and hidden-RGB losses as applicable. Codec loss classification examines the actual rendered pixels. Lossless encoding does not make a flattened image a lossless project replacement.

Native and browser conversion dialogs retain source bytes and document ownership until confirmation. DCX page selection is separate from conversion-loss consent. Native launch, Open, Recent and drop, browser Open, CLI import/render and MCP all apply admission before publication. Rejected initial Open leaves an untitled document rather than adopting the rejected source as a Save destination. Suffixes are hints; signature conflicts and unsupported aliases are refused.

## Adapter and independent evidence

- `tests/extended_rasters.rs` and `tests/dcx.rs` cover exact admitted pixels, odd-width packed/planar PCX, truncation, strict boundaries, directory selection and negative admission. Explicit FFmpeg oracles independently decode QOI, Netpbm and the admitted PCX profiles.
- `tests/extended_exports.rs` checks source immutability, exact roundtrips, output/pixel bounds and cancellation. Independent FFmpeg decoding passes QOI and PAM. Official farbfeld-4 `ff2pam`, followed by FFmpeg PAM16 decode, verifies every exported 16-bit sample exactly. An 8-bit FFmpeg narrowing comparison that changed some values by one was excluded, not treated as a pass.
- `tests/specialist_rasters.rs` covers the real JPEG XL/DDS/AVIF export facade, exact loss codes, profile refusal, cancellation and source immutability. Pillow 13.0.0 independently reopens exported DDS with exact RGBA including hidden RGB. FFmpeg 8.0.1 swaps red/blue for the valid DXGI RGBA profile, so that oracle result is retained as an incompatibility rather than changing the file to satisfy it.
- Independent FFmpeg/libdav1d AVIF checks cover color and auxiliary alpha. Native and actual browser Wasm encode checks pass; writer availability does not imply reader availability.
- JPEG XL has checked-in independently generated libjxl fixtures and selected decoder/encoder tests. A fresh external `djxl` reopen was not rerun for the latest facade wiring because the tool is unavailable and installed FFmpeg has no JPEG XL decoder. Generic JPEG XL Open remains disabled pending confinement and reader qualification.
- PDN/PAINT adapter tests and `tests/project_import.rs` cover supported graph reconstruction, exact pixels/order/offsets, metadata/property losses, corruption, budgets, cancellation and PCND save/reopen. They do not establish arbitrary modern Paint.NET or Windows Paint compatibility.
- OpenRaster fixtures are generated independently by Python standard-library ZIP and PNG code. Stored/DEFLATE tests verify layer hashes, XML entities, negative offsets, exact merged-preview composition and PCND roundtrip. Prefix truncation, every observed cancellation boundary, path traversal, CRC/header disagreement, symlinks, XML property refusals and resource limits are covered. Desktop/browser/CLI/MCP conversion tests pass. External GIMP/Krita reopening and writing remain unqualified.
- Native/browser import, export and DCX page-dialog tests cover translated controls, explicit consent, stale owners and cancellation. CLI format-profile/foreign-import and MCP tests cover real command consumers, page options and loss acknowledgments. These headless checks do not replace live browser, native reference or cross-platform release testing.

Latest specialist raw oracle outputs, tool versions and excluded outcomes are retained under ignored `docs/validation/format-specialists-2026-10-25`. Source tests and checked-in fixtures remain the reproducible default evidence. Optional external-tool tests are invoked separately and are not silently counted as default-suite passes.

## Historical performance baseline

The 2026-10-24 `raster_codecs` baseline has 80 Divan cases, at least 30 samples each. The CLI Hyperfine baseline has 64 workloads, ten measured runs and two warmups each; all 640 measured executions succeeded. Independent preflight reopens 40 projects and checks pixels, source hashes and eight TIFF alpha declarations. Raw JSON, sample CSV, fixture/tool hashes and excluded observations remain under ignored `benchmarks/results/format-raster-2026-10-24`.

For those historical 1000x486 workloads, warmed CLI import medians span approximately 24 to 61 ms and export medians 18 to 39 ms. Dense GIF encoding in the earlier quiet-build baseline took approximately 293 to 333 ms, versus approximately 6 to 10 ms for different palette/sparse cases. Other user workload was uncontrolled. These are workload observations, not speedup comparisons, interactive latency measurements or measurements of the new specialist formats. Historical TIFF cells using the old alpha-ambiguous writer remain retained but are superseded for current-profile qualification.

No PNG/JPEG backend has been switched to Zune, and no performance improvement is claimed merely from adding a dependency.

## Remaining interoperability work

The ICO/CUR adapter admits at most 256 entries and 64 MiB of encoded input. It checks all entry dimensions and payload ranges before decoding the explicitly selected entry. Its initial profile covers noninterlaced eight-bit PNG entries with a restricted chunk set and uncompressed 1/4/8/24/32-bit BITMAPINFOHEADER entries. Private gaps, suffixes, conflicting metadata, compressed bitmaps, and ambiguous legacy 32-bit alpha are refused. Cursor hotspots remain in the decoded entry DTO, with an explicit loss when converting into a flat document. Pillow 13.0.0 independently decoded all six authored ICO fixture entries to exact expected RGBA hashes; CUR payload identity and hotspot fields were independently checked as bytes. Default Rust tests cover both containers, loss codes, entry identity, directory reordering, pixel budgets, truncation, conflicting geometry, CRC failures, and alpha refusals. Desktop/browser headless English/Japanese controls verify explicit selection and separate consent; stale/cancelled imports preserve the source document. All seven language catalogs contain the new controls and loss descriptions. These import checks do not qualify original Windows cursor behavior or preserving saves. Fixture generation and independent verification are documented in `crates/services/paintcraft-formats/tests/icon-fixtures/README.md`.

Fresh ICO/CUR export uses the selected `ico` container library with explicit RGBA PNG and 32-bit bitmap payloads. The 2026-10-25 Pillow 13.0.0 check matched all 24 exported entry buffers, including fully transparent and opaque PNGs; it separately verified cursor hotspots and bitmap masks. Preparation renders one immutable source, fits each requested size independently onto transparent padding, and offers nearest or premultiplied encoded-sRGB Lanczos3 resampling. Source and aggregate destination admission are each at most 4,194,304 pixels; output is at most 64 MiB and 256 entries. Resize, flattening and hidden-RGB loss need explicit consent before encoding. CLI/MCP retain source, destination, state and history guards. Studio desktop/browser share translated entry controls, transparency previews and cursor hotspot markers. A changed source or configuration requires a new displayed preview and renewed loss consent. Native export retains the previewed bytes through a parented rfd picker and verifies an existing destination fingerprint before replacement; imported source provenance follows its document across tabs. Browser export downloads immediately without changing the project name or saved state. Fully transparent bitmap output, original Windows application qualification and foreign preserving save remain pending. No hard upstream scratch-memory, cancellation-tail or performance guarantee is claimed.

- Qualify JPEG XL reader confinement and cancellation tails, JXR independent comparisons and a decoder-only HEIF profile.
- Finish and qualify extended document/asset adapters, including ORA groups and layered writing, PSD/PSB, SVG rasterization with explicit dimensions/DPI, legacy icon alpha and Adobe resources. EXR/HDR require precision-aware storage/import gates; JP2 and historical MSP need dedicated evidence.
- Add separately qualified PDN/PAINT fresh and preserving writers, source-version coverage and original-application reopen evidence.
- Complete full browser/native reference and packaging matrices, worker failure and aggregate-memory accounting, and real-device cancellation/latency qualification.

The implemented subsets advance the pack; they do not complete its full interoperability, preservation or release gates.

### Adobe Swatch Exchange assets, 2026-10-25

The selected `ag-psd` 0.3.0 now reads and writes bounded ASE 1.0 palette
resources. A complete preflight verifies exact block extents, strict UTF-16
names and terminators, known models/types and balanced flat groups before the
upstream reader runs. Nested groups, unknown block data, suffixes and nonfinite
channels are refused. Admission limits are 1 MiB encoded data, 8192 blocks,
4096 swatches, 1024 groups, 1024 UTF-16 units per name and 256 KiB aggregate
name data. Checks surround codec calls and run per resource item; they are not
a hard upstream allocation or intra-call cancellation boundary.

RGB, CMYK, Gray and LAB channels retain their finite f32 bits. This operation
does not interpret profiles or convert colors to display RGB. Root/group order,
duplicate names, empty groups, Global/Spot/Normal types, Japanese/non-BMP names
and negative zero are covered by authored fixtures. An independent `swatch`
0.4.0 parser verified all six mixed-fixture swatches and actual CLI export,
including every channel bit. A separately `swatch`-written fixture is checked
by the Rust decoder and writer. Provenance, commands and limits live in
`crates/services/paintcraft-formats/tests/ase-fixtures/README.md`.

CLI and MCP inspect typed resources and safely write fresh ASE copies with
input SHA guards, source-alias protection and bounded staging. Resource
inspection has a published `palette-inspection` schema. Real subprocess and
MCP stdio tests cover destination refusal, malformed/oversized input, policy
before acquisition and unchanged painting/history/dirty state. These commands
do not create a document, adopt a project destination or acknowledge a painting
as saved. Classic and all ordinary image/project Open, conversion and Save
routes remain denied for ASE.

Studio desktop/browser now share a translated Palettes dialog, available in
File and the Colors window. It displays groups, names, model/type and channel
values, with virtualized rows. Picking normalized RGB/gray explicitly rounds
to an opaque 8-bit sRGB primary/secondary tool color. CMYK, LAB and out-of-range
samples remain retained/exportable but cannot be picked without color-managed
conversion. The imported resource is frozen separately from document state and
survives closing/reopening the dialog; it is not yet a persistent asset catalog.

Desktop import/export uses parented native rfd pickers. Export retains prepared
palette bytes during destination selection, checks the resource generation and
frontend, refuses source aliases including open documents in other tabs/parked
workspaces, and uses the bounded expectation-checked save coordinator. Browser
file reads are capped at 1 MiB and late picker results are generation-checked;
export requests a download without adopting a project destination or changing
the title/dirty flag. Failed or cancelled imports keep the previous palette.
English/Japanese headless controls and a 390 × 640 layout check pass. All seven
locale catalogs include the new controls and adoption-policy text.

The local production Wasm build was exercised in headless Chrome: importing
`mixed.ase`, exporting its copy and picking an RGB swatch succeeded without page
errors. The 282-byte download matched the original exactly (SHA-256
`3b3b96a8bd4ab40fe22829e9bef3ae868b61d51c1a2a41aec81416013d6788bf`), including
all model/type/channel values verified by the independent parser. The title
remained `Untitled - PaintCraft` and dirty stayed false; the selected tool color
became `[64,128,255,255]` while the canvas remained unchanged. This single scene
does not qualify every platform or original Adobe application. Color-managed
adoption, persistent catalogs and original-app reopen evidence remain pending;
this is not complete F14 or release qualification.

The [ASE workload baseline](benchmarks/palette-assets-2026-10-25/BASELINE.md)
now retains calibrated Criterion samples, Divan allocation statistics and
three Hyperfine CLI sessions. Independent pre/postflight checks cover all
4,358 swatches in the four workloads. On the local Apple M1 Pro, the 4096-color
Criterion decode/encode session medians span 1.146–1.174 ms / 0.948–0.984 ms;
complete CLI inspection/export span 9.77–13.72 ms / 20.44–30.57 ms. All 480
measured CLI executions succeeded. The ranges retain slower sessions and are
observations of these resource workflows, not speedup or interactive guarantees.

### CSH resource adapter, 2026-10-25

The internal CSH adapter preflights the complete source before invoking the
selected `ag-psd` 0.3.0 reader. Its typed resource keeps original unsigned
top/left/bottom/right bounds, Unicode names, ASCII IDs, Boolean operations,
open/closed subpaths, linked/unlinked knots, initial fill and clipboard records.
Normalized coordinates retain their exact signed 8.24 integers. The adapter
cross-checks decoded geometry against the backend rather than replacing it with
an approximate raster. The [Adobe path specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/)
documents the coordinate and record representation.

The current admitted profile is CSH version 2, shape version 1, at most 4 MiB,
256 shapes, 65,536 aggregate path records, 1,024 UTF-16 units per name and
256 KiB aggregate encoded name bytes. Truncated extents, invalid UTF-16,
inverted unsigned bounds, inconsistent knot counts/topology, unknown operations,
unsupported flags, nonzero reserved data and repeated control records are
refused. Duplicate names/IDs and record order are retained. These input and
structure allowances do not prove a hard allocator or cancellation-tail bound.
The fresh writer uses selected `ag-psd` container/name/ID headers with an exact
ordered-record body adapter: the upstream path writer omits selector 6, reorders
control records, resets the origin and converts coordinates through pixel-space
floats. PaintCraft retains the original bounds and signed fixed-point values
instead. Name terminators/padding are canonicalized. Constructed resources pass
the same wire validation, and every export is decoded and compared with its
typed source before bytes are returned. Tests cover all Boolean operations,
extreme bounds, record byte equality and cancellation at every checkpoint.
This adapter is not registered for any public operation yet. Resource
editing/placement, previews, frontend integration, independent-reader evidence
and original-application qualification remain required for F14.

### SVG/SVGZ rasterization, 2026-10-25

The selected resvg/usvg 0.48.1 now has an explicit-size/DPI rasterization adapter
in `paintcraft-formats::svg`. CLI `import` and MCP `import_image` now expose it
through explicit size/DPI options and named rasterization-loss consent. Studio
desktop/browser Import now opens shared size/DPI/fitting controls and requires a
preview and conversion consent. Generic import discovery and ordinary Open never
guess SVG parameters. Classic SVG access and SVG export remain denied.

Preparation requires output dimensions, DPI and contain/stretch fitting. It
accepts UTF-8 SVG or a single checked SVGZ gzip member, limits encoded and
inflated source to 8 MiB, XML/expanded references to 16,384 nodes, nesting to 64,
individual dimensions to 16,384 and output to 4,194,304 pixels. Paths, styles,
text, font data, embedded images and filter complexity have additional admission
limits. DTDs, processing instructions, scripts, animation, foreign renderable
elements, external URLs/files, CSS escapes/at-rules, advanced color spaces and
unqualified computed CSS values are refused.

Fonts come only from explicitly supplied memory, with a 64 MiB/64-face allowance.
No system font search runs inside the adapter. Family substitution is reported;
missing glyphs refuse preparation. Embedded base64 PNG/JPEG/still GIF/WebP images
pass the existing bounded raster reader, then become private metadata-free PNGs
for resvg. Metadata/depth/palette losses from that conversion remain named. Both
data and string resolvers deny everything outside the admitted image cache.
The result is straight RGBA8 in encoded sRGB; source vector/text editability is
lost and reported as `svg-rasterized`.

CLI/MCP use the shared root Selawik regular/bold assets and may read up to 30
explicitly supplied outline font files. TTC face counts, directories, offsets
and expanded name metadata are bounded before fontdb reserves or parses faces.
Embedded SVG, bitmap and color glyph tables are refused because their upstream
rendering paths bypass the document's admitted resource resolver/cache. Partly
unreadable collections are refused rather than silently omitting faces.

Raster document identity includes source digest, dimensions, DPI, fitting and
rendered samples, so changing options or effective fonts cannot reuse an unrelated
state identity. PCND roundtrips preserve all raster samples. CLI/MCP tests cover
SVGZ, fitting, physical-unit DPI coverage, required/mutually exclusive options,
named loss consent, shared fonts, supplied Japanese fonts, unchanged failure
state and source/font alias protection after saving. Incompatible profiles and
missing SVG parameters are refused before source acquisition.

Studio's controls start at an explicit 512 × 512 pixels, 96 DPI, preserving aspect
ratio on transparent padding. They use the shared root Selawik and Noto fonts;
additional font-file selection is currently available only in CLI/MCP. Preview
work runs only when requested, samples the full selected raster, and retains the
exact private converted document for import. Changing options clears the preview
and consent. Import remains disabled until the preview has been displayed and
the user accepts the reported conversion losses. Cancellation or a changed
document/profile cannot publish the prepared copy. Native SVG acquisition and
browser file-size checks use the 8 MiB SVG source allowance before loading bytes.

Headless English/Japanese controls tests cover desktop and browser hosts; a
390 × 640 browser-host test verifies reachable consent and visible footer buttons.
A local production Wasm build was checked in headless Chrome: explicit Studio
Import of `aligned.svg`, preview, consent, updated title and PCND download all
succeeded without page errors. At 512 × 512, 96 DPI and contain fitting, its
177-byte downloaded PCND was byte-identical to the native CLI output (SHA-256
`2143ab6e67f775b5e207da858c0084b150380394cc3da46ca00a2412bcfc81bd`).
This single browser scene is workflow evidence, not full browser/OS qualification.

Nine authored scenes have independent headless Chrome reference pixels and
source/font hashes. Aligned geometry and nearest-sampled embedded pixels match
exact RGBA. Six other scenes pass measured premultiplied-channel tolerance checks;
their mean absolute differences range from 0.165365 to 1.783875 on an 8-bit scale.
A ninth CSS-smooth discrepancy is retained separately because Chrome reports that
value unsupported; it is not agreement evidence. The corpus found and now guards
the usvg pixelated-presentation-attribute compatibility fix. Commands, per-scene
statistics and limitations are in
`crates/services/paintcraft-formats/tests/svg-fixtures/README.md`.

Tests also cover truncated/corrupt/concatenated SVGZ, oversized expansion, XML
budgets, cyclic/exponential references, filter limits, unsupported resources and
cancellation at every adapter checkpoint. Render scratch/work estimates reject
excessive jobs but do not impose a hard allocator/process limit. Upstream XML,
font, image and rendering calls have no cooperative cancellation hook inside a
call; no hard cancellation-tail or performance guarantee is claimed. These
remaining confinement, font-selection and platform qualification gates are still
part of the roadmap. Studio preview preparation currently runs synchronously;
no UI latency guarantee is claimed for an admitted complex source.

### PSD/PSB conversion, 2026-10-25

The selected `ag-psd` 0.3.0 has a bounded preflight and private native-document
conversion adapter. Studio desktop/browser expose this through explicit Import;
CLI `import` and MCP `import_image` use the same conversion. Ordinary Open,
Classic profiles and foreign preserving/fresh writes remain unavailable.

The initial profile accepts eight-bit RGB, Normal raster layers, explicit alpha,
Unicode names, signed offsets, opacity and hidden state. Raw, PackBits RLE, ZIP
and ZIP prediction channels are checked for exact decoded size before library
decoding. Admission caps encoded input at 64 MiB, metadata at 1 MiB, layers at
256, and both canvas pixels and aggregate normalized layer pixels at 4,194,304.
Masks, groups, clipping, text, effects, non-Normal blending, extra channels,
non-default fill opacity, high depth and unsupported metadata extensions are
refused. Image resources are bounded and omitted from the private decoder view;
their loss is reported, with ICC omission identified separately. Source numeric
layer IDs are remapped to deterministic native UUIDs using file digest and record
index. No foreign preserving Save or writer is enabled.

Layered sources use their actual editable raster pixels, not the merged preview.
Layerless sources become a single composite layer. A private representation
adapter uses the selected library's ZIP channel decoder for single-stream merged
previews that its whole-file reader cannot directly read. Original bytes remain
unchanged. The sixteen independently authored PSD/PSB fixtures were verified with
psd-tools 1.26.0 and Pillow 13.0.0, including exact layer and composite pixels.
Tests cover those fixtures, every truncated prefix, oversized ZIP expansion,
malformed RLE, budget failures, unsupported semantics, cancellation at every
conversion checkpoint and exact PCND roundtrips. Fixture provenance and commands
are in `crates/services/paintcraft-formats/tests/psd-fixtures/README.md`.

Checks surround backend work and run between layer decodes. There is no upstream
cancellation callback within a channel decode, and these checks do not establish
a hard aggregate process-memory or cancellation-tail limit. Original Photoshop
qualification, advanced layer semantics and fuller metadata retention remain
separate follow-up work. All seven locales include the new loss descriptions.
Headless English/Japanese tests exercise actual desktop/browser confirmation
controls, cancellation, stale owners, unchanged history before consent, layer
identity and PCND preservation. Real CLI and MCP protocol tests require named
loss consent and reject writes through original-source filesystem aliases.

The production Wasm build was also exercised in headless Chrome on 2026-10-25:
Studio imported the ZIP-predicted PSD and RLE PSB fixtures through the browser
picker, displayed the metadata confirmation, and downloaded 262-byte PCND copies.
Both downloaded documents matched native CLI conversions byte-for-byte after
canonical encoding, including every layer sample and document field. The browser
reported no page errors. The built asset maximum was 17,438,894 bytes against the
26,214,400-byte deployment limit. This verifies the local built application, not
a deployed site or Photoshop behavior. Machine-readable outcomes remain under
ignored `docs/validation/format-specialists-2026-10-25/psd-browser-results.json`.

### OpenRaster writer follow-up, 2026-10-25

The fresh-copy writer now streams ZIP32 members with PNG layer pixels, Normal
layer metadata, merged preview and a fitted, non-upscaled thumbnail. Unicode and
XML whitespace roundtrip, all 256 layer-opacity values retain exact 8-bit values,
and document-title omission requires the named loss acknowledgment before output.
Geometry, root identity and metadata admission precede serialization; output and
member budgets, cancellation and failed sinks discard private results. Standard
unit tests, strict Clippy, real Wasm-target checking and an independently executed
Python ZIP/XML/PNG oracle pass.

The GIMP oracle must verify actual layer pixels/properties, edit layers, save to
XCF and reopen those edits. Official SHA-256-verified macOS GIMP 3.0.6 and 3.0.8
revision 1 reference packages stalled during bundled Python plugin initialization
before loading the exported file. These startup failures are excluded compatibility
evidence, not decoder failures. Writer admission stays unqualified; there is no
frontend writer or foreign preserving-save claim. The opt-in bounded harness and
its requirements are documented in `tests/ora-fixtures/README.md`.

### PCX writer delivery, 2026-10-25

The selected `pcx` 0.2.5 writer now powers Studio, CLI and MCP RGB24 export.
Classic export filters remain unchanged. Native/browser export dialogs include the
PCX choice and translated description in all seven languages. Transparency uses
an exact white matte and requires `alpha-discarded` consent; exports leave source
pixels, history and dirty state unchanged. No foreign preserving Save is offered.

Both RGB24 and indexed-8 APIs admit at most 4,194,304 pixels, width 65,534, height
65,535 and 64 MiB encoded output. Width 65,535 would overflow the codec's padded
u16 row field and is rejected before entering it. Row padding, literal escape
values, long RLE runs, source/index lengths, positive DPI, cancellation and invalid
palette references are checked. Indexed output accepts explicit original indices
and palette entries, including duplicate colors; it never silently quantizes RGBA.
Studio desktop and browser now offer indexed palette controls and a fitted preview.

CLI and MCP now additionally expose explicit indexed policies. CLI `render`
accepts `--pcx-encoding rgb24|indexed-exact|indexed-256`; MCP `export_image` uses
the equivalent optional `pcx_encoding` field. RGB24 stays the default. Exact
indexed mode refuses over 256 post-matte colors; indexed-256 preserves exact
small palettes and otherwise uses the existing `color_quant` NeuQuant backend.
Only actual changed colors add `palette-quantized`; alpha loss remains separate.
Wrong-format options fail before rendering/writing. Serial and parallel CLI
exports match, and actual MCP tests preserve session state and original files.

Preparation owns the exact palette/indices used by both its fitted nearest-sample
preview API and the encoder. Independent Pillow 13.0.0 and FFmpeg 8.0.1 reopen
the quantized result with exactly those preview pixels. Quantizer training is
bounded to 32,768 evenly distributed source samples, with cancellation checks on
both sides of the upstream call. This bounds training input, not wall-clock time
or total process memory. Source/mapping/preview loops check cancellation in bounded
chunks. Desktop/browser palette controls are connected and retain RGB24 as the default. The RGB24 benchmark below does not measure
these new indexed policies.

The explicitly invoked Pillow 13.0.0 and FFmpeg 8.0.1 oracle reopens twelve exports
across odd/even widths 1, 7, 63, 64, 65 and 257. All RGBA samples match; Pillow also
retains every index byte and all palette entries/order. These checks establish the
qualified codec subsets, not universal historical Paint/Paintbrush compatibility.

The PCX benchmark adds sixteen encode/decode workload cells with 30 samples each.
After independent Pillow/FFmpeg verification of the actual eight fixture files,
Apple M1 Pro 1000×486 RGB24 export medians span 4.794 to 16.78 ms; decode medians
span 6.409 to 11.33 ms. The dense-alpha export includes the white matte. These are
codec baseline observations under uncontrolled host workload, not speedup or
interactive-latency claims. Raw fixtures/hashes, allocation output, final timing
log and excluded build/concurrent-run observations remain under ignored
`benchmarks/results/pcx-writer-2026-10-25`.

### Palette frontend follow-up, 2026-10-25

The shared PCX encoding control offers RGB24, exact indexed colors and explicit
up-to-256-color conversion. Both Studio frontends show a cached fitted preview
and a color-loss notice when conversion changes colors. Unavailable previews
disable export; a new source/policy must be displayed for a frame before the
export button becomes actionable. Controls, descriptions and warnings are
translated in all seven languages. Classic export admission remains unchanged.

Desktop captures the encoding policy across native destination selection and
deferred text placement into the final loss review. The suggested format's filter
comes first. Changed picker sources or stale loss confirmations cannot export
pixels the preview did not show. Private text preview snapshots match actual
placement while preserving the editable draft, document and undo history. Browser
download options carry the chosen policy into the admitted encoder, including
private editable-text snapshots. Closing the modal releases preview textures.

Actual English/Japanese headless controls exercise exact-mode refusal, quantized
preview, cancellation and native output. Picker/source guards, browser encoder
output, independent palette-preview comparisons, font/catalog coverage and strict
Clippy pass. Production Wasm/Astro builds pass; these checks do not establish
Windows reference UI fidelity, every device or original Paintbrush compatibility.
