# TIFF selected-page correctness fixtures

These fixtures were authored from the arrays and arithmetic patterns in
`reference.c`. No external artwork is included. The producer uses libtiff 4.7.2;
LittleCMS 2.19.1 creates the sRGB ICC profile with a fixed creation date.
Neither native library is a PaintCraft runtime dependency introduced by this work.

`oracle.json` records file/source/tool/library hashes, every page's raw samples,
and its expected RGBA8 conversion. The reference reader uses
`TIFFReadScanline`/`TIFFReadTile`; RGBA convenience APIs are excluded because
they can premultiply alpha or transform source samples. Float samples are
recorded as raw IEEE32 words and remain unqualified for editable import.

The corpus covers 13 files and 24 pages:

- Both byte orders; unsigned 1/8/16-bit grayscale/RGB/straight RGBA.
- Gray+straight-alpha 8/16, including WhiteIsZero and BlackIsZero.
- Strips and odd-sized tiled images with partial edge tiles.
- None, LZW, Deflate, PackBits, Fax4 one-bit gray and JPEG8 gray/RGB profiles.
- Real ICC on tiny and larger pictures, exercising both results of the
  generic image facade's optional metadata lookup.
- Unsupported first-page CMYK, associated alpha and F32 followed by valid
  independently selected later pages; explicit BigTIFF refusal.

U16 conversion retains the existing TIFF high-byte policy, `value >> 8`.
The 129 and 255 source samples prove this differs from JPEG XL's nearest257 policy.
Hidden RGB/gray at zero alpha and the original input bytes remain intact.

The selected-page profile has 64 MiB encoded input, 1024 pages, 4096 tags per page,
65536 tags total, 1 MiB tag values per page, 4 MiB aggregate tag values and 4 MP
selected output limits. Inventory validates complete absolute-offset IFD graphs;
the decoder reads a view of the selected IFD without changing the source.

Regenerate with a locally built qualification tool under the shared target tree:

```sh
clang -std=c11 -Wall -Wextra -Werror -O2 reference.c \
    $(pkg-config --cflags --libs libtiff-4 lcms2) \
    -o ../../../../../target/qualification/tiff-pages/reference
python3 generate.py \
    --reference ../../../../../target/qualification/tiff-pages/reference \
    --libtiff-library /path/to/libtiff \
    --libtiff-version 4.7.2 \
    --lcms-library /path/to/liblcms2 \
    --lcms-version 2.19.1 \
    --evidence ../../../../../docs/validation/tiff-pages
```

Remaining TIFF requirements include further compression combinations, BigTIFF,
palette/CMYK and associated-alpha conversions, orientation materialization,
retained color/DPI metadata, high-precision/float adoption and genuine multipage
document/export semantics. This selected-picture conversion does not close
those requirements or imply foreign preserving Save support.
