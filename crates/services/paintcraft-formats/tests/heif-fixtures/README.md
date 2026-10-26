# Owned HEVC still-image qualification fixtures

The two 32 × 24 gray8 images were authored for PaintCraft. `gray32x24.h265` uses
`(x * 7 + y * 11) % 256`; `constant32x24.h265` uses 17 at every sample. They contain
no photographs or downloaded conformance streams and may be used under the
repository license.

FFmpeg 8.0.1 with its independent libx265 encoder produced the lossless elementary
streams. FFmpeg's HEVC decoder then reproduced the authored raw gray bytes
exactly. `oracle.json` records sample construction, settings and all SHA-256s.
Reproduce from the corresponding raw gray8 file with:

```sh
ffmpeg -f rawvideo -pixel_format gray -video_size 32x24 -i source.gray \
    -frames:v 1 -c:v libx265 -color_range pc \
    -x265-params lossless=1:repeat-headers=1:keyint=1:bframes=0:pools=none:frame-threads=1:wpp=0:log-level=error \
    -f hevc image.h265
ffmpeg -i image.h265 -frames:v 1 -f rawvideo -pix_fmt gray decoded.gray
```

Development tests construct small HEIF containers in memory around these streams,
retaining VPS/SPS/PPS in `hvcC` and VCL units in checked item storage. Primary
selection deliberately differs from item inventory order. Grid samples put the
gradient tile on the left and the constant tile on the right; auxiliary alpha
retains the constant plane at its native 8-bit depth. These are decoder fixtures,
not a product HEIF encoder, and no HEIF Save/Export route is enabled.

All coded-item SPS declarations are checked before invoking the selected decoder
on these trusted fixtures. The external oracle verifies the elementary pixels;
it has not independently verified the constructed HEIF graph or broad color,
transform, high-depth, malicious-input or cross-platform behavior. Further
confinement and interoperability work is required before frontend admission.

`primary.heic`, `grid.heic` and `alpha.heic` freeze those owned in-memory wrappers
for actual native worker tests. `worker-containers.json` records their byte hashes.
Regenerate into an **existing empty private directory** with
`PAINTCRAFT_HEIF_FIXTURE_OUTPUT=/private/tmp/your-empty-directory cargo test -p paintcraft-formats --lib heif::qualification::export_owned_worker_fixtures_when_requested`.
The opt-in test refuses to overwrite existing files. No production HEIF encoder
is introduced. Native subprocess tests verify exact Y/alpha samples and the
original full-range CICP (2,2,2), never a guessed sRGB conversion. Their native
worker isolation, bounded I/O and resource tests do not independently qualify the
container wrappers or enable frontend opening.
