# Owned AVIF still-image fixtures

These small generated pictures contain no third-party artwork. The repository's
existing `tests/avif.rs::ffmpeg_libdav1d_reopens_color_and_auxiliary_alpha` produced
them on 2026-10-26 through `image` 0.25.10, ravif 0.13.0 and rav1e 0.8.1: speed
10, quality 100, one encoder thread, explicit sRGB/identity color.

Both pictures are 16×16. Source RGB is `[80,90,100]`; opaque alpha is 255, while
the alpha fixture uses the row-major pixel index 0..255. `.source.rgba` is the
authored buffer; `.decoded.rgba` is the independent container/plane readback.
Lossy AVIF encoding is not treated as pixel preservation: the existing oracle
permits RGB error ≤3 except at alpha 0, and alpha error ≤2. Those encode losses
are separate from comparing two decoders of the exact same bitstream.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| opaque.avif | 307 | d17eb0071cbd75e9c6bf00a87b50fb9ffa68d19d616d630ce58feefbdb380824 |
| alpha.avif | 509 | db7a44c37e8336bab7ed4cadee430e1b579cbcf1f1b5fa5d088cf11cba27a61d |

FFmpeg/FFprobe 8.0.1 with libdav1d independently parsed the containers and
decoded both pictures; alpha was explicitly joined from the auxiliary AV1
stream using `[0:0][0:1]alphamerge`. `ffprobe.json` retains the exact stream
reports: primary High/profile 1, 8-bit GBR 4:4:4, full range, BT.709 primaries,
sRGB transfer; auxiliary Main/profile 0, 8-bit full-range monochrome. FFprobe is
a different container/plane integration, **not a different decoder engine from
the selected native dav1d backend**. Independent libaom comparison remains a
separate qualification step.

The raw `av1C` and reduced sequence headers contain level index **31**. FFprobe's
`level: 23` field is retained as an observed diagnostic discrepancy; it is not
used as a canonical coded-level oracle. The native helper must compare the raw
level index 31 to the Rust admission, without relaxing that comparison.

The bounded Rust admission tests resolve actual primary/alpha item identities,
property associations, coded extents and AV1 sequence declarations against these
reports. No public AVIF read gate is established by these fixtures. Raw local
oracle outputs are retained under
`docs/validation/avif-admission-2026-10-26/oracle-first/`.

Primary contracts used for admission are the [AVIF 1.2 specification](https://aomediacodec.github.io/av1-avif/v1.2.0.html),
[AOM AV1 sequence/color/frame syntax](https://github.com/AOMediaCodec/av1-spec/blob/master/06.bitstream.syntax.md)
and [AV1 profile semantics](https://github.com/AOMediaCodec/av1-spec/blob/master/07.bitstream.semantics.md).
Professional/profile 2 permits 8/10-bit 4:2:2: parsing that declaration does not
qualify a color conversion. The initial admitted profile is narrower: direct
reduced still-picture identity sRGB8/profile 1 plus optional straight full-range
monochrome/profile 0 alpha.
