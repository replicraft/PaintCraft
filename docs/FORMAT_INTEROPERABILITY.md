# Format interoperability implementation

The implementation follows `docs/temp/paintcraft-format-interoperability-classic-studio-v3` alongside the canonical editor and PCND specifications. The planning pack is not another workspace or a replacement CLI schema.

An implemented codec is separate from an enabled frontend operation and from release qualification. Discovery comes from `paintcraft_formats::registry`; callers still run the bounded selected decoder and resolve conversion losses before publication. No new layered format is advertised merely because its family appears in the registry. Adapter qualification here remains narrower than release, original-application and cross-platform qualification.

## Frontend contracts

- Classic 7/10/11 retain raster-oriented workflows. The planned common formats are PNG, JPEG, BMP, still GIF and single-page TIFF. Native `.paint` belongs to the qualified Classic 11 profile. Classic does not acquire Studio's vector or foreign-project imports.
- Studio's stock inventory is exactly PDN, PNG, JPEG, JXL, JXR, BMP, GIF, TGA, DDS, TIFF, HEIF, WebP and AVIF. PCND is PaintCraft's additional native format.
- HEIF/HEIC writing is denied, including when an encoder happens to exist. SVG export is denied. SVG import is a separate Studio rasterization operation.
- PAINT exchange, PSD/PSB, ORA, ICO/CUR, QOI, EXR/HDR, JP2, PCX/DCX/MSP, PNM, farbfeld and ICNS are separate Studio import profiles. ABR brushes, ASE palettes and CSH shapes use asset workflows, not document Open.
- RAW, PDF, KRA, XCF, animation and advanced multi-surface DDS remain separate future profiles.

## Current delivery ledger

| Family or operation | Implemented scope | Frontend availability and remaining qualification |
| --- | --- | --- |
| PCND | Existing bounded native reader, canonical writer, compact sparse chunks and recovery integration | Existing native workflow retained; strict container validation follows candidate admission |
| PNG, JPEG, BMP | Existing bounded RGBA8 conversion and explicit flat-export losses | Existing qualified adapter subset; no claim that every alias, metadata profile or codec variant is supported |
| GIF | Bounded single-frame, full-logical-screen admission and RGBA8 conversion; palette export with exact alpha/hidden-RGB loss reporting | Read operations enabled in Classic/Studio profiles after explicit loss consent; frontend exports remain disabled |
| TIFF | Bounded single-page integer grayscale/RGB/RGBA subset, explicit precision/metadata losses, RGBA8 writer | Read operations enabled after loss consent; multipage, CMYK, floating point, ambiguous/associated alpha and unqualified orientation variants refused; frontend exports remain disabled |
| JXL | Independent `jxl` decoder and lossless `zune-jpegxl` writer adapter, native 8/16-bit sample representation | Internal adapter only; independent libjxl pixel oracles pass for the documented subset; hostile-input confinement and frontend integration remain pending |
| PDN | Tested bounded inert historical PDN3 XML and body-header inspection | Header inspection is not NRBF object reconstruction, layered decoding or preserving Save; current Paint.NET profiles and original-app reopen tests remain pending |
| PAINT | Tested bounded ISO-box traversal and file-relative extent/overlap checks | Brands remain hints; item/property/layer reconstruction and original-app qualification remain; no project decoder or writer advertised |
| TGA, WebP | Bounded integer TGA and still WebP adapters, lossless RGBA8 writers, independent libwebp fixtures/oracle | Internal adapter subsets; animation and unqualified extensions refused; frontend integration remains |
| DDS | Bounded single-surface RGBA/BGRA/BC1/BC2/BC3 decode, explicit transfer/alpha DTO, exact RGBA8 writer with explicit transfer and straight alpha | Internal adapter only; arrays, mipmaps, cubemaps, volumes and unsupported precision refused |
| AVIF | Bounded encoder-only `image`/ravif adapter, native and actual Wasm runtime checks | No decoder or frontend route; lossy even at quality 100, alpha/hidden-RGB loss reported; opaque codec scratch and cancellation tails remain unqualified |
| JXR, HEIF | Selected backend and platform/profile work remains | No enabled operations; HEIF writing remains prohibited |
| ORA, PSD/PSB, icons, QOI, EXR/HDR, JP2, PCX/DCX/MSP, PNM, farbfeld, ICNS | Registry policy inventory | No codec implementation inferred from inventory |
| ABR, ASE, CSH | Distinct asset operation policy | Resource import/export workflows remain |
| RAW, PDF, KRA, XCF, animation, advanced DDS | Future specialization policy | No inherited Classic support or preserving-write claims |

