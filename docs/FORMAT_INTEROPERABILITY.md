# Format interoperability implementation

This ledger follows `docs/temp/paintcraft-format-interoperability-classic-studio-v3` alongside the canonical editor and PCND specifications. It records implemented operations as of 2026-10-26, not completion of the entire planning pack.

Codec implementation, enabled frontend operations and release qualification are separate gates. `paintcraft_formats::registry` controls discovery and admission; individual adapters enforce their narrower file profiles and resource limits. A listed format does not imply support for every variant, metadata field or original application version.

## Available workflows

Desktop and browser share the frontend registry, with platform-specific qualification for each operation. Native-only JPEG XR, HEIF, AVIF and HDR/EXR conversions therefore do not imply browser readers; JPEG XL has its own explicitly qualified browser import. CLI and MCP import/export operations also apply that registry, using an explicit frontend profile rather than inheriting every compiled codec. All foreign conversions require acknowledgment of their reported losses before publication. Failed, cancelled or stale conversions leave the existing document intact.

- PCND is the native layered project format and the preserving Save destination.
- Classic 7/10/11 open and export PNG, JPEG, BMP, still GIF and single-page TIFF. CLI Classic profiles can explicitly import the admitted PCX and MSP subsets; the Classic 11 CLI profile also admits the limited PAINT conversion. Desktop/browser Import image is currently a Studio action, so these headless allowances do not imply additional Classic menus.
- Studio also opens TGA, still WebP and admitted 2D DDS top-mip images with loss review when lower levels are omitted, and opens the admitted historical PDN subset as a converted copy.
- Studio's explicit Import image workflow adds QOI, Netpbm, farbfeld, PCX, historical monochrome MSP, selected DCX pages, OpenRaster layers with a limited neutral-group conversion and the admitted PAINT subset. These extended imports do not silently expand ordinary Open or Recent filters.
- Studio offers **15 flat exports**: PNG, JPEG, BMP, GIF, TIFF, TGA, WebP, JPEG XL, DDS, AVIF, QOI, PAM, farbfeld, PCX with RGB24 or indexed palettes, and exact opaque black/white MSP1. Native Studio, CLI and MCP can also explicitly select MSP2 row-RLE. PAM output always uses `.pam`, never a `.ppm`, `.pgm`, `.pbm` or generic `.pnm` filename.
- Native Studio, CLI and MCP offer explicit DDS texture copies: uncompressed RGBA8 or lossy BC1/BC2/BC3, sRGB or converted linear8 samples, and generated mip levels. Native Studio previews the decoded base image and reviews losses before choosing the output. The browser retains its existing single-level RGBA8 exporter; advanced browser texture export is not enabled.
- DCX requires a page choice. Desktop/browser stage a page chooser before decoding and loss confirmation; CLI `import --page` and MCP `import_image` use an explicit zero-based page index. There is no implicit page-zero import.
- ICNS import requires a specific entry with logical size, Retina scale and physical pixels shown separately. Studio desktop/browser preview and review import/export losses; fresh export offers only eight qualified PNG types. CLI/MCP use the same explicit entry/type policy; ordinary Open and preserving ICNS Save stay denied.
- Studio explicitly imports the narrow metadata-free sRGB8 JPEG XL profile through a verified packaged native helper or the bounded browser Worker. CLI and native MCP use the native route. Ordinary Open and Classic JPEG XL reads remain denied; strict profiles without conversion losses do not invent an approval prompt.
- Native Studio, CLI and native MCP explicitly import the qualified opaque RGB8 JPEG XR profile with mandatory print-resolution loss consent. Studio displays the actual picture and effective horizontal/vertical DPI before approval. Ordinary Open, browser and Classic JPEG XR reads remain denied.
- Native Studio, CLI and native MCP explicitly import the qualified direct-primary HEIF sRGB8 color/straight-alpha profile. Actual YCbCr reconstruction and nonempty item-name losses require consent; an unnamed identity-color source has no conversion-loss prompt. Ordinary Open, browser, Classic and all HEIF writing remain denied.
- Native Studio, CLI and native MCP explicitly import the qualified identity sRGB8 and explicit SDR YCbCr AVIF still/straight-alpha profiles. SDR requires a verified V6/V7 helper and reconstruction consent; identity accepts V5/V6/V7. Nonempty source item names also require consent. Ordinary Open, browser and Classic reads remain denied.
- Native Studio, CLI and native MCP convert the admitted Radiance HDR and OpenEXR subsets to 8-bit sRGB with explicit exposure, curve, source/alpha interpretation and actual loss consent. Studio reviews the immutable picture before adoption; CLI/MCP inspect first and require the exact source and assessment hashes for import. This is not HDR editing or preserving foreign Save.

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
| DDS | Studio 2D top-mip import; default single-level RGBA8 export; native Studio/CLI/MCP explicit RGBA8 or BC1/BC2/BC3 texture copies with generated mips | Reader requires explicit sRGB and straight/opaque alpha, at most 64 MiB and 4,194,304 top-level pixels; lower levels are structurally checked and omitted with consent. Advanced fresh DX10 output offers sRGB or real linear8 conversion, single/full/custom mip counts, Nearest or stored-sample Box, and Normal/High BC effort. BC1 requires opaque input; BC2/BC3 use straight alpha. BC, actual hidden-RGB and linear8 quantization losses need consent. Browser advanced export, BC1 cutout, other BC formats/swizzles, custom mip preservation and foreign preserving Save remain unavailable; arrays, cubes and volumes are refused. See the DDS texture-copy profile below |
| JPEG XL | Studio lossless RGBA8 Export; native Studio/CLI/MCP explicit Import for `native-jxl-srgb8-worker-v1`; browser Studio explicit Import for `web-jxl-srgb8-worker-quarantine-v1` | Both readers require metadata-free static identity-oriented straight sRGB8, at most 64 MiB input and 4,194,304 pixels. Native import requires the matching packaged helper/schema 2 manifest. Browser import uses the production isolated Worker and persistent quarantine on uncertain completion. Ordinary Open and Classic reads remain denied. Export accepts 1-pixel dimensions and retains its 64 MiB output limit |
| AVIF | Studio lossy RGBA8 Export; native Studio/CLI/MCP explicit Import for identity sRGB8 and SDR YCbCr variants | Reader: reduced-still identity 4:4:4 or explicit BT.709/sRGB8 full/limited-range 4:4:4 or 4:2:0 with qualified vertical/colocated siting, optional straight alpha, at most 64 MiB input and 4,194,304 pixels. SDR requires an exact V6/V7 helper/manifest and reconstruction consent; identity accepts V5/V6/V7. Named item loss also requires consent. Unknown 4:2:0 siting, HDR, grids, browser/Classic/Open and preserving Save remain denied. Export is lossy even at quality 100, may change alpha/hidden RGB, and retains its 1,048,576 pixels, 4096 per side and 16 MiB output limits |
| QOI | Studio explicit Import and lossless RGBA8 Export | Strict complete raster/end marker and sRGB profile; linear declaration refused; exact alpha and hidden RGB retained by the codec |
| Netpbm | Studio explicit P1 through P7 Import; PAM P7 RGBA8 Export only | Admitted max values 1, 255 or 65535 and recognized grayscale/RGB/alpha tuples; bounded 64 KiB header. High-depth import reduces to 8 bits; metadata loss explicit |
| farbfeld | Studio explicit Import and lossless RGBA8 Export | Exact body length required; imports reduce 16-bit samples to 8 bits with explicit loss. Exports widen each 8-bit sample by multiplying by 257; this does not add high-depth editing |
| PCX | Classic/Studio explicit Import; Studio RGB24 flat Export | Version 5 RLE: admitted packed 1/2/4/8-bit, planar 1-bit and RGB24 profiles; minimal even row padding; strict row/plane bounds. Palette and metadata import loss explicit; older/ambiguous variants refused. RGB24 export composites alpha onto white with explicit loss consent. Exact indexed-8 writer API preserves supplied palette order/indices; desktop/browser controls offer RGB24, exact indexed colors or explicit 256-color conversion with a preview |
| DCX | Studio selected-page Import; native CLI/MCP explicit single-page fresh copy | Strict bounded directory and typed RGB24/indexed8 multi-page backend; headless fresh copy uses one document composite and explicit PCX pixel policy. No preserving Save or desktop/browser export controls |
| PDN | Studio Open or explicit Import as a converted layered copy | Historical PDN3 normal-blend, same-sized full-canvas bitmap layers only; names/order/visibility/opacity/pixels retained, metadata loss explicit. No preserving/public fresh writer; internal typed PDN3 writer and bounded native preparation are implemented, pending original-app qualification |
| PAINT | Studio and Classic 11 explicit Import as a converted layered copy | Observed MIF1 `iovl`/`unci` DEFLATE RGBA8 profile only; offsets/order retained, layers numbered/visible/full-opacity because source properties are unverified. Canvas fill becomes a layer, with depth loss when needed. Internal fresh writer and gated layered-copy preparation exist; public/preserving writes await Windows Paint qualification |
| ORA | Studio explicit Import as a converted layered copy | Flat Normal PNG layers with names, signed offsets, visibility, opacity and raw pixels; requires merged preview and thumbnail. A separate explicit Studio Import variant converts exact neutral `auto` Normal groups to ordered editable leaves with group-structure/name/control loss consent. Isolated/default groups, nonneutral opacity, group offsets, unsupported blends/extensions, ZIP64 and resource variants are refused. Fractional opacity is rounded to 8 bits and explicit print resolution is discarded with consent; fresh writer implemented internally but not enabled pending external-application qualification |
| JPEG XR | Native Studio explicit Import, CLI and native MCP `import_image` for `native-jxr-srgb8-worker-v1`; optional qualified V8 adds upfront long-header `native-jxr-writer-rgb8-worker-v1`. Aliases: `.jxr`, `.wdp`, `.wmp` | Strict unsigned opaque RGB8, at most 64 MiB input and 4,194,304 pixels, through the verified same-build helper. Print resolution is discarded only with consent, including inferred 96 dpi. The exact native Studio fresh-copy RGB8 writer and distinct long-header importer have actual headless English/Japanese Studio and CLI/MCP proof through qualified V8; staged Apple-silicon macOS release/source-installer selection is locally qualified with optimized binaries, while the other five targets and global Cargo default retain V7. No generic export, ordinary Open, Classic/browser read or preserving Save |
| HEIF/HEIC | Native Studio explicit Import, CLI and native MCP `import_image` for `native-heif-srgb8-worker-v1`, with `.heic`, `.heif` or `.hif` aliases | Direct-primary explicit sRGB8 color and optional straight8 alpha through the verified helper. YCbCr reconstruction and nonempty source item names require their actual loss consent. No ordinary Open, Classic/browser read, grid/ICC/HDR/deep color, preserving Save or HEIF writing |
| ICO/CUR | Studio explicit selected-entry Import on desktop/browser; CLI `--entry` and MCP `entry`. Fresh multi-entry export in Studio desktop/browser, CLI `export-icons` and MCP `export_icons` | Reports every admitted entry and cursor hotspot before selection. Fresh exports use explicit sizes, RGBA PNG or 32-bit bitmap entries, independent aspect-ratio fitting, and per-entry CUR hotspots. Desktop/browser exports require a displayed preview and explicit loss consent; foreign preserving save remains unavailable |
| PSD/PSB | Studio explicit Import on desktop/browser; CLI `import` and MCP `import_image` | RGB8 Normal raster-layer subset retains names, offsets, opacity, visibility and alpha. Metadata losses need explicit consent. Groups, masks, text, effects, clipping and high depth remain unavailable. Internal fresh RGB8 layered writer has independent raw-layer/profile proof; public writing remains disabled pending original-application qualification |
| SVG/SVGZ | Studio desktop/browser Import; CLI `import` with `--svg-width`/`--svg-height`; MCP `import_image` with `svg` options | Explicit size/DPI/fitting and rasterization consent. Studio previews use bundled fonts and additional font files; CLI/MCP accept outline fonts too. No Classic access, ordinary Open or SVG export |
| MSP | Explicit Studio import and fresh MSP1 export in desktop/browser/CLI/MCP; native Studio/CLI/MCP also select MSP1 or MSP2; headless Classic legacy import | DanM raw and admitted LinS row-RLE profiles with independent Pillow checks. Explicit versions use one monochrome composite and retained two-stage admission. No implicit threshold, dither, matte or alpha removal; actual flatten/hidden-RGB losses require consent. Native Studio has version selection and an actual encoded preview. Browser UI version selection, original Windows Paint compatibility and preserving Save remain unqualified |
| ICNS | Studio desktop/browser explicit selected-entry Import and fresh typed export; CLI `inspect-icns` / `import --entry` / `export-icns`; MCP `inspect_icns` / `import_image` / `export_icns` | Admitted PNG and small RGB/mask readers; eight explicit PNG output types. Logical size, Retina scale and physical pixels stay distinct. Displayed preview and loss consent required in Studio. No ordinary Open, preserving Save, Classic access, legacy writer or canonical ARGB support; native readback exclusions remain |
| EXR/HDR | Native Studio explicit conversion; CLI `inspect-float` / `import-float`; MCP `inspect_float` / `import_float` | Verified final V7 helper, at most 16 MiB source and 1,048,576 pixels. Admitted RGBE or single-part scanline RGB[A] HALF/FLOAT becomes 8-bit sRGB under explicit settings and actual loss consent. No ordinary Open, Classic/browser conversion, HDR editing, foreign writer or preserving Save; broader source profiles remain unqualified |
| JP2 | Not yet enabled | OpenJPEG remains a candidate; deferred F7 import requires its own bounded decoder, color/loss policy and independent consumer qualification |
| ASE | Studio Palettes dialog; CLI `palette inspect` / `palette export`; MCP `inspect_palette` / `export_palette` | Flat groups, exact names/order/models/types and finite f32 values. RGB/gray tool-color adoption is explicit; CMYK/LAB conversion, persistent catalogs and Adobe qualification remain |
| ABR | Studio desktop/browser selection and bounded native-static R8 painting; CLI/MCP inspection and painting | Exact sample masks, pattern channels, signed bounds and original ordered payloads retained. Up to 256 presets per admitted library; unsupported dynamics, textures, transforms and precision reduction are refused. Full preset behavior and original-application qualification remain; no ABR export or ordinary image Open |
| CSH | Studio Shape resources dialog with numeric placement and a canvas drag tool; CLI `shape inspect` / `shape export`; MCP `inspect_shapes` / `export_shapes` | Retains bounds, names, IDs and exact ordered path records. Supported single closed paths rasterize with curves and explicit consent in Studio. Multiple/open paths, Boolean placement and broader interoperability remain |
| RAW, PDF, KRA, XCF, animation | Future profiles | No inherited Classic support or preserving-write claims |

