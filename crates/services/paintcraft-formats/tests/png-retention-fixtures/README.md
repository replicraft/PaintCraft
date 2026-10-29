# PNG original-source retention corpus

The 27 PNGs and their `.rgba` expectations are authored by `generate.py`, using
Python's standard library for framing, filtering, Adam7 layout, sample packing,
zlib compression and CRCs. Expected samples are independently checked with the
recorded FFmpeg executable/version in `oracle.json`. Regeneration overwrites the
corpus and oracle; review any differences before accepting them.

Coverage includes grayscale, grayscale/alpha, RGB, RGBA at 8/16 bits, grayscale
and RGB tRNS, indexed 1/2/4/8-bit transparency, odd dimensions, Adam7 and hidden
RGB at alpha zero. The 16-bit conversion oracle uses the established high-byte
policy. Profiles and pHYs are tested independently and together, alongside
omitted text and EXIF metadata. PNG EXIF remains historical omitted metadata;
these fixtures do not request PNG autorotation.

RGB profiles are exact copies of `assets/profiles/sRGB2014.icc` with that asset's
existing provenance. `gray-structural.icc` is an original structural fixture
written by this generator, with D50 and gray-tag framing. It is **not** evidence
of CMS correctness or suitability for an active color transform. Retention
keeps decompressed ICC bytes, not the original PNG profile keyword/framing.

The generator, sample patterns, gray profile and raw expectations are original
PaintCraft test data, available under the repository's license. FFmpeg is an
independent decoder used for qualification, not a runtime dependency. Failing
initial corpus/qualification runs are retained separately under ignored
`docs/validation/png-source-retention/`; they are not represented as passes.
