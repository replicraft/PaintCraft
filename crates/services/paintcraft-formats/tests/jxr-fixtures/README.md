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

The format inspection module only inspects bounded headers, directory fields and codestream
extent/mode/dimension consistency. It does not decode pixels or validate entropy
payloads. Its narrow inspected subset is RGB8 with no alpha, ICC/EXIF, orientation,
extra directories, tiled/frequency layouts, windowing or unknown metadata. Other
valid JPEG XR profiles are refused, rather than silently converted.

The selected [jpegxr-pure-rs 0.1.0](https://github.com/henriksson-lab/jpegxr-pure-rs/tree/cd626cd555e944c6cdcf273fac52434ca1fedd61)
is linked only into the isolated native helper and explicit native developer tests,
not an inline formats decoder. An explicitly ignored test compares
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

## Private native worker qualification, 2026-10-26

The native helper now has a private `JxrRgb8` request. Public JXR Open, import,
export, browser decoding and preserving Save remain unavailable. The same two
independently JXRLib-authored lossless RGB8 fixtures pass exact pixel comparison
through the actual helper. Output adds opaque alpha without changing any RGB
sample. Source resolution fields retain their exact f32 bits (`00000000` for
these fixtures); they are not invented as 96 DPI or silently rescaled.

Color layout alone does not establish sRGB. The strict profile excludes ICC,
`COLOR_SPACE`, `PTM_COLOR_INFO` and unknown color contexts. The explicit policy
`t832-uint-rgb-absent-context-default-srgb` follows the default interpretation for
unsigned RGB with no other color information in
[ITU-T T.832(2012), Annex C.2, printed page 179](https://www.itu.int/rec/dologin_pub.asp?id=T-REC-T.832-201201-S!!PDF-E&lang=e&type=items).
[Microsoft's WIC color-space documentation](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-codec-native-pixel-formats#color-space)
also distinguishes pixel layouts from color context and describes this absent-
context default. This is a stated inference policy, not a claim that the source
contains an embedded sRGB profile. ICC/HDR/linear, alpha and deeper precision are
refused, not narrowed.

The parent creates a random private seekable source inside the already admitted
resource job, retaining its actual create-new identity. Unix directory/file modes
are 0700/0600. It keeps that backing through supervised child termination/reap and
explicitly closes it afterward. Cleanup checks captured identity and never
recursively deletes unknown children; a partial-write replacement/cancellation
regression proves the foreign replacement and original partial file survive.
Windows ACL equivalence is not claimed. Disk bytes are input-limited, not covered
by the Rust allocator; this is not a filesystem sandbox or disk quota. Parent
process death can leave a private temporary directory even though the child's
stdin lease ends. A same-user concurrent namespace/write/spawn race is not
eliminated by identity observations.

The helper verifies the exact private-file extent and every byte against the
SHA-bound immutable request, then uses only `Decoder::open`. Its audited path is
`open -> create_decoder_from_path -> pk_codec_factory_create_decoder_from_file ->
create_ws_file_owned(Read) -> Initialize`; the called decoder sources do not
create output or shared temporary files. The predictable-temp convenience APIs
remain unused. Header admission precedes decoder initialization; dimensions and
all RGB24 precision/order/alpha declarations are checked again afterward.
Explicit fallible output buffers precede `decode_into`. The existing helper's
256 MiB requested-Rust-heap boundary and parent kill/reap deadline apply; neither
is a physical RSS or process-tree/filesystem containment claim.

Three focused actual-helper tests cover both independent pixel oracles, source
bytes, owner/codec mismatch, exact metadata, ICC refusal, caller pixel limits,
missing/mismatched private backing, admitted-header truncated entropy and
cancellation with cleanup after reap. Existing HEIF/JXL/helper allocator and
parent-death tests also pass. This is qualification on macOS 14.7.2 arm64, not
cross-platform or original Windows application evidence. No lossy/alpha/HDR
writer or arbitrary entropy conformance is claimed.

```sh
cargo test -p paintcraft-codec-worker --test native_jxr --offline --locked
cargo test -p paintcraft-platform --lib source::private --offline --locked
```

The changed native build record is `BUILD_V3` with `JXR_PRIVATE_SOURCE1`, while
the binary request/response envelope remains version 2. Browser transport still
admits only HEIF/JXL codes; code 3 is explicitly native-only. Packaging now carries
the exact Microsoft redistribution notice as `JPEGXR_LICENSE.txt` with the
newly linked native helper. The metadata/license discrepancy above remains
recorded; no package-metadata label is substituted for the distributed notice.