Native launch, Open, Recent and drop, browser Open, and CLI import/render now share frontend admission rules. The native launch path loads persisted frontend settings before opening its initial file. Failed initial Open leaves a fresh untitled document rather than binding the rejected path as a Save destination. Encoded source acquisition is bounded. Filename extensions are hints, not proof of content.

The native import workflow stages decoded source pixels and requires pre-publication loss consent. Browser imports use matching owner/session/profile guards. Cancellation or a stale confirmation leaves the current document intact. Registry visibility remains narrower than codec presence. GIF/TIFF read operations now pass actual native/browser confirmation and CLI acknowledgment tests; their frontend exports remain disabled. Existing CLI export acknowledgments remain mandatory; capability output retains the canonical `paintcraft.build-capabilities/v1` envelope and does not claim release qualification.

## Selected routes for remaining work

These are candidates from the interoperability pack, not dependencies to add without an implemented operation:

| Work | Selected route and qualification boundary |
| --- | --- |
| PNG/JPEG specialization | Zune only after exact-profile correctness and comparable benchmarks justify replacing the current adapter |
| DDS | `dds`; qualify one 2D surface first, refuse unselected arrays/cubemaps/volumes |
| JXR | `jpegxr-pure-rs`; experimental backend requires independent JXRLib comparisons |
| HEIF | Evaluate `heic`, `heifer`, `heic-decoder` or optional libheif; decoder only |
| PSD/PSB and Adobe assets | Rust `ag-psd`; separate layered and ABR/ASE/CSH workflows; no invented deep-color preservation |
| PCX/DCX/MSP | `pcx` for pixels, bounded in-tree DCX pages, specimen-backed historical MSP parser |
| ORA | Existing bounded ZIP services plus safe XML and qualified PNG layer decode |
| AVIF | `image`/ravif/libavif, with decode and encode packaging qualified separately |
| EXR/HDR | Precision-aware storage and import gates before exposing a decoder |
| SVG | Existing `resvg`, explicit Studio target size/DPI and rasterization consent |
| ICO/CUR | Multi-resolution and hotspot-aware asset adapter |
| JP2 | Dedicated OpenJPEG or equivalent after packaging and security qualification |

TIFF writing uses the same `tiff` codec underlying `image` directly for one required control: explicit unassociated alpha (`ExtraSamples=2`). The generic image facade omitted that tag. The reader refuses ambiguous extra channels, duplicate fields and unsupported pixel models. Raster admission also checks allocation address-space limits before 16-bit expansion on 32-bit hosts. Partial GIF frames are refused until logical-screen background composition has an explicit qualified policy.

## JPEG XL boundaries

The decoder accepts static integer sRGB samples at 8/16-bit precision. Animation, float/HDR, custom ICC conversion, non-alpha extra channels and associated alpha are refused. The writer accepts lossless two-dimensional RGBA8 and opaque RGB16. Single-row/column writes are refused because the selected writer rejects those dimensions. Non-opaque RGBA16 writes are refused because its 0.5.2 alpha header can corrupt precision.

Input/output and geometry admission do not constitute a hard codec scratch-memory limit. The selected decoder exposes no host allocator hook, and its internal ICC allocation bound is larger than the adapter metadata bound. Process confinement and cancellation-tail qualification remain open. See the checked-in synthetic fixtures and provenance in `crates/services/paintcraft-formats/tests/jxl-fixtures`.

## Evidence and measurements