QOI, PAM and farbfeld writers admit at most 4,194,304 pixels and 64 MiB encoded output. Their bounded private output is discarded on failure or cancellation. These checks do not constitute a hard aggregate resident-memory or codec scratch-memory limit.

PDN/PAINT conversions limit source acquisition to 64 MiB, layers to 256 and aggregate decoded layer pixels to 4,194,304. Unknown graph/container variants fail closed. PDN uses inert typed graph parsing without executing .NET objects. PAINT brands alone do not prove a supported document. Converted sources never become preserving Save bindings; users save the new project as PCND. Native writer qualification still requires reopen in pinned original Windows applications, beyond PaintCraft self-roundtrips.

## Loss handling and publication

Flat exports compose visible layers into a single RGBA8 image and report flattening, canvas-fill, metadata, precision, palette, alpha and hidden-RGB losses as applicable. Codec loss classification examines the actual rendered pixels. Lossless encoding does not make a flattened image a lossless project replacement.

Native and browser conversion dialogs retain their prepared candidate and document ownership until confirmation. DCX page selection is separate from conversion-loss consent. DDS top-mip imports use the existing desktop/browser loss reviewer, including ordinary Studio Open. CLI import/paste require `--allow-loss dds-other-mips-not-imported`, and MCP `import_image` requires the same named code in `allow_losses`. Accepted mip omission creates an unsaved raster copy with no preserving DDS binding; one-level Open adds neither an omission loss nor an unsaved-conversion prompt. Native and headless consumers retain captured source identities, and native review rechecks the raw digest before adoption; source/alias guards remain after a PCND Save. These guards cover normal source selection, hardlinks and renames, not hostile namespace changes during staged Save publication; the shared final publication-callback improvement remains separate work. Native launch, Open, Recent and drop, browser Open, CLI import/render and MCP all apply admission before publication. Rejected initial Open leaves an untitled document rather than adopting the rejected source as a Save destination. Suffixes are hints; signature conflicts and unsupported aliases are refused.

## Adapter and independent evidence

- `tests/extended_rasters.rs` and `tests/dcx.rs` cover exact admitted pixels, odd-width packed/planar PCX, truncation, strict boundaries, directory selection and negative admission. Explicit FFmpeg oracles independently decode QOI, Netpbm and the admitted PCX profiles.
- `tests/extended_exports.rs` checks source immutability, exact roundtrips, output/pixel bounds and cancellation. Independent FFmpeg decoding passes QOI and PAM. Official farbfeld-4 `ff2pam`, followed by FFmpeg PAM16 decode, verifies every exported 16-bit sample exactly. An 8-bit FFmpeg narrowing comparison that changed some values by one was excluded, not treated as a pass.
- `tests/specialist_rasters.rs` covers the real JPEG XL/DDS/AVIF export facade, exact loss codes, profile refusal, cancellation and source immutability. Pillow 13.0.0 independently reopens exported DDS with exact RGBA including hidden RGB. FFmpeg 8.0.1 swaps red/blue for the valid DXGI RGBA profile, so that oracle result is retained as an incompatibility rather than changing the file to satisfy it.
- DDS backend tests inventory complete mip-chain geometry, enforce count/flags/capabilities and exact encoded tail lengths, bound pixels and cancellation, and prove only the top surface is read. `tests/dds_facade_mips.rs` checks five public-eligible sRGB encodings, actual omission loss, unchanged strict APIs and single-level hidden RGB without invented loss. Thirteen independently authored distinct-colored mip fixtures include legacy/DX10, odd dimensions and sRGB/linear profiles. Pillow 13.0.0 confirms nine top-pixel outputs; DXGI 91/72/75/78 are explicitly unsupported by that oracle, with independent channel/block expectations rather than external-success claims. Omitted lower levels receive structural validation, not decompressed semantic validation. See `tests/dds-mip-fixtures/README.md` and its hash provenance.
- DDS consumer evidence is retained in [`dds-consumers-2026-10-26/33e5c052-03a7-4dc0-a26c-95c46d94e22e`](validation/dds-consumers-2026-10-26/33e5c052-03a7-4dc0-a26c-95c46d94e22e/README.md), with final source/artifact hashes and raw logs. Four native tests include English/Japanese actual review controls, cancellation, stale owner/frontend/drafts, source mutation, PCND reopen, one-level behavior and the bounded extensionless signature probe. Browser host tests exercise 16 English/Japanese Open/Import review scenarios. Actual CLI, stdio MCP and paste tests cover named-loss refusal/approval, pixels/PCND, extensionless inputs and source hardlink/rename protection. All seven loss translations, consumer strict checks and actual Wasm compilation pass. Host egui review execution and Wasm compilation are separate evidence; no browser upload automation, original texture application or whole texture-gate completion is claimed.
- Independent FFmpeg/libdav1d AVIF checks cover exported color and auxiliary alpha; native and actual browser Wasm encode checks pass. The separate native identity reader now has bit-exact independent libaom plane/RGBA, actual installed consumer and bounded native-allocation evidence below. Neither qualifies browser reading or broader AVIF profiles.
- JPEG XL has checked-in independently generated libjxl fixtures and selected decoder/encoder tests. The pinned independent `djxl` 0.12.0 oracle has reopened fresh first-party RGBA8 and RGBA16 outputs, including single-axis geometry, exact 16-bit partial alpha and hidden RGB. Opaque RGB16 output remains byte-identical to its captured baseline. Native and browser facade tests also cover 1-pixel Studio exports; the editor remains 8-bit. Generic JPEG XL Open remains disabled. The separately qualified native isolated Import profile is documented below.
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

### Ordered pack gaps, audit of 2026-10-26

The [implementation sequence](temp/paintcraft-format-interoperability-classic-studio-v3/docs/IMPLEMENTATION_ROADMAP.md)
and [36-family target matrix](temp/paintcraft-format-interoperability-classic-studio-v3/docs/FORMAT_MATRIX.md)
are wider than the enabled subsets above. The next work is ordered by usable
consumer paths and prerequisites, rather than treating every candidate as a dependency:

1. **Extend isolated-reader qualification across supported platforms.** The narrow browser JPEG XL consumer now has actual production picker, adoption, PCND reopen and failure evidence below. Extend native JXL/JXR release-platform installation coverage and cross-browser qualification separately; browser quarantine is a logical failure policy, not a synchronous Worker-stop or aggregate physical-memory guarantee.
2. **Close the finite qualified stock profiles, then remaining required integrations.** Native identity sRGB8 and explicit SDR YCbCr AVIF profiles now have independently checked decoder/consumer routes, not full stock parity. V6 adds full/limited-range 4:4:4 and qualified 4:2:0 with reconstruction consent; unknown siting, HDR, grids, transforms and browser execution remain separate. HEIF has its narrow native color import below, while broader camera/grid/ICC/HDR/deep-color and browser profiles remain unqualified. The existing isolated helper and bounded preflight must remain the reader boundary; enabling `image`'s separate native feature alone is insufficient. HEIF writing remains prohibited.
3. **Implement and qualify layered writes.** PDN has a bounded internal typed PDN3 writer and native layered-copy preparation; public and preserving writes still await actual Paint.NET open/edit/resave. PAINT now also has an internal bounded typed writer, immutable native preparation and gated CLI/MCP fresh-copy routes; public/preserving Windows Paint qualification is still required. Its UI-name, visibility and opacity extensions remain unqualified. ORA's internal writer is not publicly enabled; editable group semantics and original-application qualification remain. Neutral pass-through group conversion is a limited import profile, not hierarchy preservation. PSD/PSB now have a bounded internal fresh RGB8 Normal raster-layer writer with independent psd-tools/Pillow checks; public layered export still awaits actual Photoshop open/edit/resave qualification. Groups, masks, text, effects and high-depth output remain unsupported. Separately, JPEG XR has an isolated fixed-RGB8 writer, native parent preparation and actual Studio/CLI/MCP fresh-copy consumers with independent C evidence. Default promotion, broader writer profiles and release-target execution remain pending.
4. **Extend qualified HDR/EXR conversion and distribution evidence.** Native isolated conversion, actual Studio preview/adoption and CLI/MCP inspection/import now work for the bounded profiles below. The final V7 helper uses selected image HDR and pinned `exr` 1.74.2 with Rayon disabled; standalone in-process qualification adapters are not the product isolation boundary. Explicit settings, digest-bound actual losses and the retained-linear-alpha compositing limitation remain required. Broader Radiance orientations/XYZE, EXR tiled/deep/multipart or differing windows, additional compression/color profiles and high-depth editing remain unsupported; their implementation and qualification cannot be inferred from the narrow route. All six native release targets and broader original-file/reference matrices still need execution. Zune HDR 0.5.2 failed retained exponent-wrap probes and is not adopted. JP2 remains deferred F7 work, not an implemented conversion or an initial-release dependency.
5. **Complete remaining required profiles without widening labels.** Narrow subsets do not close the pack's broader format requirements. DDS now has top-mip imports with omission consent and native RGBA8/BC1/BC2/BC3 fresh exports with explicit generated-mip policy. This does not close the whole texture gate: advanced browser consumers, further qualified encoding/alpha/swizzle/filter profiles and release/reference evidence remain. The current array/face policy is explicit refusal of arrays, cubes and volumes. Custom stored mips are not represented in Document; exports derive new levels from the base and never claim to preserve or update stale source mips. `TEXTURE_ICON_VECTOR.md` conditions additional BC families on encoder/profile qualification; compiling broad container support does not enable them. Additional JXL/JXR profiles still need their specified implementation and evidence. Track DCX multi-page consumers, remaining browser MSP2 UI selection, original historical-app evidence, TIFF precision/pages, ABR dynamics/pattern behavior, ASE color adoption and CSH path variants against their individual contracts; being unimplemented is not a basis for calling a requirement optional. Explicitly future animation/multipage editing remains distinct from current single-image conversion. Foreign preserving Save remains a separate gate throughout.
6. **Keep optional F7 imports distinct from initial-format parity.** JP2, RAW, selected PDF pages, KRA/XCF and the animation model remain future capabilities. They need their own bounded services, source/loss policies and real reference consumers; compiling a library is insufficient.
7. **Close the delivery gates for each implemented profile.** Complete the pack's hostile-input fuzzing, independent reference/version matrices, reproducible codec and whole-operation benchmark exports, failure/cancellation-tail tests and installed-helper/packaging coverage on all six release targets. Local controlled fixtures and headless UI tests do not establish general original-application compatibility or a hard aggregate-memory guarantee.

Current codec adoption is intentional and narrower than the candidate inventory:
`image` remains the common raster adapter, with direct `tiff`, `dds`, `pcx`,
`ico`, `icns`, `ag-psd`, `resvg`, `jxl` and `zune-jpegxl` paths where active.
The isolated AVIF helper statically builds pinned dav1d 1.5.4 behind its explicit
feature and charged allocation boundary; it does not add a system decoder fallback.
Zune-first PNG/JPEG/facade routing, `libheif` and OpenJPEG have not been
adopted merely because the pack lists them. The qualified native float helper
does adopt `exr` for its admitted scanline profile. Changing a working backend still
requires matching correctness cases and measured evidence, not a dependency-only switch.

The implemented subsets advance the pack; they do not complete its full interoperability, preservation or release gates.

