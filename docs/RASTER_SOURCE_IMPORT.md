# Stock raster conversion safety

Ordinary raster Open and explicit image imports create converted PaintCraft
documents. They do not bind preserving Save to the original image.

## Source authority

Desktop acquisition retains the regular file identity from the same open handle
that supplied the decoded bytes. The raw source SHA-256 and byte count are
checked after decoding, before construction after loss consent, and immediately
before adoption. A changed or replaced source cannot replace the current work.
The acquired identity remains an overwrite guard after adoption, PCND Save and
tab switching, including renamed originals and hardlinked aliases.

CLI and MCP also revalidate captured regular sources before publication.
CLI checks again at the final staged-save checkpoint. Browser File objects and
legacy stream inputs retain their immutable snapshot policy. These checks do
not provide filesystem compare-and-swap or serialize other processes after the
last checkpoint. Source authority is separate from converted document identity;
the change does not add a persistent PCND authorization record.

## PNG and BMP declarations

Bounded source inspectors describe original geometry, packed storage, palettes,
transparency, color declarations, resolution and omitted metadata before RGBA8
conversion. ICC inspection validates container bounds and records a payload
digest. It does not interpret a profile or perform color management.

PNG inspection records gAMA, cHRM, sRGB, ICC and cICP declarations with their
priority, HDR declarations, physical dimensions or aspect units, significant
bits, EXIF and ancillary payload digests. BMP inspection records row direction,
stride, masks, palette reserved bytes, V4 calibration and V5 embedded profiles.
Linked BMP profiles are not opened. Resolution does not rescale pixel geometry.

Imports explicitly report discarded declarations, resolution and metadata.
Palette expansion, ICC omission and EXIF omission retain their existing loss
codes; EXIF is reported once. All new omission messages have seven-language UI
translations. These descriptors do not retain the original metadata or index
plane in PCND. Full source preservation and color-managed editing remain open.

Indexed PNG decoding reads the original packed 1/2/4/8-bit index plane through
the selected `png` backend, validates every index against PLTE, then expands
straight RGBA while retaining hidden RGB and tRNS alpha. This avoids the pinned
backend's EXPAND behavior, which substitutes opaque black for undeclared
palette entries. Adam7 is deinterlaced before checked expansion. Existing 16-bit
conversion continues to retain each sample's high byte.

## Local evidence and remaining scope

Nineteen independently serialized PNG/BMP specimens include exact packed rows,
an asymmetric seven-pass Adam7 image, duplicate palette colors, hidden RGB,
16-bit samples, EXIF, ICC, resolution and calibration. Structural mutations and
cancellation controls accompany the fixtures. Records are under
`docs/validation/png-bmp-metadata/` and `docs/validation/stock-source/`.

The integrated macOS ARM checks passed 418 format tests, 649 desktop tests,
46 CLI unit tests and two compiled CLI/MCP source lifecycle tests. Existing
ignored installed-helper tests remain separate qualification work. The browser
WebAssembly build passes; this is not a fresh runtime browser or six-target
qualification claim. JPEG orientation/CMYK, GIF logical-screen placement,
generic raster visual review, wider source preservation and the full format
roadmap remain in progress.
