# Radiance preflight fixtures

These owned, Python-generated RGBE files qualify bounded header and packet inspection only. They do not enable an HDR reader, establish decoder confinement, or qualify all Radiance orientations. Run `python3 reference.py` here to reproduce the source files, hashes, and two floating-point reference conventions in `manifest.json`.

`literal-2x2.hdr` and `calibrated-2x2.hdr` contain identical stored packets. Ordered EXPOSURE records multiply to 6; COLORCORR records multiply to [1, 2, 2]. Both floating-point reference files remain identical between those inputs. These records describe adjustments already present in stored samples: inspection neither reapplies nor undoes them. Original Radiance `pvalue` defaults to exposed values (`+o`); recovering original radiance is a separate `-o` operation. This initial profile preserves calibration provenance without implementing recovery.

The independent Python scalar packet reconstruction writes both lower-edge (`mantissa * scale`) and original Radiance midpoint (`(mantissa + 0.5) * scale`) references. Exponent zero is black under both conventions. This records a real convention difference, rather than presenting a correct lower-edge implementation as universally identical to the original Radiance reader. The current parser reconstructs neither convention.

Valid inputs cover flat literals, legacy repeats including a zero low run digit followed by a high digit, modern component RLE, calibration provenance, exact recognized Rec.709 chromaticities, nearby unknown chromaticities, and rounded equal-energy white. Nearby and rounded declarations remain unresolved, retaining raw values for an explicit source assignment; they are never snapped to a nominal color space.

Negative fixtures cover inconsistent modern row width, zero-length modern packets, row overrun, truncated literals and excessive legacy digit chains. A legacy marker collision and a nonstandard orientation are explicitly unqualified inputs, not claims that every broader Radiance reader must reject those files. Initial inspection admits only `-Y +X`; orientation normalization remains a later decoder task. Source bytes, header bytes/lines, dimensions, packet count and callback work are bounded before any image decoder or pixel allocation.

Unknown bounded header records are retained as provenance, not executed. Indented lines and comments are inert; recognized active keys match exactly. Repeated conflicting PRIMARIES are retained and require source assignment. Missing primaries use Radiance's documented native equal-energy defaults, whose green primary differs from Rec.709. No arbitrary coordinate tolerance is used.

Primary references:

- [Radiance picture format](https://radsite.lbl.gov/radiance/refer/Notes/picture_format.html)
- [Radiance native primaries and white point](https://raw.githubusercontent.com/LBNL-ETA/Radiance/master/src/common/color.h)
- [Radiance RGBE reconstruction](https://raw.githubusercontent.com/LBNL-ETA/Radiance/master/src/common/color.c)
- [pvalue exposed/original semantics](https://radsite.lbl.gov/radiance/man_html/pvalue.1.html)
- [ra_xyze explicit original-exposure recovery](https://raw.githubusercontent.com/LBNL-ETA/Radiance/master/src/px/ra_xyze.c)

The selected Zune HDR 0.5.2 candidate failed an independently retained exponent-wrap probe; it is not adopted. Existing image 0.25.10 HDR reconstructs the controlled lower-edge exponent vectors correctly, but its header and packet behavior still needs this preflight, bounded decoder execution and further qualification. Its HDR feature remains disabled by default; the optional private qualification adapter below now exists. Raw candidate failures, upstream snapshot and alternative outputs are retained separately under the ignored `docs/validation/hdr-policy-2026-10-26/` evidence directories.

## Private candidate adapter

The optional `hdr-codec` feature enables only existing `image/hdr` for private qualification. `radiance::decode::decode` accepts a sealed `Inspection`, whose private borrowed packet slice is the same immutable source used for preflight and SHA binding. It accepts no independent replacement body. Only an internally generated minimal header and the validated borrowed packets reach image; original metadata stays on the inspection receipt. This avoids the backend reinterpreting calibration, indented provenance or numeric header precision. It does not rewrite the input file.

`Decoded` exposes as-stored RGB32F using the lower-edge convention, without alpha or implicit gamut/tone/exposure conversion. The corpus adds all ten controlled exponent cases [0, 1, 96, 97, 128, 129, 159, 160, 200, 255], including subnormals and both sides of the Zune exponent-wrap failures. Actual samples are compared bit-for-bit to independent Python lower-edge references for every admitted fixture; midpoint references remain separately retained.

Logical admission covers both RGB32F buffers while safely converting native-endian output bytes, the selected backend's one RGBE scanline, and a bounded canonical-header allowance. The retained result charges its float allocation until drop. This does not cap allocator overhead or physical RSS. Checks before backend reads and during output conversion propagate cancellation and drop private buffers; they do not interrupt a backend CPU loop, provide a hard deadline or replace killable isolated execution. No worker profile, frontend, format registry read gate or default feature is enabled by this adapter.

Pinned candidate source: [image v0.25.10 HDR decoder](https://raw.githubusercontent.com/image-rs/image/v0.25.10/src/codecs/hdr/decoder.rs), source SHA256 `9e38b35e96b2f3c134b3a255fcfc13d3177b525fbd6a684205d9e538d2838d1a`; published crate SHA256 `85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104`. The exact decoder snapshot and provenance record are retained in ignored qualification evidence at `docs/validation/hdr-policy-2026-10-26/adapter-609cf141-f0d8-4020-af61-bd0810987c4d/`. The parser must precede this candidate: upstream accepts mismatched modern row-width markers and zero-length packets, and unbounded legacy repeat digit chains can overflow its multiplier. Those malformed cases are rejected by our preflight before invocation.

Validation: `cargo test -p paintcraft-formats --features hdr-codec --lib radiance:: --offline --locked` passed all six tests, including nine admitted fixture files and all ten exponent extremes. `cargo clippy -p paintcraft-formats --features hdr-codec --lib --tests --offline --locked -- -D warnings` passed. Scoped Rust formatting and diff checks passed. This is host adapter qualification, not isolated worker, browser, frontend or original-application qualification. The exact final test output is retained beside the source snapshots.

The preserved official Radiance `color.c` snapshot identifies revision 2.42 (2025-05-06); SHA256 `468200b3613da0cd4cde6a905cb1183515520c6746ee5a47e4a4a8c71d9bb1a6`. Its midpoint reconstruction remains a separate reference convention; it is not the adapter's lower-edge output contract.