### Native HDR/EXR conversion, 2026-10-26

The exact native Studio variants are `native-hdr-sdr8-worker-v1` and
`native-exr-sdr8-worker-v1`. They require the final V7 helper, matching schema 2
manifest and seven-variant list; older V4/V5/V6 and historical private V7
descriptors do not authorize float conversion. Input is at most 16 MiB and
1,048,576 pixels, with each side at most 65,536. Radiance admits RGBE in `-Y +X`
orientation with bounded packets. EXR admits v2 single-part increasing scanlines,
RGB[A] HALF/FLOAT channels, equal data/display windows and NONE/RLE/ZIPS/ZIP.
Unsupported structures and unresolved interpretations fail closed.

Conversion requires explicit Clip or Reinhard-per-channel, exposure in milli-EV,
negative-sample handling, and alpha preservation or linear-light flattening over
an explicit backdrop. Source-space or alpha assignments are explicit when needed.
Radiance uses the declared lower-edge RGBE representative and as-stored calibration;
EXR alpha defaults to the premultiplied convention unless explicitly overridden.
Nonfinite samples and out-of-range alpha are refused. Additive color at zero
alpha requires explicit linear-light flattening; preserve does not silently discard it. Assessment retains the bounded raw header, source facts,
settings, actual statistics, output hash and complete actual loss list. Header
receipt retention does not imply PCND metadata preservation. Partial alpha also
reports the encoded-sRGB editor versus scene-linear compositing limitation.

Studio displays the prepared immutable picture and settings with explicit loss
approval before publication. CLI/MCP inspection is read-only; import repeats the
same settings and requires raw source SHA256, assessment SHA256 and every actual
loss. Captured same-file identity, cancellation and editor-owner checks protect
publication; failed review/import leaves prior dirty/saved state intact. CLI writes
a staged fresh PCND copy, and MCP adopts an unsaved document. Ordinary Open,
Classic/browser conversion and foreign preserving Save remain denied.

Independent references cover seventeen cases: twelve exact converted RGBA buffers
and five expected policy refusals. Official OpenEXR-reopened floats and independent
Radiance lower-edge samples feed a separate Decimal100/NumPy conversion oracle,
never the Rust decoder or conversion implementation. Actual local Studio
English/Japanese review and failure tests, worker/parent process tests, and raw-layer
PCND reopen checks exercise the qualified path. The final dev/archive installation
passed all seventeen CLI/MCP cases and source/cancel/rollback controls. The separate
optimized `aarch64-apple-darwin` source archive, installed through the real CLI
installer, passed those same cases in 3.76 seconds and guards in 0.58 seconds.
Its CLI SHA256 is `33dccc52d90d34be83c5f8f18b05c73588107c0f7a9314503a4beb44d657e1a8`
and helper SHA256 is `24a014c300a3b8050b50afa4e71d1391c2949446b12bae7583a2ca48017ce29d`.
These are local correctness observations, not a performance comparison or six-target
release proof. The earlier private-profile timeout and successful isolated replay
remain separate evidence; the product deadline was not raised.

See `crates/services/paintcraft-formats/src/float_import/README.md` and
`tests/float-import-fixtures/README.md` for source policy, accounting and oracle
provenance. Logical allocation admission and supervised process termination do not
prove a hard aggregate RSS limit or arbitrary hostile-code sandbox. Original-file
coverage, broader required profiles and cross-platform release qualification remain.

### Native JPEG XL document import, 2026-10-26

Native Studio **Import image…**, CLI and native MCP `import_image` admit only
`native-jxl-srgb8-worker-v1`: metadata-free, static, identity-oriented,
original-profile straight sRGB RGBA8, including hidden RGB and alpha. Input is
at most 64 MiB and 4,194,304 pixels. ICC/custom profiles, container metadata,
high depth, preview/animation and other unqualified forms are refused. The
matching same-build helper and schema 2 manifest must explicitly list the
qualified variant; missing/inconsistent helpers fail closed. Ordinary Open,
Classic and generic JXL reads remain denied; the explicit browser profile below
is separately qualified. HEIF and JPEG XR have their own narrow native conversions below. Export remains the independent lossless RGBA8 path.

Desktop keeps the existing editing Executor and resident realm. It acquires an
input identity/metadata before submission, then captures bytes only inside an
admitted worker with the retained preparation lease. The shared codec job owns
source verification, helper installation/build checks, private child input,
bounded protocol and supervised process execution. Decoding and document
preparation stay private. Active tab, Session owner/state/epoch, document Arc,
frontend, dirty state, preview stamp and absence of document drafts are checked
before adoption. Cancellable progress has no empty conversion-loss approval
screen; this strict metadata-free profile reports no discarded metadata.

A failed, cancelled, stale or closed import cannot install into another owner.
The original source identity follows the adopted unbound, dirty document across
normal PCND saves, renamed input aliases, inactive tabs and parked workspaces.
Actual image-export/project-save admission refuses overwriting it. Native
reader output does not adopt a foreign preserving Save destination or source
project lineage. The preparation reservation remains held through result
retirement/adoption; document tile leases remain intrinsic across Session,
history and snapshot owners. Desktop reuses its existing provisional 2 GiB edit
preparation admission. This is logical resource ownership, not a physical-RSS or
arbitrary-filesystem/descendant-process sandbox guarantee.

Two default App tests cover policy/draft/extent/admission rejection before
publication and changed tab/Session/frontend/draft/closed pending ownership.
An explicitly invoked copied-installation test stages the actual helper and
schema 2 manifest beside the headless unit executable, with production discovery
and no PATH/environment override. Its two child fixtures verify exact six-pixel
RGBA including alpha 0/1/127/128/254/255, same-file pending/installed source
guards, normal deferred PCND Save, source rename/aliases/tabs/workspaces, retained
resident snapshot leases, changed ready-result owner/dirty/draft/source rejection
and cancellation of a running real helper before preparation charge retirement.
After the shared launch-context migration, the full default native suite passes
466 tests with 12 explicitly ignored fixtures, strict all-target checks pass,
and the copied real-helper qualifier and both child fixtures pass again.
No native app window is launched. This is focused installed-helper workflow
qualification; broader native installation/OS and real-device latency matrices,
additional source profiles and a browser consumer remain open.

### Native JPEG XR document conversion, 2026-10-26

Native Studio **Import image…**, CLI and native MCP `import_image` admit the
explicit `native-jxr-srgb8-worker-v1` variant for `.jxr`, `.wdp` and `.wmp`.
This is a single-frame unsigned opaque RGB8 profile, with absent color context
interpreted using the T.832 default sRGB policy. ICC/EXIF/unknown fields, alpha,
high depth, orientation/transforms, tiled/frequency/windowed streams and other
unqualified forms are refused. Admission is at most 64 MiB encoded input and
4,194,304 pixels. The matching same-build helper must qualify the exact selected import variant.
Historical V4–V7 helpers retain their schema 2 contracts; the optional qualified
V8 helper has the separate exact schema 3 contract described below. There is no
inline decoder, ordinary Open, Classic/browser reader or foreign preserving Save.

Every conversion requires `jxr-resolution-not-retained` consent because the
flat document does not keep physical print density. The captured assessment and
CLI/MCP receipts retain exact original per-axis IEEE bits separately from
effective density and provenance. T.832 A.7.25/26 assigns 96 dpi when a source
axis is absent or numeric zero; this is reported as inferred default density,
not as a physical value of zero. Explicit positive values remain distinct.
Unknown loss codes and missing consent fail before publication.

Studio previews the immutable prepared picture, shows effective horizontal and
vertical DPI with translated default labels, and requires a prior actually
visible preview frame and an explicit loss checkbox before **Open image**.
The preview texture is a nearest-sampled thumbnail bounded by 1024 pixels per
side and the adapter limit; the imported document remains full size. A declared
metadata reservation covers the full flat RGBA buffer, thumbnail and ColorImage
copy before allocation and stays held through approval or cancellation. This
excludes graphics-driver allocations and is not a hard physical-memory limit.
JPEG XL keeps its existing automatic adoption without an empty consent prompt.

JPEG XR reuses the existing admitted editing Executor, private native codec job,
parent-lease process supervision, retained preparation/resident leases and
same-acquired-file source identity. Tab, Session owner/state/epoch, base document,
frontend, dirty/draft and source verification remain binding through adoption.
Cancel, stale ownership or changed source bytes discard private results and
leave the prior document intact. The acquired source identity follows an
adopted converted document and protects renamed/hardlinked source aliases from
image export or project-save replacement; normal PCND Save remains available.
These are logical ownership guards, not arbitrary filesystem TOCTOU protection
or a general operating-system sandbox guarantee.

Qualification uses the repository-owned 4×3 gradient independently reopened by
the original C JXRLib, including authored 96×144 dpi and zero-field/inferred-96
variants. The copied installed-helper App test exercises eight real headless
English/Japanese review cases: approval, cancellation, source replacement after
preview and a new text draft after consent. It checks exact 12-pixel RGB output,
opaque alpha, translated default density, preview-reservation release and source
rename/alias refusal. The same run repeats the existing JPEG XL and running
helper cancellation fixtures. The default native suite passes 469 tests with
13 explicitly ignored fixtures; strict all-target checks and all seven locale
catalogs pass. Seven installed CLI/MCP tests additionally cover exact density
receipts, absent/unknown consent, converted PCND save, source aliases,
cancellation and missing helpers. This qualifies the controlled profile and
consumer paths, not broad original-application compatibility, every release
platform or real-device latency. Broader JPEG XR profiles remain separate work.

### Historical private JPEG XR fresh-writer qualification, 2026-10-26

The original optional `jxr-encode` helper feature implemented a fixed opaque RGB8 lossless
writer through the reviewed selected-library RGB8 factory and capped seekable
sink. It includes `float-codecs` but does not change the default V7 build.
`--jxr-encode` exists only in the private V8 candidate, with distinct `PCENC001`
and `PCJXR001` frames. Its historical exact schema 3 manifest and private encoder verifier
are separate from production decoder discovery, which still rejects this candidate.
At this historical stage there was no public CLI/MCP export command, Studio
export control or registry writer gate. These private artifacts remain unchanged;
current optional final-profile consumers are described in the following section.

The private request admits at most 1,048,576 pixels, 3,145,728 packed RGB bytes,
65,536 per side and 64 MiB encoded output. Request geometry/stride/length, fixed
profile, cap, owner/nonce and RGB digest are bound canonically. Output is retained
in a capped child-owned memory cursor, not a path or reopened temporary file;
the typed reply streams its completed bytes without a second whole child reply
copy. Fresh headers require raw positive-zero X/Y resolution bits, interpreted
as effective 96 DPI. The existing 256 MiB requested Rust/selected-C allocation
ceiling, parent lifetime lease and supervised deadlines apply. These are logical
admission/process controls, not a physical RSS bound or general OS sandbox.

Local macOS 14.7.2 Apple Silicon debug qualification passed fourteen whole-file
child encodes through untouched original C JXRLib and four parent-runner C pixel
cases. Six nonignored child controls cover exact build declarations, capped
refusal, malformed/cross-direction input, cancellation/deadline/output bounds,
actual parent death during blocked input, and complete-request lease EOF.
Parent controls additionally cover source/owner/request/output binding, retained
lease through publication preparation and exact schema/manifest rejection.
The largest independently checked child picture was 257×129, not the admitted
million-pixel maximum. Reserved output capacity and observed requested-heap
peaks do not qualify all admitted geometry, entropy or worst-case workspace.

The immutable private helper SHA256 is
`bf364cfe171df011d224f5f95637089b97c96dcdf1ae4c30cc326fcf2af62c5c`.
The shared default helper was separately rebuilt as V7, rejects the encoder
argument/new frames, and has SHA256
`51bcdda3830a648ee35b90e5868f7700a0392ca687ede2412ee42146e8d29f9b`.
Earlier compile/test-harness failures and their final passes remain separate
evidence; no deadline was raised. In this original private1 artifact, capped-sink `Io` failures reported
`CodecFailed`; subsequent seekcap2 work added typed `OutputLimit` classification.
Only that typed quota condition becomes `ResourceLimit`; generic IO and callback
failures remain `CodecFailed`. Global heap exit 75 remains `ResourceLimit`.

The historical limitations above do not describe the later optimized matrix or
current CLI/MCP consumers. Broader alpha/quality/depth profiles, Windows
application interoperability and execution on all six release targets remain
unproved. See the [writer developer record](../crates/services/paintcraft-formats/src/codec_worker/jxr_encode/README.md).

### Qualified JPEG XR fresh-copy consumers, 2026-10-26

The optional `jxr-codecs` producer now has an exact nonprivate V8 build record,
profile `native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1` and schema 3
manifest declaring eight imports and one export. Private V8 records are refused;
V4–V7 decoder contracts and the default V7 build remain unchanged. Canonical
packaging verifies this descriptor without executing cross-target binaries and
retains the same JPEG XR, dav1d and HDR/EXR notices.

