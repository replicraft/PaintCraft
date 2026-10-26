# Owned HEIF color qualification fixtures

These are authored 32×24 color patterns and a separate 33×25 4:4:4 boundary set,
not photographs or downloaded codec conformance material. They may be used under
the repository license. `generate.py` specifies every original sample, constructs
small direct-primary HEIF wrappers, and refuses existing nonempty output folders.
No production HEIF encoder is introduced.

The selected reader is `heic-decoder` 0.1.1 at the revision already pinned in the
HEIF module notes. Independent encoder and decoder evidence is separate:

- FFmpeg 8.0.1 with libx265 authors HEVC with explicit encoder VUI parameters.
  FFmpeg's independent HEVC decoder reproduces every authored Y/Cb/Cr or GBR and
  alpha sample exactly. Metadata declares primaries 1 and transfer 13 explicitly;
  no unspecified source is relabeled sRGB.
- Pinned Pillow-HEIF 1.1.1, Pillow 12.0.0, libheif 1.20.2 and libde265 1.0.16
  decode every authored wrapper, including auxiliary straight alpha. The C oracle
  separately reads the 4:2:0 native Y/Cb/Cr planes, which match authored samples.
  `oracle.json` and `odd/oracle.json` record wheel and native library SHA-256s,
  versions, source/pixel hashes and exact commands. No oracle library ships in
  PaintCraft as a result of this qualification.
- `rational_oracle.py` independently computes RGB using exact `Fraction`
  arithmetic, H.273 matrix 1 and the chosen type-0 chroma reconstruction. The
  selected RGB conversion matches this reference and explicitly configured
  FFmpeg bilinear reconstruction within 1 code value over every pixel. Identity
  GBR is exact. Alpha is exact, including 0, 1, 127, 128, 254 and 255. RGB checks
  include pixels whose alpha is zero.

## Explicit reconstruction policy and negative evidence

Admitted color requires matching container NCLX and coded VUI declarations:
BT.709/sRGB primaries 1, IEC 61966-2-1 transfer 13, matrix 0 full-range 4:4:4 GBR
or matrix 1 full/limited-range 4:4:4 or 4:2:0. For 4:2:0, the initially qualified
siting is type 0: chroma origin `(0, 0.5)` in luma coordinates, bilinear
interpolation, outermost-sample extension and unsigned8 rounding/clipping.
Unspecified color, ICC, other siting, shifted conformance crops, deep precision,
premultiplication, display transforms and derived color images are refused by
this initial internal admission profile. Odd 4:4:4 geometry and bottom/right
coded padding are tested. Odd 4:2:0 and color grids remain unqualified.

libheif's RGB output is **not** interchangeable with that policy for these 4:2:0
patterns. Nearest/default RGB differs by up to 69 full-range and 79 limited-range;
forcing libheif's bilinear option differs by up to 98 and 113 respectively,
including a bottom-boundary difference. Both original independent outputs are
retained. Tests assert these negative differences rather than increasing a
rounding tolerance. The independent native planes still match exactly. For
4:4:4, libheif agrees exactly at full range and within 1 value at limited range.
This is qualification of a declared PaintCraft policy, not universal rendering
parity with every HEIF application.

Earlier FFmpeg commands using top-level color options produced a lossless-labelled
stream whose decoded samples differed from the authored input. The successful
commands instead use x265's explicit VUI parameters, with identical authored
pixels; no encoded stream was rewritten to make an oracle pass.
`negative-evidence.json` retains hashes and sample mismatch counts from the prior
generation, whose original files remain in the qualification scratch evidence.
FFmpeg's mov demux also rejected the earlier gray HEIF wrappers with “moov atom
not found”; elementary HEVC readback never established container interoperability.

## Reproduction

Use Python 3.14 in a private environment on macOS arm64 for the pinned oracle
wheel ABI. Install only the exact versions below, without changing global Python:

```sh
python3 -m venv /private/tmp/paintcraft-heif-oracle
/private/tmp/paintcraft-heif-oracle/bin/python -m pip install \
    --only-binary=:all: pillow-heif==1.1.1 Pillow==12.0.0
/private/tmp/paintcraft-heif-oracle/bin/python \
    crates/services/paintcraft-formats/tests/heif-color-fixtures/generate.py \
    /private/tmp/paintcraft-heif-owned-new
/private/tmp/paintcraft-heif-oracle/bin/python \
    crates/services/paintcraft-formats/tests/heif-color-fixtures/generate.py \
    /private/tmp/paintcraft-heif-owned-odd-new odd
cargo test -p paintcraft-formats --features heif-codec --lib heif::admission
```

`libheif_oracle.py` uses the pinned public 1.20.2 C ABI and library-allocated
options, selecting an explicit algorithm without allocating an undersized C
structure. It only processes these tiny owned specimens. It is not production
FFI or an untrusted-data execution boundary.

Primary references:

- [ITU-T H.273](https://www.itu.int/rec/T-REC-H.273): primaries, transfer and matrix code points.
- [ITU-T H.265](https://www.itu.int/rec/T-REC-H.265): chroma sample locations.
- [libheif 1.20.2 decoding options](https://github.com/strukturag/libheif/blob/v1.20.2/libheif/api/libheif/heif_decoding.h).
- [libheif 1.20.2 color options](https://github.com/strukturag/libheif/blob/v1.20.2/libheif/api/libheif/heif_color.h).

This prerequisite does not enable HEIF document import or writing. Selected SPS
parsing remains confined to the existing isolated worker boundary; bounded source
inspection does not by itself bound upstream reference/scratch allocations.

`anonymous-identity444-alpha.heic` is a separate valid wrapper with empty primary
and alpha item names. Run `generate.py /private/tmp/new-empty-output anonymous`
to reproduce it. `anonymous-oracle.json` proves its unchanged HEVC elementary
bytes and identical independent libheif RGB/alpha against the named identity
fixture. It qualifies the no-metadata-loss case without rewriting encoded pixels.
Named wrappers separately report `heif-item-names-not-retained`, since source item
labels in a receipt do not mean they are stored in the converted document.
