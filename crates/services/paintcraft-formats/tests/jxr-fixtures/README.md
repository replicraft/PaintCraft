# JPEG XR inspection and developer qualification fixtures

These two RGB8 gradients were authored for PaintCraft. Original C JXRLib, compiled
from the pinned `jpegxr` 0.3.1 source package, encoded and independently decoded
them. Every decoded byte matched `(x * 47 + y * 13 + channel * 71) % 256`.
Dimensions are 4 × 3 and 17 × 19, including non-macroblock-aligned edges.
`oracle.json` records exact source/archive, fixture, raw-channel and harness hashes.
The generated fixture data and `oracle.c` use PaintCraft's repository license;
no downloaded photographs or third-party test images are included.

Reference source: [original C JXRLib in jpegxr 0.3.1](https://github.com/bvibber/jpegxr/tree/3e8b298304cd8a42e773afb0af5cf57bd49549c4/jxrlib).
The supplied Microsoft notice is reproduced in `JXRLib-LICENSE.md`.
The reference executable is a development oracle only and is not shipped with
PaintCraft. Its authored `oracle.c` harness uses the public JXRLib API.

To recreate it, extract the source package and compile the harness with:

```sh
clang -O2 -w -D__ANSI__ -DDISABLE_PERF_MEASUREMENT \
    -Ijxrlib/common/include -Ijxrlib/image/sys -Ijxrlib/jxrgluelib \
    oracle.c jxrlib/image/sys/*.c jxrlib/image/decode/*.c \
    jxrlib/image/encode/*.c jxrlib/jxrgluelib/*.c -lm -o jxr-reference
./jxr-reference encode 4 3 rgb8-4x3.jxr rgb8-4x3.rgb
./jxr-reference decode 4 3 rgb8-4x3.jxr rgb8-4x3.decoded.rgb
```

Repeat the final two commands with 17 and 19 for the other fixture. Binary
reproducibility of the executable is not promised across compilers; recorded
fixture/channel hashes identify the actual evidence.

## Deliberate qualification boundary

The runtime module only inspects bounded headers, directory fields and codestream
extent/mode/dimension consistency. It does not decode pixels or validate entropy
payloads. Its narrow inspected subset is RGB8 with no alpha, ICC/EXIF, orientation,
extra directories, tiled/frequency layouts, windowing or unknown metadata. Other
valid JPEG XR profiles are refused, rather than silently converted.

The selected [jpegxr-pure-rs 0.1.0](https://github.com/henriksson-lab/jpegxr-pure-rs/tree/cd626cd555e944c6cdcf273fac52434ca1fedd61)
is a native **development-only** dependency. An explicitly ignored test compares
its decoding against the independent authored samples:

```sh
cargo test -p paintcraft-formats jxr::qualification::selected_decoder_matches_independent_jxrlib_rgb8_fixtures -- --ignored
```

That experiment checks trusted fixtures and creates a private, random host scratch
directory, with Unix mode 0700 and source files 0600. It uses `Decoder::open` and a
fallibly allocated, size-checked output buffer. It does not pass malformed entropy
payloads to the experimental decoder. Normal tests exercise malformed headers only.

The upstream `Decoder::from_slice`/`from_reader` and writer-to-memory helpers create
predictable files in a shared temporary directory. We do not invoke them. The
upstream decoder exposes dimensions only after codec initialization, and has no
host allocator quota or cancellation callback. `PixelFormat` also has no public
constructor for fresh RGB/RGBA frames, so no fresh-writer adapter is supplied.
See the pinned [API source](https://github.com/henriksson-lab/jpegxr-pure-rs/blob/cd626cd555e944c6cdcf273fac52434ca1fedd61/src/api.rs).

Cargo metadata declares BSD-3-Clause; its actual distributed `LICENSE.md` has the
Microsoft notice reproduced here and two redistribution conditions. This records
the observed discrepancy without changing either upstream declaration or notice.

These results do not qualify production Open/Export, arbitrary JPEG XR files,
hostile-input confinement, alpha/deep-color/HDR, browser execution, fresh writing,
performance or preserving saves. No frontend or runtime format registry route is
enabled by this experiment.