Exact Native/Studio `ExportFlat` variant `native-jxr-rgb8-lossless-worker-v1`
backs native Studio’s existing **Export image** dialog, CLI `export-jxr` and MCP `export_jxr`. It writes a staged fresh copy,
leaving native saved path, source identities, document/history and dirty state
unchanged. The composite must be wholly opaque: there is no hidden matte or
alpha discard. Actual flatten/hidden-color losses require explicit acknowledgment.
Geometry is at most 1,048,576 pixels and 65,536 per side, with 64 MiB output.
The fixed lossless RGB8 profile stores positive-zero density fields, whose
T.832 effective value is 96 DPI. Alpha/depth/quality choices are not offered.

A shared structural classifier selects short headers through the existing
legacy profile, including fresh short files; it never retries after a decoder
failure. Qualified long writer headers select the distinct
`native-jxr-writer-rgb8-worker-v1` ImportConvert variant before launch. This
writer-compatible subset requires fresh version 1, exact fixed RGB8 flags,
positive-zero density, at most 1 MP and 64 MiB input. Import still requires
`jxr-resolution-not-retained` consent. Ordinary Open, Classic/browser conversion,
foreign preserving Save and generic no-variant JXR export remain denied.

An immutable optimized Apple Silicon helper, SHA256
`529ed471936258cba3ce1e72444c98bc5f9f0c2848b808552ca213dc3f2c716c`,
passed 29 fresh small/maximum/skinny/noise encodes through untouched original C
JXRLib, four typed-cap controls, and selected-reader plus C verification of all
29 retained corrected-writer files. Child/C deadlines remain 10 seconds and the
requested allocation ceiling remains 256 MiB; these are not physical RSS bounds.

Actual canonical archive installation paired that optimized helper with the
current debug CLI. Four CLI cases (4×3, 17×19, 4096×1 and 1×4096) passed every
independent C RGB byte and PCND reopen comparison, including upfront long-header
selection. MCP exercised actual loss consent, dirty/native-binding preservation,
renamed-source and hardlink refusal, cancellation and transparent refusal.
A separate forged-manifest case proved exact schema 3 refusal without replacing
its existing destination or changing the MCP session. Two earlier fixture
failures remain recorded: the first lacked diagnostic stdout; the second exposed
a noncanonical temporary installation path and was fixed only in the test.
No successful CLI/MCP case was rerun to hide those failures.

Native Studio uses its existing editing Executor and native-only export choice;
portable/browser export inventories remain unchanged. It prepares the actual opaque
composite once, retains the admitted encoded result through destination selection
and visible flatten/hidden-color/replacement confirmation, and rechecks captured
owner, source identities, helper and destination at the final staging checkpoint.
Export does not change document contents, history, dirty state, title or Save binding.

The actual copied installation paired that optimized helper with debug App
`d3dbdeb36a7fb4f58921998bc2c5c502070ac940efef156d0e28b48210a61687`
on Apple Silicon. Twenty-four headless English/Japanese cases passed in 19.21
seconds: fourteen export/guard cases, two fresh 4096×1 export/import/PCND round
trips and eight existing legacy JPEG XR review cases. Four fresh UI exports
matched every RGB byte through the independently pinned original C decoder;
its successful exit is required before reading pixels. The layered fixture’s
expected pixels come from the independent Fraction oracle. Cases cover visible
loss/replacement approval, cancellation, drafts, frontend/tab changes, renamed
source hardlinks, changed helper metadata, and source-alias refusal after PCND Save.
The native OS destination picker is exercised through its completion seam; no OS
window or native file dialog was automated. Two earlier fixture failures remain
recorded: zero oracle stdin allowance, and constructing an Editor with a path
without the real App Open association. The fixes affect only the test runner/setup.

This is qualified local helper and Studio/CLI/MCP evidence, not completion of F12.
Staged release/source-installer selection now enables qualified V8 only on Apple-silicon
macOS; five other native targets and the global Cargo default retain V7. Fresh
immutable source `a1c1663637814a351e094d462b4c9f8401d224bc` produced the optimized
CLI/App/lib-test artifacts. SHA-bound installed CLI baseline/portable/forged/C
cases and the same 24 optimized App cases passed through the actual bundle
Resources layout. Full ad-hoc signed DMG/update-ZIP validation, original nested
signed-helper execution and a new signed-helper CLI archive/private-prefix
portable run passed. The raw helper is the unchanged qualified `529ed471…`
producer; its signed derivative is `cb17b763…`. See
local ignored evidence at `docs/validation/jxr-staged-macos-2026-10-26/receipt.json`.
Remote CI, five held native runtimes, Developer-ID signing/notarization, original
Windows interoperability and required ICC/alpha/depth/other compression
profiles remain separate unverified or unfinished work.
Raw receipts are in the ignored local validation records, including
`docs/validation/jxr-native-ui-2026-10-26/closeout.json`; tracked installed-worker
tests retain the reproducible assertions.

### Native HEIF sRGB8 document conversion, 2026-10-26

Native Studio **Import image…**, CLI and native MCP `import_image` admit
`native-heif-srgb8-worker-v1` for `.heic`, `.heif` and `.hif`. Input is at most
64 MiB and 4,194,304 pixels. The same-build isolated helper, exact known V4,
AVIF-enabled V5, SDR-enabled V6 or final V7 record and matching schema 2 manifest are required. Each manifest
lists its finite variant set, including this independently qualified HEIF route. Bare Open, Classic,
browser HEIF reads, preserving Save and all HEIF writing remain denied.

This initial color profile requires a directly coded primary and optional
same-size, directly coded straight8 alpha. Container NCLX and every coded VUI
must agree explicitly: primaries 1 and sRGB transfer 13, with full-range identity
GBR 4:4:4 or matrix 1 YCbCr 4:4:4/4:2:0 at full or limited range. Type-0 4:2:0
uses chroma origin `(0, 0.5)`, bilinear interpolation and outermost-sample
extension. Conversion expands the declared range, rounds/clips to RGB8 and
preserves alpha exactly, including RGB at alpha zero. 4:4:4 does not interpolate
chroma. Odd 4:4:4 edges are qualified; odd 4:2:0 and shifted conformance crops are
refused. Grids/identity-derived color, unknown properties, extra image/metadata
items, ICC, unlabelled color, display transforms, premultiplication and deep/HDR
profiles remain unqualified. A compiled codec does not grant those profiles.

Matrix-1 sources report `heif-color-reconstruction`; nonempty primary or alpha
item names report `heif-item-names-not-retained`. Names appear in the source
assessment receipt, but do not become PCND metadata. Only that actual name loss
sets CLI `discarded_metadata`; YCbCr reconstruction is a separate color loss.
The unnamed identity fixture has neither loss and needs no empty approval
screen. Receipts bind exact source SHA, size, NCLX/range/chroma, alpha declarations,
source item names, the selected reconstruction policy and acknowledged losses.
Unsupported metadata is refused rather than silently discarded.

Parent-side assessment uses the bounded shared item graph and never invokes the
selected SPS parser. The helper checks actual coded and decoded geometry,
precision, sampling, metadata and alpha before emitting the typed sRGB response.
The shared job validates its model/extent/source/owner again before allocating
document tiles. Existing Launch, cancellation, same-open source identity and
resident preparation/adoption services are reused. The imported document is an
unbound, dirty PCND copy; captured source aliases remain protected after normal
PCND saves, including source renames and hardlinks. Studio loss approval is tied
to the actual prior-visible-frame picture and current owner/draft state.

The [owned HEIF color fixtures](../crates/services/paintcraft-formats/tests/heif-color-fixtures/README.md)
record pinned FFmpeg 8.0.1/libx265 encoding, exact authored/native-plane readback,
and independent libheif 1.20.2/libde265 1.0.16 container/alpha readback through
Pillow-HEIF 1.1.1. A separate exact-Fraction matrix/range/siting reference and
explicitly configured FFmpeg bilinear conversion agree with selected RGB within
1 code value over all pixels; identity RGB and alpha are exact. The 33×25 set
checks odd 4:4:4 borders. Source bytes are never rewritten to make an oracle pass.

libheif RGB is retained as independent negative evidence for these 4:2:0 patterns:
default/nearest differs by up to 69 full-range and 79 limited-range values;
forced bilinear differs by up to 98 and 113, including bottom-edge behavior.
Its independently decoded native planes still match exactly. This qualifies a
declared PaintCraft reconstruction policy, not universal application rendering
parity. Earlier lossless-labelled FFmpeg color-option mismatches and failure to
recognize the old gray wrappers are retained separately, not counted as positive
container qualification.

The focused HEIF filter passes 34 tests. Ten actual copied-installation CLI/MCP
tests include named/unnamed identity, color receipts, PCND reopen, missing/unknown
consent, source aliases, cancellation and missing helpers, alongside existing
JXL/JXR regressions. The actual copied App qualifier runs eight English/Japanese
review cases plus two unnamed identity cases, checking exact alpha/identity and
independent YCbCr pixels, cancellation, source replacement, stale drafts and
renamed-source Save As refusal. Default native tests pass 470 with 14 explicit
ignored fixtures; the real copied qualifier is invoked separately and asserts
one executed test per child. Strict all-target checks pass. Broader file profiles,
original camera/application comparisons, real Windows/Linux installations and
browser HEIF execution remain separate gates. No performance improvement or
physical-RSS/filesystem sandbox guarantee is claimed.

### Native AVIF identity sRGB8 document conversion, 2026-10-26

Native Studio **Import image…**, CLI and native MCP `import_image` admit only
`native-avif-srgb8-worker-v1` through a verified AVIF-enabled V5/V6/V7 helper,
the corresponding exact profile and schema 2 variant set: V5 uses
`AVIF_NATIVE_PROFILE` with four entries; V6 uses `AVIF_SDR_NATIVE_PROFILE` with
five; final V7 uses `FLOAT_NATIVE_PROFILE` with seven. V4 refuses AVIF; existing
JXL/JXR/HEIF jobs accept exact V4/V5/V6/V7 contracts. Integrity failure
does not trigger another decoder or helper-profile attempt. Complete packages
and CLI installers include the helper, manifest and JPEG XR/dav1d notices.

The first reader is a direct reduced-still-picture AV1 profile-1 8-bit primary,
full-range 4:4:4 with explicit primaries 1, sRGB transfer 13 and identity matrix
0. Optional alpha is same-size profile-0 8-bit monochrome, full range with CICP
2/2/2 and the exact straight auxiliary link. Container and actual sequence
properties must agree. Native GBR planes are reordered exactly and alpha-zero
coded RGB is retained. No YCbCr reconstruction is silently substituted.

Input is at most 64 MiB, 4,194,304 pixels and 65,536 per side; source/item/property
bounds and actual OBU declarations are checked by the bounded pure-Rust parent
parser. The helper repeats admission, decodes with pinned static dav1d 1.5.4,
then compares decoded geometry, precision, layout, color and still flags before
returning typed pixels. Decoder-owned C requests, including context, scratch,
pictures and alignment headers, use the existing 256-MiB requested-heap account.
Threads and frame delay are one and the frame pixel limit is set before decode.
This does not bound libc internals, thread stacks, physical RSS or aggregate
host/child memory. No OS sandbox claim follows from process isolation.

Nonempty primary or alpha item names require `avif-item-names-not-retained`:
receipts preserve their names and exact source/color/alpha assessment, but PCND
does not store those labels. Unnamed identity pictures have no conversion loss
and do not show an empty approval prompt. Named Studio approval is bound to the
actually visible preview, current owner and unchanged draft. The shared native
Job/Launch, retained preparation/adoption leases and same-open acquired source
identity are reused. Failed/cancelled/stale imports preserve the prior document;
converted PCND saves cannot overwrite renamed or hardlinked sources, including
after an ordinary PCND Save. Imports remain unsaved, unbound project copies.

Checked-in [libaom oracle evidence](../crates/services/paintcraft-formats/tests/avif-fixtures/libaom-oracle.json)
uses pinned libaom 3.15.2 as a genuinely different decoder engine: all native
planes from both primary streams and the alpha stream match selected dav1d
exactly, as does assembled RGBA including hidden RGB. FFmpeg/libdav1d validates
container/plane assembly separately; it is not counted as a second codec engine.
Losses from the ravif encoder's original RGBA input remain separate evidence.

Actual copied V5 Studio qualification passes 12 English/Japanese cases spanning
named consent/cancel/source changes/draft staleness and unnamed opaque/alpha
adoption, nonempty prior documents, exact PCND reopening, lease retirement and
source aliases after Save. The historical V5 installed CLI/MCP suite passed 12 cases; its historical V4
run passed 10 and skipped two AVIF cases. The expanded current suite has 14
cases, including two SDR cases: V6 passed all 14. A concurrent V5 run passed 10
and hit two existing HEIF deadlines; both unchanged cases passed individually,
and the complete serial V5 replay passed all 12 (68.31 seconds). Current serial
V4 passed 10 and explicitly skipped four AVIF cases (59.14 seconds). The original
concurrent failure remains recorded; simultaneous-load deadline qualification is
not established. The V6
helper suite passes 38 tests, with private fixtures explicitly ignored. Actual local
Apple-silicon source and archive installs also pass AVIF CLI/MCP import and PCND
save. Synthetic six-target package checks and configured release jobs are not
six-target execution or clean-install evidence.

