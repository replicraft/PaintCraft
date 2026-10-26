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

The selected Zune HDR 0.5.2 candidate failed an independently retained exponent-wrap probe; it is not adopted. Existing image 0.25.10 HDR reconstructs the controlled lower-edge exponent vectors correctly, but its header and packet behavior still needs this preflight, bounded decoder execution and further qualification. Its HDR feature remains disabled. Raw candidate failures, upstream snapshot and alternative outputs are retained separately under the ignored `docs/validation/hdr-policy-2026-10-26/` evidence directories.
