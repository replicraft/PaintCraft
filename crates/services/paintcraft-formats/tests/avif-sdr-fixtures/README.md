# Owned AVIF SDR precursor fixtures

These fixtures qualify a private parser/reconstruction precursor. They do not
widen the existing identity v1 reader or enable a public YCbCr import route.
All samples are authored by PaintCraft: luma/chroma gradients and sharp edges,
full 8-bit head/footroom, and alpha values 0, 1, 127, 128, 254 and 255.

`libaom_encode.c` directly sets reduced-still encoding, lossless mode, one thread,
profile/depth, exact primaries/transfer/matrix/range and chroma-position controls.
There is no RGB-to-YUV encoder conversion or inherited Y4M color inference.
`libaom_oracle.c` independently decodes the real streams to native planes. Its
`.planes` outputs use three full-area slots; subsampled data is tightly packed
at the beginning of each chroma slot and the remaining bytes are zero. The
`.yuv` files are authored tightly packed input. Lossless native readback must
match it exactly, including odd 17×13 chroma ceilings.

The 24 qualified candidate containers span full/limited range, 4:4:4 and both
explicit 4:2:0 positions, 16×16 and 17×13 geometry, and opaque/straight-alpha
forms. Eight additional UNKNOWN-position containers are refusal cases; they
have no RGB golden because a sample position is not inferred. `.obu` files are
actual independent encoder output. The minimal AVIF wrappers explicitly declare
those same metadata fields; coded/container disagreement must fail admission.

## Color policy and independent scalar oracle

[AV1 color-config semantics](https://aomediacodec.github.io/av1-spec/#color-config-semantics)
defines CSP_VERTICAL (1) as horizontally co-located, halfway between luma rows:
origin `(0, 0.5)`. CSP_COLOCATED (2) has origin `(0, 0)`. UNKNOWN (0) and reserved
(3) do not acquire a guessed position in this precursor. AVIF's required matching
configuration semantics are in [AVIF 1.2 section 2.2.1](https://aomediacodec.github.io/av1-avif/v1.2.0.html#av1-item-configuration-property).
Any container-default interpretation for UNKNOWN is separate investigation.

Explicit sRGB primaries/transfer are CICP 1/13; matrix 1 is BT.709 YCbCr.
The range/matrix interpretation follows
[H.273](https://www.itu.int/rec/T-REC-H.273-202407-I/en) and
[BT.709](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.709-5-200204-S%21%21PDF-E.pdf):
`Kr=2126/10000`, `Kb=722/10000`, `Kg=1-Kr-Kb`. Full range uses Y directly and
chroma centered on 128. Limited range expands Y with `(Y-16)*255/219` and
chroma with `(C-128)*255/224`. Bilinear interpolation uses the explicit origins
and extends outermost samples. Interpolation is a declared PaintCraft policy,
not an AV1-mandated reconstruction filter or universal viewer-parity claim.

`generate.py` independently evaluates reconstruction with Python `Fraction`,
computes green from the luminance identity, and rounds/clamps exactly to RGB8.
It never uses the selected decoder/converter to generate `.rational.rgba`.
Alpha remains exact and coded RGB at alpha zero is included. These scalar
outputs are distinct from native-plane decoder agreement.

## Reproduction

Use pinned libaom **3.15.2**, commit
`af3dc9aadc793c1b00edc003e3fc19a44fdeb574`; archive SHA-256
`01d2e7a4a04a4c57d33eba487ff00f5cea29932d95a848de760d800adb8054a2`.
The source archive is
`https://aomedia.googlesource.com/aom/+archive/af3dc9aadc793c1b00edc003e3fc19a44fdeb574.tar.gz`.
This test-only build is not a product dependency or system installation.
With `$AOM_SOURCE`, `$AOM_BUILD`, `$ENCODER` and `$ORACLE` pointing to private
build paths, run from this fixture directory:

```sh
cmake -S "$AOM_SOURCE" -B "$AOM_BUILD" -DAOM_TARGET_CPU=generic \
    -DENABLE_DOCS=OFF -DENABLE_EXAMPLES=OFF -DENABLE_TESTS=OFF -DENABLE_TOOLS=OFF \
    -DCONFIG_AV1_ENCODER=1 -DCONFIG_AV1_DECODER=1 \
    -DBUILD_SHARED_LIBS=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build "$AOM_BUILD" --parallel 4
cc -std=c11 -O2 -I"$AOM_SOURCE" -I"$AOM_BUILD" libaom_encode.c \
    "$AOM_BUILD/libaom.a" -lm -lpthread -o "$ENCODER"
cc -std=c11 -O2 -I"$AOM_SOURCE" -I"$AOM_BUILD" libaom_oracle.c \
    "$AOM_BUILD/libaom.a" -lm -lpthread -o "$ORACLE"
python3 generate.py --encoder "$ENCODER" --oracle "$ORACLE" --output .
```

`oracle.json` records tool hashes, settings, source revision, independently decoded
metadata and fixture hashes. Its commands use `$ENCODER` and relative paths;
actual local invocations and initial failures belong in ignored validation logs.
The initial CLI/Y4M attempt was rejected for a vertical input label mismatch;
no stream was patched or metadata relaxed to hide that failure. The explicit C
encoder is the successful producer used here.

## Selected decoder and reconstruction evidence

The private selected-decoder qualifier passes all 24 admitted candidate forms.
Actual charged dav1d 1.5.4 native planes match the independently decoded libaom
planes byte for byte. Reusing `heic-decoder` 0.1.1's existing matrix/range/bilinear
converter with the explicit AV1 origins matches every exact-Fraction RGBA value
in these fixtures, with **zero** differing RGB channels and exact alpha. The
alpha cases include 42 nonzero hidden-RGB pixels at alpha zero for even geometry
and 37 for odd geometry. This is controlled-fixture evidence for that declared
float32 policy, not a claim that all possible rounded values are universally
identical across applications.

`selected-proof.json` records the exact commands, source hashes and per-case
results. Four focused tests exercise selected native-plane/RGBA equality,
plane-length and alpha disagreement, cancellation during staging and after
conversion, explicit large-buffer accounting, and forged lengths/profiles.
The existing code5 subprocess suite passes its identity/cancellation cases and
refuses all 24 new YCbCr candidates with a canonical unsupported response and
normal exit. No new public reader or helper protocol is enabled by these checks.

The work declaration counts source bytes, three native byte-plane slots for
each decoded item, **four bytes per staged i32 sample**, RGBA, converter axes,
read flags and conservative row buffers. The existing combined Rust/C child
heap cap remains charged once; host source/output buffers are separate. These
large-buffer capacities exclude allocator overhead, stacks and RSS, and do not
qualify calls outside the isolated helper. An eventual new helper path must use
the declaration before native decode and retain the existing owner/source guards.