Ordinary Open, Classic/browser readers, foreign preserving Save, unknown chroma
siting, ICC, HDR/deep precision, grids, transforms, premultiplication and broader
metadata remain refused. The separately qualified SDR increment below does not
complete full AVIF interoperability. See [AVIF qualification](AVIF_QUALIFICATION.md)
for bounds, allocation scope, independent provenance and remaining gates.

### Native AVIF SDR YCbCr import, 2026-10-26

The exact `native-avif-sdr-ycbcr8-worker-v1` native Studio/CLI/MCP route uses the
verified V6/V7 helper. It admits explicit BT.709 primaries/matrix, sRGB transfer,
8-bit full/limited range and 4:4:4 or explicit vertical/colocated 4:2:0, plus
optional same-size straight alpha. Unknown 4:2:0 positions remain refused. The
same-FD family selector seals source classification once and requires exact
frontend admission before helper discovery/launch. Identity code 5 remains
compatible with V5/V6/V7; there is no decoder-error fallback.

Every SDR import requires actual range/chroma reconstruction consent; named
items additionally require name-loss consent. Native Studio reuses the visible
preview and source/draft/session/tab/frontend guards. Eighteen actual copied V6
English/Japanese cases passed, including nonempty-document preservation, exact
Fraction RGBA, saved-PCND reopen and renamed/hardlinked source guards. All twelve
V5 identity App cases passed unchanged. CLI checks all 24 layout/range/parity/alpha
combinations and named consent; MCP checks actual adoption/refusal/source/session
behavior. These local Apple-silicon checks do not establish six-target execution.
See [AVIF qualification](AVIF_QUALIFICATION.md) and its independent fixture and
ignored raw-proof links. Browser, Classic, ordinary Open and preserving Save are
unchanged.

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

### ABR resource reader, 2026-10-25

The internal `paintcraft-formats::abr` adapter preflights the complete admitted
source before invoking selected `ag-psd` 0.3.0. ABR versions 6/7/9/10, minor 1/2,
admit raw/PackBits 8-bit and 16-bit sample masks. Signed origins, IDs
and full-precision mask values survive independently of the backend's narrowed
8-bit convenience view. The selected parser also supplies computed/sampled and
other recognized brush metadata; this does not install or render those brushes.

The selected Rust reader lacks 16-bit PackBits. After exact byte-row decoding,
the adapter gives it an equivalent private raw-16 view, admitted at no more than
48 MiB before allocation. Only those compressed sample records are rewritten,
and admitted pattern sections are handled separately as described below;
IDs, opaque header data, signed bounds and depth stay exact, while unaffected
sample records and all descriptor/hierarchy payloads are copied verbatim.
Each emitted replacement is decoded and compared with every original `u16`
before the selected library performs its metadata/high-byte check. Returned
resource sections retain the original compressed payloads. This private decoder
view is not a user-visible ABR writer and never changes the acquired file.

Embedded `patt` resources now retain exact RGB/gray/indexed channels as eight-bit,
sixteen-bit or finite f32 bit patterns, with raw/PackBits decoding, partial
rectangles, palettes, optional palette words and original unwritten/empty-written
slot positions. They remain separate from explicitly converted informational
RGBA8 previews. Canonical previews reduce 16-bit values by rounded division by
257 and clamp/round finite float values, reporting both conversions. Missing
color slots, extra color/spot slots or a written user mask produce an explicit
unavailable-preview reason instead of guessing roles by written ordinal. The
last sheet-mask slot alone supplies optional alpha. Original source planes and
ordered pattern payloads remain unchanged, and resources are immutable.
Optional four-byte indexed palette tails are preserved opaquely and always
disable previews, including zero tails. Adobe defines color-count/transparency
fields for the separate ACT format but not these pattern bytes; neither ignoring
them nor applying ACT semantics is qualified by the source-channel checks.

Because the pinned pattern reader omits alpha and narrows source precision,
original `patt` sections never reach its whole-file path. Canonical preview views
use the selected writer/reader under complete framing/packet admission; original
alpha is handled from its separately retained source channel. Pattern admission
allows 64 records, 4,194,304 pixels each, 16,777,216 aggregate pixels, 66 slots
per record, 16,777,216 aggregate channel values and 64 MiB of decoded channel
bytes. Private converted pattern views have conservative encoded bounds of
36 MiB each and 144 MiB aggregate, admitted before upstream work and processed
one at a time. None of these allowances proves physical peak memory, color-managed
display, native rendering parity or brush-engine adoption.

Bounded Action Descriptor walks cover `desc` and `phry` sections before upstream
tree allocation. Complete ordered section payloads remain available, including
unknown fields or hierarchy content omitted/defaulted by the parsed brush model.
Encoded path/alias values remain inert metadata; the reader never follows them.
The resource operation does not publish a document or mutate editor tools,
history or preferences. No public format capability is enabled yet.

Admission allows 16 MiB of source, 64 sections, 256 samples, 16,384 pixels per
dimension, 4,194,304 pixels per sample and 16,777,216 aggregate sample pixels.
Descriptor limits include 32,768 aggregate counted items/values, depth 16,
4,096 UTF-16 units per string or ASCII bytes per key/alias, 1 MiB of aggregate
text and 4 MiB of aggregate raw blobs. These checks prevent known unchecked
extent, sample-allocation and PackBits-packet paths from reaching the selected
decoder. They do not prove a hard allocator or cooperative cancellation-tail
bound during upstream calls.

The [owned fixture suite](../crates/services/paintcraft-formats/tests/abr-fixtures/README.md)
has twenty-six focused tests and successful Node.js 24.10.0 / upstream JavaScript
`ag-psd` 33.0.0 checks for the initial five source specimens and both Rust-generated private
raw-16 views. The public JavaScript channel decoder verifies every low bit;
metadata, signed bounds and opaque headers also match the compressed sources.
The JavaScript reader and Rust port share lineage. Independent `psd-tools`
1.26.0 additionally verifies 45 exact array slots across ten patterns in six
owned specimens, including 16/f32 bits, alpha, palettes, holes and auxiliary/user
masks. JavaScript preview vectors agree for five qualified patterns; its optional
palette-word ambiguity and guessed channel roles are retained as explicit
qualification limits. This is not original Adobe qualification. Other pattern
color models, one-bit/ZIP/predictive channels, ambiguous preview layouts, legacy
ABR 1/2, full preset behavior and wider qualification remain required. ABR export is outside the import-only asset policy.

The [native sampled-brush backend](SAMPLED_BRUSHES.md) separately admits exact
R8, native-size static sampled presets under an explicit PaintCraft policy.
Original descriptors and samples remain in the adopted immutable resource;
unsupported transforms, dynamics, textures and high depth are refused, while Q8
spacing conversion requires acknowledgement. Owned wire pixels render through
the existing mask executor with preview/publication and Undo/Redo checks. The
additional fifteen-candidate fixture has a successful upstream JavaScript
metadata/mask check, with the same shared-lineage limitation. Raster/tool/editor
checks separately cover independent dense pixels, resource bounds, cancellation,
no-op history and captured preview/release equality. Studio desktop and browser
now expose resource import, individual preset assessment, explicit static-tip
adoption and pointer painting. These checks do not establish Photoshop
brush-rendering parity.