- Native confirmation tests include 16 English/Japanese Classic/Studio GUI configurations; browser confirmation tests include 12 Classic GUI configurations. CLI tests cover all four frontend profiles, missing loss acknowledgment and disabled GIF/TIFF export.
- Registry tests cover frontend/operation policy, stock inventory, suffix/signature conflicts, unavailable variants and mandatory write prohibitions.
- GIF/TIFF tests cover synthetic external byte layouts, truncation, pixel/metadata limits, variant refusal, exact supported pixels and export loss acknowledgment. They do not establish Windows Paint compatibility for every file profile.
- DDS tests use hand-authored channel/block fixtures and negative surface/precision/budget checks. Transfer and alpha mode remain explicit instead of silently claiming color conversion.
- TGA tests cover orientation, RLE and palette indices; WebP has independent tiny C libwebp fixtures and an explicit external writer oracle.
- AVIF encoder checks include native independent FFmpeg/libdav1d decode and an actual Wasm encode smoke. Native decoder availability is not inferred from the encoder.
- JPEG XL tests include independently generated libjxl fixtures and an additional explicitly invoked external `djxl` oracle. The normal test suite runs offline using the small checked-in fixtures.
- `raster_codecs` has an independent correctness preflight and a Divan baseline of 80 cases, at least 30 samples per case. Raw runs, hashes, sample CSV and excluded/provisional observations are retained under ignored `benchmarks/results/format-raster-2026-10-24`. Historical TIFF cells used the pre-fix alpha-ambiguous writer and are explicitly superseded for current-profile qualification; they remain in the raw evidence. The CLI fixture generator now verifies explicit unassociated alpha.
- In the captured quiet-build baseline, before later admission hardening, dense 1000x486 GIF encoding took approximately 293 to 333 ms; palette/sparse cases took approximately 6 to 10 ms. These are workload observations, not optimization gains or semantically equivalent cross-format comparisons. Other user workload was uncontrolled.
- The current CLI baseline has 64 workloads with 10 measured runs and two warmups each: all 640 measured executions succeeded. Independent preflight reopens 40 imported projects and checks decoded pixels, source hashes and all eight new TIFF alpha declarations. For 1000x486 fixtures, warmed import medians span approximately 24 to 61 ms and export medians 18 to 39 ms. These are different workloads, not speedup comparisons or interactive latency measurements. Raw Hyperfine JSON, derived CSV, frozen CLI/fixture hashes and observations remain in the same ignored results directory. The postprocessor was corrected for Hyperfine 2 records after timing; raw runs were retained and no samples removed.

No PNG/JPEG backend has been switched to Zune, and no performance improvement is claimed from adding a dependency. PDN/PAINT writers require successful reopen by pinned original Windows applications, beyond reading back with PaintCraft itself. Native reference applications and their fixture qualification are still required.

## Validation snapshot

The final format suite passed 136 tests, with four external-oracle/subprocess tests excluded from its default run. The explicitly invoked libjxl, libwebp and FFmpeg oracles passed separately. PDN and PAINT inspectors passed nine and eight tests respectively.

Native unit coverage passed 341 tests. The broader native GUI run passed 273 tests and exposed three obsolete Save fixtures that assumed a rejected startup file became the Save destination. Those fixtures now select an explicit scratch destination or open a valid bound project; all three targeted reruns passed. An existing thumbnail cancellation test also gained a worker-start handshake so it tests an executing worker deterministically. Web coverage passed 90 unit tests plus its preview integration test; CLI coverage passed 31 tests.

Strict Clippy passed for all modified native targets and the actual `paintcraft-web` Wasm graph. Formatting, source files under 800 lines and 950 messages across all seven locales pass. Fresh browser output and the macOS debug bundle were built; the macOS bundle passes deep strict signature verification. Headless validation did not launch application windows. Selected logs and a hash manifest are retained under ignored `docs/validation/formats-2026-10-24`.

Remaining work proceeds through the pack's stages: finish standard-raster frontend/export workflows and preserving targets, qualify codec confinement and specialist platforms, reconstruct PDN/PAINT layers, then add extended document and resource imports. Registry inventory alone does not complete those stages.
