# JPEG source and orientation fixtures

The JPEG samples are authored arithmetic tiles and channel arrays in `reference.c`.
No external artwork is included. The qualification producer and raw reader use
libjpeg-turbo 3.2.0, independently of PaintCraft's pinned Zune decoder. This work
adds no libjpeg runtime dependency.

`oracle.json` records the producer source/binary/library hashes, file hashes,
raw component hashes and exact expected RGBA hashes. `.raw` files preserve the
independent reader's grayscale, RGB or CMYK samples. For YCCK input the reference
reader explicitly requests decoded CMYK; it does not mistake K for alpha.

The 54 cases cover:

- All eight EXIF orientations, both TIFF byte orders and non-square source data.
- Baseline and progressive JPEG with independently authored exact sample tiles.
- Gray, RGB and YCbCr source models.
- Adobe CMYK, unmarked conventional CMYK and Adobe YCCK, with colored CMY/K
  endpoints that distinguish channels and polarity.
- Adobe transform zero with numbered RGB component IDs, exercising the selected
  decoder's provisional header model and final RGB correction.

Every expected pixel comparison is exact. Orientation uses a Python destination
row oracle and an additional authored `ABCDEF` grid, independent of the Rust
coordinate mapping. Its row/column interpretation follows the
[CIPA orientation table](https://cipa.jp/std/documents/e/DC-X010-2017.pdf).
The initial orientation-seven oracle error was caught by that label control
before qualification; the diagnostic and initial corpus remain in ignored
validation evidence.

The CMYK policy is an explicit **device-color approximation**, with nearest
integer channel multiplication by K divided by 255. Adobe samples use the
inverted convention. Unmarked conventional samples are complemented first, with
an explicit model-assumption loss. This is not ICC color management or a claim
that the original samples were encoded as sRGB.

The JPEG adapter retains the existing 1 GiB encoded-input and shared/caller pixel
ceilings. There is no arbitrary marker-count or default 8 MiB metadata limit.
Callers may provide lower explicit input, pixel and metadata budgets. Metadata
inventory streams source payloads and uses a fixed 255-slot ICC sequence index.
Its decoder-only encoded copy changes ancillary APP1/APP2/APP13 marker IDs to
COM; lengths, payload bytes, offsets, JFIF and Adobe color markers remain exact.
The original source is never changed. The pinned COM decoder path skips those
payloads rather than retaining omitted EXIF/ICC/XMP/IPTC resources.

Cancellation checks cover source inspection, copy/hash chunks, allocation
initialization, pixel rows and orientation. Checks before and after the opaque
library decode do not establish hard interruption latency, physical RSS bounds
or a sandbox.

Regenerate under the shared target tree:

```sh
clang -std=c11 -Wall -Wextra -Werror -O2 reference.c \
    $(pkg-config --cflags --libs libjpeg) \
    -o ../../../../../target/qualification/jpeg/reference
python3 generate.py \
    --reference ../../../../../target/qualification/jpeg/reference \
    --library /path/to/libjpeg
```

Broader JPEG profiles, metadata preservation, ICC conversion, unusual files and
original-application interoperability remain separate requirements. A passing
fixture corpus does not close the full format roadmap or qualify every JPEG.
