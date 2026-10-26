# EXR bounded preflight qualification

Eight owned fixture files were produced and reopened by official OpenEXR 3.5.2 in an isolated Python 3.14 environment with NumPy 2.4.3. `oracle.py` contains the complete source pixels and writes the `.exr`, interleaved RGB/RGBA float32 little-endian reference bytes, and hashed `manifest.json`. The Rust EXR crate does not participate in this oracle. The current Rust preflight does not decode samples; those independent references are retained for the next selected-decoder qualification step.

Run `python oracle.py` only in a private environment containing the pinned packages. OpenEXR's Python writer mutates the supplied channel dictionary into Channel objects, so the oracle preserves independent array copies before writing and compares reopened samples to those copies. The first harness attempt exposed that Python API behavior; no product decoder was involved.

The private initial profile is version-2, single-part, non-deep, scanline RGB or RGBA. Channels are exactly top-level R/G/B with optional A, ordered as required by the format, HALF or FLOAT (mixed types also representable), and full-rate 1x1 sampling. UINT, extra channels/groups, tiled/multipart/deep data, long names, and other compression families are refused. NONE, RLE, ZIPS and ZIP compression declarations and their bounded chunk geometry are represented; preflight does not prove a compressed payload decodes. Both zero and signed nonzero origins work when data and display windows match. Origin is retained verbatim; a later conversion must explicitly report coordinate normalization rather than silently dropping source placement. Differing windows require an explicit canvas policy and remain refused.

Dimensions use signed 64-bit differences before admission: sides at most 65,536, at most 4,194,304 pixels, at most 64 MiB source and 64 KiB header, at most 128 attributes, and short names. Offset tables and chunk headers must agree with admitted geometry, signed source row origin, channel byte widths and increasing line order. Every chunk is wholly in bounds; intervals cannot overlap, leave gaps, point into the header/table or leave trailing unqualified content. This is bounded container admission, not a decompressor, process heap limit or physical RSS guarantee. Source storage remains caller-admitted. Callback cancellation during header/table/chunk/hash work publishes no inspection.

Exact canonical float32 chromaticities recognize only the precision policy's known nominal spaces. Nearby values are retained, never snapped to a gamut. The explicit `colorInteropID=lin_rec709_scene` is recognized only when any chromaticities agree. Missing, unfamiliar or conflicting color metadata remains unresolved and requires the precision policy's explicit source-space assignment; unlabelled EXR never silently becomes Rec.709. All original bounded attributes remain borrowed from the sealed SHA-bound source, including unknown provenance, for later receipt/loss decisions. Duplicate attributes and channel names are refused.

Alpha follows the OpenEXR conventional premultiplied meaning; source samples, including additive RGB at alpha zero, are not changed by inspection. The existing precision policy separately requires explicit flattening or refusal for additive alpha-zero color, supports deliberate alpha-convention override, and refuses nonfinite or out-of-range alpha. This preflight does not claim sample validity or silently unassociate, clip, tone-map or quantize samples.

Primary sources:

- [OpenEXR file layout](https://openexr.com/en/latest/OpenEXRFileLayout.html), whose definition explicitly defers to the official C++ library.
- [OpenEXR technical introduction](https://openexr.com/en/latest/TechnicalIntroduction.html), including distinct windows, channel sampling and premultiplied alpha.
- [ASWF OpenEXR color interchange recommendation](https://github.com/AcademySoftwareFoundation/ColorInterop/blob/main/Recommendations/04_OpenEXRFiles/OpenEXRFiles.md), including explicit source-color identification.
- [Selected exr 1.74.2 API](https://docs.rs/exr/1.74.2/exr/): BSD-3-Clause, default Rayon must be disabled if adopted. Its `MetaData::read_from_buffered` explicitly does not fully validate metadata; hostile source must pass bounded preflight before codec parsing. No workspace EXR dependency is added at this stage.

Oracle wheel hashes:

- `openexr-3.5.2-cp314-cp314-macosx_11_0_arm64.whl`: `c14e8b1a48cf8c2a57e087930c59380d3c593635f57fd5fe5b651df8da52dfda`.
- `numpy-2.4.3-cp314-cp314-macosx_14_0_arm64.whl`: `a111698b4a3f8dcbe54c64a7708f049355abd603e619013c346553c1fd4ca90b`.

Exact wheels and provenance are preserved in ignored evidence at `docs/validation/exr-policy-2026-10-26/preflight-44731bf5-c011-4349-898d-338ec5890e49/`. No EXR decoder, helper profile, frontend import, public reader or export is enabled here. Decoder allocation/confinement, pixel comparison against these references, source-bound conversion preview/loss consent and real consumer workflows remain required.

Validation: `cargo test -p paintcraft-formats --lib exr:: --offline --locked` passes four tests over all eight official fixtures, source/reference hashes, retained signed origins/color/alpha conventions, every prefix truncation of the minimal source, hostile offsets/chunks/header extents/count/names, signed-window overflow and callback cancellation. `cargo clippy -p paintcraft-formats --lib --tests --offline --locked -- -D warnings`, scoped Rust formatting and diff checks pass. Exact final logs are retained in the evidence directory. No pixel decoding or isolation qualification is implied by these container results.