CLI `brush inspect` and `brush paint`, plus MCP `inspect_brushes`, `adopt_brush`
and `paint_sampled_stroke`, expose this bounded static-tip policy. Typed
`paintcraft.brush-inspection/v1` output and `schema brush-inspection` enumerate
preset capabilities and required conversions. Libraries admit at most 256
presets before repeated descriptor assessment. Inspection and selection do not
change document/history/save binding; actual painting uses the same captured tip
and atomic mask executor. Empty documents gain their first transparent layer and
stroke in one undoable Brush edit, while ineffective coverage leaves no layer.
The MCP session retains at most sixteen distinct painted source versions,
coalescing repeated adoption by exact digest and frozen file identity. Selected
and painted sources remain protected across rename/hard-link aliases and source
replacement; destination identity failures reject the write. CLI and real stdio
MCP tests verify independent source-mask pixels, failed adoption preservation,
spacing consent, no-op history, Undo/Redo and protected source files. See
[workflow examples](MCP.md#imported-brush-resources). No Classic image Open,
ABR export or Photoshop rendering-parity claim is enabled.

Studio keeps stock brushes separate from imported native-size tips. The selected
name and original size replace generic size/media controls; opacity remains
available. Spacing conversion needs per-selection approval. Changing a selection
or loading a new library resets approval; invalid/cancelled/stale imports keep
prior resources and painting unchanged. The adopted immutable library stays
owned by its document when the resource library is replaced. Native acquired
source guards also follow document tabs/workspaces and protect rename/hard-link
aliases from project saves and image/resource exports. New documents and Classic
workspaces do not inherit Studio's adopted tip. These are ownership guards, not
an arbitrary-filesystem TOCTOU guarantee.

Desktop sampled coverage prepares on the existing editing worker. Browser
coverage follows the existing synchronous editor architecture, with a conservative
upfront limit of 1,048,576 dab-support pixel visits for each preview/commit. Its
integer L1 path-length bound can refuse work that the exact Euclidean sampler
would admit; accepted strokes retain the exact sampler and are never truncated
or resized. This limit is not a wall-clock or intra-call cancellation guarantee.
A rejected preview cancels the gesture and returns to the published picture.
Focused tests cover translated desktop selection, exact preview/release samples,
blank-document layer+stroke atomicity, opacity/no-op history, cancellation,
stale/resource replacement, browser refusal and native protected input aliases.
A local production Wasm build passed a headless Chrome ABR upload, explicit
static adoption, pointer preview/release screen-pixel equality, Undo/Redo and
PCND download check with no page errors. The downloaded project reopened and
rendered through the CLI. This is focused workflow evidence, not full release
qualification or an independent Photoshop rendering comparison.

### ICNS selected-entry conversion and fresh export, 2026-10-26

Pinned `icns` 0.5.0 is used only for admitted container/type handling; optional
PNG/JPEG2000 features are disabled. PNG pixel work reuses the existing image
codec and shared ICO/CUR preflight, with separately bounded ICNS sRGB/EXIF
metadata. Whole-file lengths, ordered records, optional TOC, image dimensions,
CRCs, opaque metadata and aggregate budgets are checked before upstream reads.
Directory entries keep logical size, Retina scale, physical size and source
record index separate; duplicate types remain ordered. Unsupported legacy/JPEG
2000 entries stay inert and cannot be selected through another size/type.

The reader has exact independent Pillow 13.0.0 evidence for eleven modern PNG
types. Fresh writing currently admits eight conventional PNG types with exact
Pillow and native Apple `iconutil` pixel hashes, including alpha/hidden RGB.
Isolated probes on macOS 14.7.2 arm64 demonstrate small 1x PNG exclusions:
`icp4` changes all 256 pixels and 192 alpha values, `icp5` all 1024 and 768,
and an isolated `icp6` produces 48x48 instead of requested 64x64. No type/density
substitution is performed. The [fixture/oracle record](../crates/services/paintcraft-formats/tests/icns-fixtures/README.md)
retains source/output/tool hashes, positive results and negative observations.
Repository-authored specimens keep PNG and small RGB/mask profiles independent;
no application-icon data is duplicated.

The reader additionally admits exact small `is32`/`il32` raw or three-channel
RLE RGB paired unambiguously with exact `s8mk`/`l8mk` alpha. Packet extents are
checked before pixel allocation; legal leading zero packets are never guessed
away. Independent fixtures and Pillow preserve exact RGBA. Raw interpretations
have divergent native RGB evidence, and native RLE extraction drops its final
packet while alpha stays exact. Candidate writing is exercised privately but
production legacy writing is refused. Apple-own opaque writer controls also
preserve all wire pixels then lose the last blue pixel on extraction. Partial
and zero-alpha controls retain association/hidden-RGB differences separately;
canonical `ic04`/`ic05` ARGB decoding/writing remains unqualified.

The directory overrides the selected library's incorrect `ic05` Retina label
using Apple's SDK `IconStorage.h`: `ic04` is16@1 and `ic05`32@1, distinct from
modern `ic11`16@2. These ARGB payloads remain inert; descriptor accuracy does
not enable decoding. The incomplete ImageIO investigation remains excluded;
its stalled ARGB control is not positive native-display evidence. Canonical
16/32 at 1x output remains blocked on native/ARGB qualification rather than a
type/density substitution.

Studio desktop/browser **Import image…** now lists the admitted directory with
original record index, type tag, logical dimensions, display scale, physical
pixel dimensions and encoding. No entry is chosen automatically. Unqualified
entries remain visible and disabled. A chosen entry must be decoded into a
private preview, actually displayed in an earlier frame, and accepted with
explicit import-loss consent before replacing the painting. Changing entries
invalidates preview/approval; stale document/frontend owners and cancellation
cannot publish. The imported painting is a dirty, unbound converted copy;
metadata, other entries and logical size/density are explicitly not retained.

Native acquisition keeps the exact opened file used for bounded decoding.
Pending import and installed document owners retain its frozen identity and
path, including after normal PCND saves, renames, other tabs and parked
workspaces. Actual project-save/image-export routes refuse aliases of these
inputs. Normal saves to a distinct bound PCND remain available. This is source
ownership protection, not race-proof traversal on arbitrary filesystems.
Browser uploads grant no ability to overwrite the original file.

**Export Apple icon…** in Studio and the CLI/MCP fresh-export operations accept
only explicit `ic07`, `ic08`, `ic09`, `ic11`, `ic12`, `ic13`, `ic14` and `ic10`
PNG types. Those are respectively 128/256/512 at 1x and 16/32/128/256/512 at 2x.
The UI initially selects none. There is no inferred Retina substitution or
legacy-writer choice. Up to eight distinct types, 4,194,304 source/aggregate
output pixels and 64 MiB encoded output are admitted. Each entry is fitted
independently from the original canvas with transparent aspect-ratio padding
and explicit nearest/Lanczos3 filtering. Retained preview RGBA buffers are the
ones encoded after displayed-preview and flatten/hidden-RGB/resize loss consent.
Native export uses a parented picker, source/owner checks and destination
fingerprinting; a changed source/destination or cancellation cannot acknowledge
a painting save. Browser export downloads immediately and leaves painting name,
history and dirty state unchanged. Original ICNS preserving Save, normal Open,
generic flat export and Classic routes remain denied.

Focused consumers cover six native import/export tests and five browser tests,
including English/Japanese controls, exact density-selected pixels, disabled
unsupported choices, stale/cancelled reviews, changed export options, renamed
source aliases, late source ownership and destination replacement. Full host
runs pass 464 native tests (9 ignored) and 189 browser tests, followed by a
focused unqualified-entry refusal test; strict checks,
seven-language catalogs and production Wasm build pass.

A headless Chrome production run imported `ic11` explicitly as 16 logical pixels
at 2x with 32 physical pixels, required preview/loss consent, then exported a
53,055-byte `ic07`+`ic11` fresh set and a 726-byte editable PCND. Unapproved
import/export attempts stayed inactive, asset download kept the painting title
and dirty state, and there were no page errors. A separate standard-library
parser checked downloaded ICNS lengths, TOC, PNG CRCs and physical type sizes;
all 1,024 unscaled `ic11` RGBA pixels matched the reopened PCND render and the
original source pixels after the disclosed transparent-hidden-RGB loss.
Source/Wasm hashes, screenshots, harness and verifier are retained in ignored
`docs/validation/format-specialists-2026-10-25/icns/`. These workflow checks do
not establish general Apple application/display compatibility, exact independent
resampling at every size or full release qualification.

### Historical MSP conversion workflows, 2026-10-25

The internal `paintcraft-formats::msp` module handles an initial historical
monochrome profile, distinct from modern layered `.paint`. `DanM` raw rows and
`LinS` encoded-length row maps undergo complete checksum, extent and packet
validation before pixel allocation. Every nonzero encoded row must expand
exactly to its byte-aligned packed width; the reader never follows arbitrary
row offsets. Limits are 16 MiB source, positive 16-bit dimensions and 4,194,304
pixels. Original header words and unused packed tail bits remain available;
RGBA expansion explicitly maps bits to opaque black/white without device/aspect
scaling. Fresh MSP1 output accepts packed bits or already opaque black/white
RGBA. Thresholding, dithering, alpha removal and source-header preservation are
not implicit.

Seven owned specimens, including one separately Pillow-written image, pass
independent Pillow 13.0.0 pixel checks. All seven fresh Rust-written copies also
pass that reader. Seven focused Rust tests cover exact pixels/header metadata,
odd widths, long RLE packets, truncation, checksum/size/map failures, conversion
refusal, cancellation and mutations. See [fixtures, provenance and exact profile](../crates/services/paintcraft-formats/tests/msp-fixtures/README.md).
The primary Pillow implementation documents zero-length MSP2 rows as inferred
white-fill; this profile refuses them. Original historical Windows Paint and a
complete normative Microsoft specification were unavailable, so original-program
compatibility remains unqualified. The separately qualified MSP2 writer now has
native Studio/CLI/MCP consumers below; browser UI version selection and explicit
threshold/dither/matte conversion controls remain unsupported.

The shared image facade and profile gates support explicit Studio import and
fresh flat export on native/browser, CLI and MCP paths. CLI `import --frontend`
also supports each Classic profile's explicit legacy conversion route, without
adding MSP to normal Classic Open or Save filters. Preserving MSP Save and Save
As are unavailable. Every import reports `msp-metadata-ignored`; native/browser
confirmation and CLI/MCP loss acknowledgements precede publication. The converted
copy is an unsaved native document. CLI file receipts and MCP conversion details
identify source bytes and metadata consent without binding the original MSP.

Native pending/installed conversions and MCP sessions retain `SourceIdentity`
from the same opened file used for decoding. Existing source guards protect
renamed and hard-linked originals across Save As and export; save copies do not
clear the retained input guard. CLI import and external-image placement retain
that identity throughout their individual command. Browser downloads produce
fresh copies and do not adopt the input as a writable project. Tests cover real
CLI/MCP conversions, consent/failure preservation, same-source aliases after
rename, source guards after project Save As, exact Studio export and color/alpha
refusal. Native tests exercise the actual export path; browser consent/encoder
regressions and an actual wasm32 target check pass. All seven locales include
MSP metadata consent and export labels. See [MCP and CLI examples](MCP.md#historical-msp-pictures).

The source-header/bitplane codec remains separately callable without document
publication. Export accepts only already opaque black or white output pixels;
acknowledging an unrelated loss never grants an implicit threshold or matte.
Existing flatten/hidden-RGB loss policy still applies. These gates qualify the
bounded format profile, not every historical specimen or original application.

#### Native headless MSP version selection, 2026-10-26

CLI `render --msp-version v1|v2` and MCP `export_image` with optional
`msp_version` expose explicit fresh copies. Absence preserves the old MSP1
route byte for byte; explicit V1 uses the same guarded preparation as V2.
`native-msp2-row-rle-v1` is admitted only for Native + Studio + ExportFlat with
`.msp`. `.msp2` is not an alias. Native Studio also offers a version chooser
and actual encoded-output preview; MSP1 remains the default. Browser UI still
exports MSP1. Neither route adds Classic export or ordinary Open. Foreign
preserving Save remains denied.

Both versions require already opaque black/white rendered pixels. The writer
adds no threshold, dithering, matte, alpha discard or compression loss; actual
common `flatten`/`hidden-rgb` consent still applies. Fresh RGBA packing writes
canonical zero unused tail bits and does not claim to retain an original MSP
header or bitplane. The existing packed-bit backend can retain explicitly
supplied tail bits independently. The V2 packet/count walk uses the existing
writer algorithm, with nonzero u16 encoded row lengths, 16 MiB output, positive
16-bit dimensions and 4,194,304 pixels.

Before the sole render, admission prices its RGBA composite, fixed 8192-byte
row-counting scratch and bounded metadata, with retained document raster payload
reported separately. Counting determines the exact packed/output quote before
those full buffers are allocated. Both acquired leases and the prepared bytes
remain held through staging and publication; source aliases, cancellation and
changed destinations refuse the copy. Success never clears dirty state, retargets
the saved binding or performs a fallible cancellation/source recheck after the
file replacement is acknowledged. These are logical payload bounds, not a whole
process RSS or latency guarantee. The explicit CLI path currently requires one
render worker and the default generic quality; absent options keep the old path.

The existing MSP2 specimens have an independent packet parser and pinned Pillow
13.0.0 evidence. Actual CLI/MCP tests check 14 complete files from two nonuniform
owned sources, including odd-width rows. Default and explicit V1 bytes agree;
V2 differs and matches the independently qualified complete `runs-2048.msp`
golden. Pillow and independent parsing verify every packed byte and visible
pixel. MCP success starts from an already-dirty bound PCND session after an
actual layer-name edit; complete inspection/history remains unchanged through
all exports and renamed-source alias refusals. No original historical Windows
Paint, browser UI version selection, release-platform matrix or benchmark
is qualified by these headless checks. See the [consumer proof](../apps/paintcraft-cli/tests/msp-version-fixtures/README.md)
and [MSP2 backend evidence](../crates/services/paintcraft-formats/tests/msp2-writer-fixtures/README.md).


#### Native Studio MSP version chooser, 2026-10-27

Native Studio selects MSP1 or MSP2 in the existing export dialog, with MSP1 as
the default. One background preparation produces the retained encoded bytes and
an actual decoded preview; changing the version retires the previous result and
consent. Both exact stage reservations remain covered by the existing editing
service lease through review, destination selection and guarded publication.
Actual flatten/hidden-RGB changes require explicit approval. Export neither
retargets the document nor clears its dirty state, and source protection remains
after saving a PCND copy. A delayed picker cannot replace a newer unrelated modal.

Local macOS ARM headless App proof passed 13 focused tests and five existing
regressions, including English/Japanese V1/V2 saves, all-seven-locale dialog
handoff, exact admission/cancellation lifetime checks and a post-stage source
alias refusal. The actual V2 chooser/export matched the independently
Pillow-verified nonuniform 2048×3 whole-file golden; pinned Pillow 13.0.0 also
verified all five saved outputs. Strict, formatting and translation checks passed.
Local ignored evidence is at
`docs/validation/native-msp-export-2026-10-27/e435e696-fd82-4daf-8cbb-d7de46bf2887/receipt.json`.
This does not establish OS picker/GUI, other native OS or original Windows Paint
execution, nor immunity to hostile namespace races beyond the final checkpoint.
Browser MSP2 version selection remains pending.

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
The owned [CSH specimens and optional oracle](../crates/services/paintcraft-formats/tests/csh-fixtures/README.md)
now exercise five shapes, 45 records, 10 subpaths and 20 knots. The upstream
JavaScript `ag-psd` 33.0.0 reader reopened both source specimens and fresh Rust
exports and matched all expected metadata/geometry values. A separate wire
comparison checked exact bounds and record bytes. This is additional
implementation evidence from the same library ancestry, not independent-lineage
or Adobe application qualification.

Studio's **File > Shape resources** dialog imports and exports a complete library,
browses individual shapes and previews their cubic path outlines. The explanation
that fill and Boolean operations are preserved but not rendered in that outline
gallery is visible in all seven languages. Selecting a shape changes only the preview;
it does not publish document pixels, dirty the painting or create an undo entry.
Previews above 4,096 knots are explicitly unavailable; exact resource data stays
available for export. The native picker is parented and protects the original
resource and open document source paths, including other tabs and parked
workspaces. Stale picker results are refused. Browser import likewise rejects
stale callbacks and exports a fresh download without changing document title or
save state. Ordinary image Open and Save do not accept CSH.

Native destination admission also protects currently loaded ASE/CSH sources from
project Save As, flat image exports and exports of the other resource kind.
Checks cover source path namespaces and current hard-link/symlink aliases, and
run again after replacement confirmation and deferred save preparation. Normal
Save to the painting's own bound PCND file remains allowed. Route-level tests
cover reciprocal resource exports, cancellation, late alias retargeting and
documents parked in other tabs/frontends. Resource acquisition retains a
read-only native handle to the exact regular file used for the bounded byte
read. Save/export admission compares that frozen identity as well as the
selected namespace, so renaming a loaded source or replacing its old name does
not remove protection of the acquired file. Existing regular destinations whose
identity cannot be compared are refused; ordinary FIFOs/devices are not opened
for comparison. Tests cover real renamed-source Save As/PNG writes, new hard
links, namespace replacement between acquisition and decoding, permission
failure, cancellation and normal painting saves. These checks do not establish
race-proof filesystem traversal or qualify every target filesystem.
If a project save was queued first, adopting an overlapping resource source is
refused until that write finishes, including writes owned by other tabs. The
earlier authorized save still completes; the previous resource stays loaded.
Unrelated resource imports remain available. The pending-save rejection is
translated in all seven languages.

CLI `shape inspect` and MCP `inspect_shapes` expose original bounds, ordered
records and exact normalized signed integers. `shape export` and `export_shapes`
write a fresh resource copy, separate from a painting's save acknowledgement.
Typed inspection is documented by `schema shape-inspection`. Resource
editing, multiple/open/Boolean-path placement, persistent
catalogs, broader interoperability and original-application qualification remain
for F14.

The local optimized Wasm build passed a headless Chrome import/select/preview/
download check. Its 1,466-byte fresh export reopened with the optional upstream
reader and matched the source geometry and metadata; the painting stayed
`Untitled - PaintCraft` with dirty state false. Ten focused shared/native/browser
tests cover cubic control geometry, English/Japanese controls, narrow layout,
source protection and stale/cancelled imports. These checks do not qualify
original Adobe applications or promise interaction latency.

A separate **Place shape…** form now provides X/Y, positive width/height, fill
color, an actual filled-mask preview and explicit conversion consent. It admits
one closed path with 2..=256 cubic knots, None/Combine operation, empty initial
fill and even/odd or nonzero filling. Multiple paths, open paths, other Boolean
operations, clipboard placement metadata and out-of-range controls are visibly
refused. The original library is retained unchanged for resource export.
A separate **Use as shape tool…** review previews the selected preset and requests
consent for raster conversion on each future drag. Studio then draws the preset
with a drag box, live filled preview, current color, backwards drags and Shift
square constraints. Resource/library identity, preset index, conversion policy,
color and target are captured with the gesture, so preview and release resolve
the same request. An explicit stock-tool selection leaves the resource tool;
returning from the color picker restores its exact retained source. Temporary
Space-pan does not replace the tool. Neither placement workflow completes the
unsupported CSH geometry profiles.

The existing tiny-skia filler receives normalized coordinates multiplied by the
chosen target dimensions through f64 and then f32. Original bounds origins stay
metadata; the chosen X/Y defines painting position. Anti-aliased coverage is
clipped to the local target box, then translated and clipped to canvas, layer
and current selection before existing source-over mask execution. Pixels outside
the local box are absent from the converted mask; original paths remain exact.
For numeric placement, preview/configuration, resource epoch/index, document
owner and target are bound together; changes require a new preview/approval. Shape placement is one
Shape undo entry; first-layer creation is atomic and ineffective masks leave no
layer/history. Native acquired input identities follow accepted placements
through saved documents, other tabs and parked workspaces, including resource
replacement and renamed source aliases.
The adopted drag preset retains its immutable source independently of the
currently browsed library. Cancelling a new review, stale resource callbacks or
switching to Classic cannot adopt the wrong resource. Stock rectangle styling
is hidden while the custom preset is selected. Browser drag requests admit at
most 262,144 output pixels and 4096 per side before allocation; oversized
requests fail visibly in full, without downsampling or incomplete masks.
Native drag preview and publication resolve on the existing workers. Browser
drag conversion remains synchronous; the smaller pixel bound is not a latency
or hard-memory guarantee.

Caller budgets declare pixmap scratch and path input, capacity-owned span
storage, preview bytes and a retained frontend-copy lease. Arc clones share
storage charges. These are bounded ownership declarations, not a hard process
RSS limit; tiny-skia fill cannot be cancelled internally. Ten backend tests,
three Shape and five Brush editor regressions, shared builder/metadata tests,
two shared widget tests, seven native placement/review tests and seven browser
shape workflow tests pass. Additional native/browser controller tests cover
worker preview equivalence, picker ownership, hidden targets, cancellation and
whole-request browser pixel limits. Four shared resource-gesture tests cover
captured identity, Shift, backwards boxes and view-only pan. Full host suites
pass 458 native tests (9 ignored) and 185 browser tests. English/Japanese
controls and all seven catalogs are covered. Strict host frontend/editor/format checks and a production Wasm build
pass. Headless Chrome exercised CSH upload, unsupported assessment, a 64 × 48
cubic preview, consent, placement, Undo/Redo and a 789-byte PCND download with no
page errors. The downloaded project reopened through CLI; 2,400 independently
calculated interior pixels matched, recovered alpha area was 1840.494 against
analytic 1843.2 with tolerance 8, and the canvas outside the target box stayed
unchanged. A second production run exercised separate tool adoption, a live
canvas drag, Escape cancellation, exact preview/release crops, Undo/Redo and an
801-byte PCND download without page errors. Its observed (99, 80), 64 × 47 box
matched 2,340 analytic interior pixels, area 1801.365 versus 1804.8 (tolerance 8),
and unchanged pixels outside the box. The Wasm hash and scripts are recorded
with the ignored local evidence. See [CSH placement policy](CSH_PLACEMENT.md)
for precise limits and remaining work. This is focused workflow evidence, not original Adobe rendering
parity or full release qualification.

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
ratio on transparent padding. They use the shared root Selawik and Noto fonts and accept additional outline
font files. Desktop adds one file per selection; the browser can add several at
once. The five bundled files count toward the 32-file, 64-face and 64 MiB total
allowances, leaving up to 27 additional files. A rejected or cancelled selection
keeps the previous font set and approved preview. Successful addition or removal
clears the preview and consent; pending font selection disables Import. Preview
work runs only when requested, samples the full selected raster, and retains the
exact private converted document for import. Changing options clears the preview
and consent. Import remains disabled until the preview has been displayed and
the user accepts the reported conversion losses. Cancellation or a changed
document/profile cannot publish the prepared copy. Native SVG acquisition and
browser file-size checks use the 8 MiB SVG source allowance before loading bytes.
Native SVG and added font inputs retain identities acquired from the same open
files as their decoded bytes. These source guards protect pending previews and
follow converted documents through tabs and interface workspaces, including
renamed input files. Closing their owning document releases them. Raw font data
and font databases are dropped after raster import. This is source ownership
protection, not an arbitrary-filesystem TOCTOU guarantee or qualification of
other foreign formats after source renames.

Headless English/Japanese controls tests cover desktop and browser hosts; a
390 × 640 browser-host test verifies reachable consent and visible footer buttons.
Additional font-file tests render a renamed outline font, verify changed glyph
pixels and removed substitution loss, and cover stale/cancelled/malformed
selections without changing the painting. Native route tests exercise pending
input protection, source rename aliases, normal bound saving, other tabs and
parked workspaces, and New/Open/Close ownership. A production Wasm font-upload
flow in headless Chrome verifies invalidated approval, fresh consent, updated
title and PCND download with no page errors. These are focused workflow checks,
not full platform release qualification.
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
their loss is reported, with ICC omission identified separately. The sole unnamed
ICC resource is recognized without an omission loss only when its bytes exactly
match the reviewed shared sRGB2014 profile; mutated, other, named or duplicate ICC
resources retain explicit ICC-loss handling. Source numeric
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

### Internal fresh PSD/PSB writer qualification, 2026-10-26

The existing PSD owner now writes a bounded fresh RGB8 Normal raster-layer
subset through selected `ag-psd` 0.3.0. PSD and PSB are explicit choices. Raw
layer pixels, alpha (including hidden RGB), Unicode names, signed offsets, stack
order, visibility and all 256 opacity values are retained. No layer trimming or
inferred background conversion is allowed. A small framing-checked adapter
inserts the unchanged shared sRGB2014 ICC resource 1039; it does not replace the
selected layer/image serializer. An actual native document title requires
`psd-document-title-not-exported` acknowledgment.

Preflight checks the complete layer inventory, names (at most 1024 UTF-16 units),
bounds and aggregate storage before model or sink work. Limits include 256
layers, 4,194,304 canvas pixels and aggregate layer pixels, 64 MiB encoded output
and a default 512 MiB logical transient declaration. These declarations include
model copies, PackBits scratch and output growth; they are not an allocator/RSS
cap. Cancellation surrounds the selected library call and checks bounded copy
chunks; it is not cooperative inside that call. Staged-sink errors and logical
library panics discard private results. Consumers must still compose source and
owner checks with atomic publication.

Four owned PSD/PSB specimens were read independently by psd-tools 1.26.0 and
Pillow 13.0.0. Every raw layer sample/property and embedded ICC byte matches the
source vectors. Opaque composites are exact. For the partial-alpha fixture,
the decoded merged cache has exact alpha but RGB error up to 60 at low alpha;
its displayed error over opaque black or white is at most 1. Fresh recomposition
of the exact editable layers differs only in RGB at alpha zero; visible RGB,
alpha and opaque-backdrop output are exact. These measured cache/consumer
results are retained, not rewritten or presented as whole-file losslessness.
Source provenance, separate metrics and reproducible commands are in
`crates/services/paintcraft-formats/tests/psd-fixtures/README.md`.

Production CLI `export-layered` and MCP `export_layered` fresh-copy consumers
compose this writer with captured-source protection, loss preflight, staged
atomic output and operation receipts. They leave document/history/dirty state
unchanged. The registry qualification gate runs before input acquisition or
output staging and currently refuses these routes; title-loss consent is not a
bypass. This is implementation and internal qualification, not enabled public
export. Foreign preserving writes remain denied. Actual Photoshop open/edit/resave is
still a required gate, and groups, masks, text, effects, foreign blend modes and
high-depth output remain unsupported. PDN now has a separately qualified internal writer; PAINT also has an internal writer; public writing remains pending. These
increments do not complete original-application qualification.

### Internal fresh PDN3 writer and native preparation, 2026-10-26

The existing `pdn-lib` owner independently serializes the observed Paint.NET
5.1.12 Normal bitmap graph through inert MS-NRBF records and bounded deferred
gzip BGRA blocks. It does not patch graph templates, host .NET, load assemblies
or copy opaque source records. The typed API requires full-canvas RGBA8 layers,
explicit names/visibility/opacity/background roles and literal producer fields;
nonempty document/layer metadata is refused. Class/assembly identities follow
the pinned observed schema, while the producer version identifies PaintCraft,
not Paint.NET. Initial limits are 64 layers, 1..32768 dimensions, 4,194,304
aggregate pixels, 4096 UTF-8 bytes per NUL-free name and 64 MiB private output.

Independent `ms-nrbf-js` 0.1.0 and the unchanged pinned PaintDotJS decoder read
owned one/two/64-layer specimens under Node 24.10.0. Every raw sample, hidden RGB,
name, visibility, opacity, background flag, Normal blend, class/library identity,
stride, deferred-memory flag and producer component matches independently
constructed expectations. The harness substitutes inert DOM/ImageData storage
and the Normal enum; this proves parsing/byte conversion, not actual canvas
rendering or Paint.NET behavior. Fixture hashes, archive integrity, commands and
logical codec-storage audit are in
`crates/adapters/pdn-lib/tests/fixtures/README.md`. Tests also cover all 256
opacity values, incompressible multi-member growth, source limits, corruption
regressions, every observed cancellation checkpoint and failed staged sinks.

Native preparation inventories the entire flat Document/Layer model and owns
immutable exact layer buffers and policies. `Exact` refuses offsets or non-canvas
layer sizes. Explicit `Materialize` translates each layer into a full-canvas
buffer, preserving raw/hidden RGB within its bounded extent and using transparent
black padding only outside that extent. It does not flatten. Changed geometry
requires `pdn-layer-geometry-materialized`; source samples outside the canvas
require `pdn-outside-canvas-samples-not-retained`, including samples beneath zero
alpha. The preparation report records policy and discarded sample count. Checked
128-bit extent arithmetic handles extreme signed origins. Source pixels/resident
tile bytes, prepared full-canvas pixels and combined logical transient storage
are admitted separately before allocation.

Ordinary native layers faithfully map to `isBackground=false`, without an
invented loss. Choosing an explicit bottom-background role adds target behavior
and requires `pdn-background-role-assigned`. An actual native document title
requires `pdn-document-title-not-exported`. Names/order/visibility/opacity and
Normal blend remain unchanged; unsupported future document semantics require
review through exhaustive model matching. These are converted fresh copies,
never preserving foreign saves.

The existing CLI `export-layered` and MCP `export_layered` routes now prepare
PDN using explicit background-role policy and optional explicit materialization.
They reuse captured-source protection, staged atomic output, publication-barrier
checks and unchanged native document/history/dirty state. The common file-operation
receipt is unchanged; preparation policy is additional CLI engine output or a
second MCP text block. **The registry still refuses public PDN writing.** Tests
exercise post-admission staged copies separately and prove that public CLI/MCP
requests, even with loss consent, cannot bypass missing qualification. Actual
pinned Paint.NET open/edit/resave remains required; the secondary JavaScript
oracle does not satisfy that gate. PAINT public qualification and newer PDN schemas remain
separate unfinished work. No broad layered-format completion is claimed.

### Internal fresh PAINT writer and native preparation, 2026-10-26

The existing `paint-lib` independently writes the observed MIF1 primary-`iovl`
profile with raw-DEFLATE `unci` layers. New deterministic IDs/boxes, essential
property associations and non-overlapping data extents are built from staged
sizes, not patched upstream templates. Layer dimensions, signed placement,
ordered raw RGBA8 (including hidden RGB), literal item names and all 16 canvas-fill
bits are retained. `nclx` explicitly declares full-range sRGB (1/13/0), while
`cmpd`/`uncC` identify unsigned RGBA8 components and BGRA byte order. There are
only `dimg` references, with no `prem` association. Those packet declarations and
the audited reference default support the straight-alpha profile; returned C#
bytes alone are not used to infer color/alpha semantics. Visibility/opacity or
unknown metadata extensions are refused rather than fabricated.

Independent Python box/extent/property/zlib checks and unchanged pinned
mspaintlib C# sources read three owned specimens with differing layer sizes,
16/32-bit overlay fields, wide/extreme signed offsets, all alpha boundaries and
exact raw fill16. The C# sources target net8 upstream but were compiled privately
for available SDK 10.0.401/runtime 10.0.12; no .NET dependency is added to the
product. Original `IovlItem.Parse` retains fill16. The high-level reference
`CanvasFill` narrows to eight bits, and its ASCII-name reader corrupts UTF-8 item
names. Those negative limitations are retained; source/wire names and samples
were not changed to force parity. Python independently verifies correct UTF-8
bytes. Exact source hashes, reference controls, reproducible commands and logical
storage audit are in `crates/adapters/paint-lib/tests/fixtures/README.md`.
Ordinary CI binds current encoder output to these independently verified bytes.

Native preparation inventories every Document/Layer field and freezes its own
raw layer buffers, original extents/origins and explicit policies. It never
clips, pads, resamples or flattens. Hidden/non-full-opacity layers and signed32
overflow are refused. Source and prepared aggregate pixels, resident tile bytes,
encoder/output storage and combined logical transient memory are bounded before
allocation. Current caps include 64 layers, 4,194,304 canvas pixels and aggregate
layer pixels, and 64 MiB private encoded output. These declarations are not an
allocator/RSS cap or hard cancellation-latency guarantee. Checks surround bounded
row/codec/staged-copy work; immutable buffers and publication source guards remain
in force through the existing atomic save coordinator.

The native model has no separate canvas-fill property. Explicit transparent
black preserves its clear composition; another RGBA16 fill adds background
behavior and requires `paint-canvas-fill-assigned`. UI names use an explicit
`require-empty` or `omit` policy. Actual nonempty name omission requires
`paint-layer-names-not-retained`; the adapter does not guess that an `infe` item
name is a Windows Paint UI name. Actual title omission requires
`paint-document-title-not-exported`. Clear fill and empty names incur no invented
loss. The preparation report carries exact fill16, name policy/count and retained
geometry alongside the unchanged file-operation receipt.

CLI `export-layered --format paint` and MCP `export_layered` now have those
explicit options and a real staged-copy path. Tests cover exact raw pixels and
positions, fill16, loss refusals, source aliases after rename, source/history
preservation, cancellation rollback and cleanup. **The unchanged registry still
refuses public PAINT writing.** Actual Windows Paint open/edit/resave remains
unproven, including UI metadata and build-specific semantics. The independent
reference checks do not satisfy that original-application gate. This is bounded
backend/consumer preparation, not completion of PAINT or the layered-format pack.

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

### Browser JPEG XL document import, 2026-10-26

Studio **Import image…** admits only `web-jxl-srgb8-worker-quarantine-v1`,
using the same metadata-free static, identity-oriented original sRGB8 decoder
profile as native JXL. Classic and ordinary Open remain denied. The public rfd
File is retained, and its size is checked before reading; at most 64 MiB input
and 4,194,304 pixels are admitted. Vite owns the real lean Worker/Wasm assets.
The Worker has a 256 MiB linear-memory maximum; this is not a browser RSS cap.

The picker captures the complete Rust Session/document/frontend/draft owner.
Rust independently binds response owner, nonce, source digest/length, dimensions,
application version and exact pixel payload to the captured request. Document
assembly charges padded tiles to the existing resident realm before releasing
the validated preparation. Adoption creates a new dirty project, keeps user
tool preferences and begins with no imported undo history. PCND downloads use
a project suffix and retain exact straight RGBA, including hidden RGB. A browser
download never claims disk persistence or marks the tab saved.

Cancellation, stale ownership, traps and malformed responses leave the current
project intact. When completion cannot be proved, the tab retains its logical
codec reservation and refuses further codec jobs until reload. A persistent
message explains how to download the project first; ordinary PCND saving remains
available. A validated completed profile refusal releases the reservation and
permits retry. Worker termination has no join/reclamation API: this policy does
not establish synchronous execution stop, physical memory reclamation or a hard
aggregate browser memory bound. Earlier failed execution-stop probes remain
preserved; this workflow does not change their conclusions.

The production `scripts/jxl-production-browser.mjs` proof exercised actual
Studio upload, new-document adoption, empty Undo, PCND download and clean reopen;
unsupported 16-bit refusal followed by valid retry; UI cancellation, service-owner
invalidation, trap and malformed Worker response. Each failure scenario first
opened the nonempty independent 3×2 PCND project. Each uncertain failure retained
its charge, refused a new Worker and still downloaded that project. Chrome
155.0.8059.85/CDP metadata, production asset/source hashes and screenshots are
retained in `docs/validation/jxl-browser-2026-10-26/attempt-sHaPOK/`. Independent
fixture samples were checked after the imported, empty-Undo, reopened and all
four failure-preserved PCND downloads, including alpha 0/1/127/128/254/255 and
hidden RGB. Host tests separately
exercise full Rust owner changes and response-frame corruption. This is evidence
for the stated profile in one actual browser, not cross-browser conformance.


### DCX fresh copies, 2026-10-26

The bounded typed DCX writer emits 1..1023 ordered independent PCX pages using the
selected PCX encoders, with complete page validation before encoding. Aggregate
pixels, output, offsets and logical transient storage are bounded; supplied palette
order and exact indices are retained. This is not a document-page or animation model.

Native Studio CLI `export-dcx` and MCP `export_dcx` prepare exactly one document
composite through the existing PCX flat-export policy. An explicit `rgb24`,
`indexed-exact` or `quantized256` mode is required. Actual flatten, hidden RGB,
white-matte alpha removal and palette-quantization losses require acknowledgment.
Captured PCND input identity and the MCP retained native-source identity protect
renamed/hardlinked inputs; the existing staged-save publication barrier rechecks
source aliases after encoding. Copies leave document/dirty state unchanged. MCP cancellation and dropped-request
lifetime checks run during preparation, staged chunks and publication, separately
from filesystem identity checks.
Generic Open, preserving Save, Classic stock support and browser exports remain
unchanged; desktop/browser have no new DCX export controls.

Untouched ImageMagick 7.1.2-32 Q8 independently decodes all nine typed RGB/indexed/
mixed pages, preserving distinct palettes, duplicate-color index identity, order,
DPI and exact RGB. It also independently verifies all six actual CLI/MCP production
outputs. Pillow 13.0.0 verifies every bounded PCX page and RGB DCX pages, but its
whole-DCX earlier indexed-page EOF palette lookup produces wrong colors. That
negative evidence remains recorded; fixtures were not changed to hide it.
See `tests/dcx-fixtures/writer/README.md` in the formats crate and
`apps/paintcraft-cli/tests/dcx-consumer-fixtures/README.md` for hashes, raw reports,
reproduction commands and admitted limits. Independent consumer evidence does not
claim every historical application or arbitrary PCX profile compatibility.

### Neutral OpenRaster group copies, 2026-10-26

The exact `ora-neutral-auto-flat-copy-v1` profile admits explicit Studio Import
only. It accepts omitted/exact full group opacity, explicit `isolation="auto"`,
Normal blending and a bounded typed tree. Depth-first leaf ordering and ancestor
visibility are preserved without an intermediate group buffer. Group hierarchy,
names and separate visibility controls are discarded only after explicit
`ora-group-structure-not-retained` consent in all seven languages. A checked merged
projection must agree with the existing encoded RGBA8 compositor; a valid but
different projection is an unsupported rendering policy, not archive corruption.

The old generic/flat reader remains strict. Ordinary Open, Classic, editable
hierarchy and preserving ORA Save are not enabled. Native/headless regular files
retain same-descriptor source identity and raw-byte hashes through consent and
publication, with source-alias protection after PCND Save/rename. CLI/MCP legacy
flat stream snapshots select the prior strict profile before decoding; grouped
streams are refused. Extensionless PK-prefix probing preserves the generic
allowance for non-ZIP formats, and unrelated ZIPs never become eligible ORA.

Owned stdlib ZIP/PNG fixtures provide independent ordered leaf records, hidden
RGB, signed origins, effective visibility and Fraction-composed expected pixels.
Native/browser-host EN/JA reviewers, real CLI/MCP consent and rollback, PCND
roundtrip, source mutation and post-save aliases are covered. The browser-host
tests and actual Wasm build are distinct from the separate headless production
picker smoke: real archive upload, visible limitation review Cancel/approve and
PCND download. An independent Python PCND reader checks every imported leaf hash
and property, hidden RGB, and the Fraction-composed merged pixels. Source/assets,
Chrome version and all failed harness attempts are retained separately.
Original GIMP/Krita opening/editing/resaving and broader group semantics remain
unqualified. See `tests/ora-group-fixtures/README.md` and ignored
`docs/validation/ora-consumers-2026-10-26` for exact source/artifact scope.


### Native DDS texture copies, 2026-10-26

The exact `native-dds-texture-v1` profile is Native + Studio + ExportFlat with a
`.dds` destination and required preflight. It is used by native Studio's DDS
options, CLI `render --dds-*` and MCP `export_image` with nested `dds` settings.
No DDS options retains the old single-level DX10 RGBA8 sRGB straight-alpha
exporter and its bytes. Classic admission is unchanged. Advanced browser export
is still pending; its existing default DDS exporter remains available.

One immutable native sRGB document composite is admitted and rendered before
encoding. Fresh output supports RGBA8, opaque BC1, or straight-alpha BC2/BC3;
BC1 refuses any nonopaque composite rather than inventing cutout alpha. Output
is DX10, one 2D surface, at most 4,194,304 base pixels, 65,536 pixels on either
side, 64 MiB encoded bytes and 17 geometry-bounded levels. The selected `dds`
0.2 encoder uses Normal or High effort, a Uniform error objective, no dithering
and serial encoding. These choices do not promise a universal color/alpha error
bound. BGRA output, BC1 cutout, associated/custom alpha, other BC formats and
source swizzles are not part of this profile.

Native samples are encoded sRGB. Choosing `linear8` performs the pinned
`srgb8-to-linear8-nearest-ties-up-v1` conversion into 8-bit linear-light sRGB
primaries; it does not retag the source bytes. Quantization differences from
exact continuous linear light require `dds-srgb-linear8-quantization` consent.
The current public DDS importer still refuses linear transfer; exporting
linear8 does not enable reopening it as if it were sRGB. Studio's preview
converts the decoded base samples for display, rather than showing linear8 bytes
as display-encoded values.

Mip choices are single, full, or an explicit total count including level zero.
Dimensions floor-halve to at least one. All smaller levels derive directly from
the immutable base: Nearest samples destination centers; Box uses exact area
weights, averages alpha and alpha-weighted stored RGB, rounds nearest with ties
up, and writes zero RGB when weighted alpha is zero. Box in sRGB is explicitly
stored-sample filtering, not linear-light filtering. Generated mips are a stated
output policy, not an invented document-data loss. Actual Box hidden-RGB
normalization needs `dds-mip-hidden-rgb-normalized`; BC needs
`dds-block-compression`; common flatten/hidden-RGB losses also apply when
reported. Native documents have no stored custom-mip inventory to preserve.
Foreign preserving Save remains denied.

Studio retains the actual encoded artifact, its decoded base preview and its
resource lease through loss review, destination choice and staged publication.
Changing settings retires the old preview and consent. Source aliases,
state/history/frontend/tab or draft changes, cancellation and changed output
fingerprints refuse publication without retargeting or clearing the native
project's dirty/saved binding. CLI/MCP use the same preparation and guarded
staged-copy boundary. CLI's explicit DDS path currently requires one render
worker; the absent-options parallel renderer remains unchanged. Resource quotes
cover logical live payloads, not a hard whole-process RSS guarantee.

Independent backend evidence comprises 96 Normal/High specimens and 416 mip
surfaces, exact rational filter/layout checks, and Pillow 13.0.0 decoded sample
measurements. Original sRGB BC wrappers were refused by Pillow; that negative
result is retained. Original DXGI/alpha declarations are checked independently,
then separately authored UNORM wrappers verify each BC payload's samples only.
They do not establish original sRGB-wrapper acceptance, GPU sampling or transfer
correctness. Both efforts retain six failures against the original exploratory
joint RGB max96/RMS32 target. Those thresholds were not raised or failures
renamed as passes; the explicit Uniform lossy policy records actual per-vector
errors and frozen goldens without a universal quality promise. See
[backend evidence](../crates/services/paintcraft-formats/tests/dds-texture-write-fixtures/README.md)
and the independent [all-256 transfer proof](../crates/services/paintcraft-formats/tests/dds-transfer-fixtures/README.md).

Actual CLI/MCP tests produced 17 whole files / 68 mip surfaces independently
checked with Pillow, plus a nonuniform opaque 7×5 CLI copy identical to the
independently qualified BC1/sRGB/Box/Normal whole-file golden (distinct from
Nearest). Native App headless English/Japanese tests exercise all four encodings,
visible settings and loss review, Linear8 preview, failed BC1-to-BC3 recovery,
quota recovery, cancel/stale/source/destination guards, and the same nonuniform
whole-file comparison. This is local host UI/CPU evidence, not a production
browser run, original texture application's reopen, six-platform runtime matrix
or benchmark. See [consumer proof and reproduction](../apps/paintcraft-cli/tests/dds-texture-export-fixtures/README.md).
