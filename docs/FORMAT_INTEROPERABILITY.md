# Format interoperability implementation

This ledger follows `docs/temp/paintcraft-format-interoperability-classic-studio-v3` alongside the canonical editor and PCND specifications. It records implemented operations as of 2026-10-25, not completion of the entire planning pack.

Codec implementation, enabled frontend operations and release qualification are separate gates. `paintcraft_formats::registry` controls discovery and admission; individual adapters enforce their narrower file profiles and resource limits. A listed format does not imply support for every variant, metadata field or original application version.

## Available workflows

Desktop and browser use the same frontend policy. CLI and MCP import/export operations also apply that policy, using an explicit frontend profile rather than inheriting every compiled codec. All foreign conversions require acknowledgment of their reported losses before publication. Failed, cancelled or stale conversions leave the existing document intact.

- PCND is the native layered project format and the preserving Save destination.
- Classic 7/10/11 open and export PNG, JPEG, BMP, still GIF and single-page TIFF. All Classic skins can explicitly import the admitted PCX subset. Classic 11 can explicitly import the admitted PAINT subset.
- Studio also opens TGA, still WebP and admitted single-surface DDS images, and opens the admitted historical PDN subset as a converted copy.
- Studio's explicit Import image workflow adds QOI, Netpbm, farbfeld, PCX, selected DCX pages and the admitted PAINT subset. These extended imports do not silently expand ordinary Open or Recent filters.
- Studio offers **13 flat exports**: PNG, JPEG, BMP, GIF, TIFF, TGA, WebP, JPEG XL, DDS, AVIF, QOI, PAM and farbfeld. PAM output always uses `.pam`, never a `.ppm`, `.pgm`, `.pbm` or generic `.pnm` filename.
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
| PCX | Classic/Studio explicit Import only | Version 5 RLE: admitted packed 1/2/4/8-bit, planar 1-bit and RGB24 profiles; minimal even row padding; strict row/plane bounds. Palette and metadata loss explicit; older/ambiguous variants refused; no writer |
| DCX | Studio explicit selected-page Import only | Strict bounded page directory, at most 1023 pages; each page must satisfy PCX admission. Other pages are explicitly not imported; no writer |
| PDN | Studio Open or explicit Import as a converted layered copy | Historical PDN3 normal-blend, same-sized full-canvas bitmap layers only; names/order/visibility/opacity/pixels retained, metadata loss explicit. No preserving or fresh writer |
| PAINT | Studio and Classic 11 explicit Import as a converted layered copy | Observed MIF1 `iovl`/`unci` DEFLATE RGBA8 profile only; offsets/order retained, layers numbered/visible/full-opacity because source properties are unverified. Canvas fill becomes a layer, with depth loss when needed. No writer |
| JXR, HEIF | No enabled frontend operations | Selected/backend qualification remains; HEIF writing is prohibited |
| ORA, PSD/PSB, SVG, ICO/CUR, EXR/HDR, JP2, MSP, ICNS | Not yet enabled | Registry inventory or work in progress is not usable format support |
| ABR, ASE, CSH | Asset workflow policy only | Brush/palette/shape import/export implementations remain |
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
- Native/browser import, export and DCX page-dialog tests cover translated controls, explicit consent, stale owners and cancellation. CLI format-profile/foreign-import and MCP tests cover real command consumers, page options and loss acknowledgments. These headless checks do not replace live browser, native reference or cross-platform release testing.

Latest specialist raw oracle outputs, tool versions and excluded outcomes are retained under ignored `docs/validation/format-specialists-2026-10-25`. Source tests and checked-in fixtures remain the reproducible default evidence. Optional external-tool tests are invoked separately and are not silently counted as default-suite passes.

## Historical performance baseline

The 2026-10-24 `raster_codecs` baseline has 80 Divan cases, at least 30 samples each. The CLI Hyperfine baseline has 64 workloads, ten measured runs and two warmups each; all 640 measured executions succeeded. Independent preflight reopens 40 projects and checks pixels, source hashes and eight TIFF alpha declarations. Raw JSON, sample CSV, fixture/tool hashes and excluded observations remain under ignored `benchmarks/results/format-raster-2026-10-24`.

For those historical 1000x486 workloads, warmed CLI import medians span approximately 24 to 61 ms and export medians 18 to 39 ms. Dense GIF encoding in the earlier quiet-build baseline took approximately 293 to 333 ms, versus approximately 6 to 10 ms for different palette/sparse cases. Other user workload was uncontrolled. These are workload observations, not speedup comparisons, interactive latency measurements or measurements of the new specialist formats. Historical TIFF cells using the old alpha-ambiguous writer remain retained but are superseded for current-profile qualification.

No PNG/JPEG backend has been switched to Zune, and no performance improvement is claimed merely from adding a dependency.

## Remaining interoperability work

- Qualify JPEG XL reader confinement and cancellation tails, JXR independent comparisons and a decoder-only HEIF profile.
- Finish and qualify extended document/asset adapters, including ORA, PSD/PSB, SVG rasterization with explicit dimensions/DPI, icons and Adobe resources. EXR/HDR require precision-aware storage/import gates; JP2 and historical MSP need dedicated evidence.
- Add separately qualified PDN/PAINT fresh and preserving writers, source-version coverage and original-application reopen evidence.
- Complete full browser/native reference and packaging matrices, worker failure and aggregate-memory accounting, and real-device cancellation/latency qualification.

The implemented subsets advance the pack; they do not complete its full interoperability, preservation or release gates.
