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

## JPEG and TIFF normalization

JPEG conversion uses the selected Zune backend with explicit device-color
interpretation and applies EXIF orientations 1 through 8 exactly once. Original
dimensions, components, density and orientation remain inspection facts. CMYK
conversion is a declared device convention, not ICC color management; an unmarked
four-component source additionally reports the assumed convention. Original
metadata is inspected separately from the bounded decoder projection.

Supported ordinary single-image TIFF and explicit selected pages normalize all
eight orientations. Raw dimensions and orientation remain source facts; output
geometry swaps axes for orientations 5 through 8. Selected-page assessment and
worker framing version 2 bind both geometries, source orientation and actual
output samples. Old framing is refused. Selected GrayA16 support does not imply
support through ordinary single-image decoding. See [TIFF page import](TIFF_PAGE_IMPORT.md).

Normalized geometry, exact pixels and versioned policy contribute to document
identity across desktop, web, CLI and MCP. Raw source hashes continue to guard
original files. See [converted document identities](CONVERSION_IDENTITY.md).

## BMP RLE and icon palette bounds

Strict RLE4 and RLE8 imports validate runs, absolute samples, padding, deltas,
row bounds and palette indices. Unwritten pixels use opaque palette entry zero.
A zero declared image size uses the bounded end-marker profile and reports
omitted metadata. Top-down compressed sources remain refused. Descriptors
distinguish compressed stream extents from uncompressed row strides. Caller
pixel admission precedes index scanning.

Bitmap ICO payloads validate actual XOR palette indices, including masked
pixels, before invoking the selected decoder. This prevents undeclared indices
from reaching its unchecked palette lookup. Padding bits are not pixels. These
changes do not add BMP16 or arbitrary icon variants.

## Visible loss review

Desktop and web flat-raster conversions with reported losses now display the
actual pending pixels before consent. A bounded nearest thumbnail fits within
200 logical points, enlarging tiny images and retaining a visible thin axis.
The complete preview must have appeared in a previous frame before its explicit
limitation checkbox can authorize Open. The resolver enforces this requirement
as well as the button. Hidden or clipped initial previews cannot authorize an
import. Loss-free imports retain their automatic route.

Cancellation, stale ownership and changed sources retain the existing document
and release the pending review. Source identity and final adoption checks remain
separate from visual consent. Thumbnail accounting declares logical scratch
and egui-image bytes; it is not a hard allocator, GPU or process-memory bound.
Layered and specialized review workflows retain their own contracts.

## Defined still GIF composition

Still GIFs can place a smaller image inside their logical screen. Opaque images
use the declared global background outside the image rectangle. Standalone
transparent images use clear uncovered pixels and retain source palette RGB
under transparent in-frame samples. Different external decoders disagree about
some background cases; the fixture records preserve those disagreements and
the explicit chosen policy.

The selected indexed GIF decoder supplies original samples; every referenced
palette index is validated before RGBA publication. Interlaced rows follow all
four passes. The expected sample count and an actual LZW end marker are both
required. Disposal does not erase the still before capture. Animation, reserved
disposal modes and an opaque partial image without a defined global background
remain refused. This adds defined still conversions, not timeline editing.

## Local evidence and remaining scope

Nineteen independently serialized PNG/BMP specimens include exact packed rows,
an asymmetric seven-pass Adam7 image, duplicate palette colors, hidden RGB,
16-bit samples, EXIF, ICC, resolution and calibration. Structural mutations and
cancellation controls accompany the fixtures. Records are under
`docs/validation/png-bmp-metadata/` and `docs/validation/stock-source/`.

The initial integrated macOS ARM checks passed 418 format tests, 649 desktop
tests, 46 CLI unit tests and two compiled CLI/MCP source lifecycle tests.
The subsequent visual-review snapshot passed 652 desktop and 309 host Web tests,
strict checks and WebAssembly compilation. Two actual offscreen egui/wgpu
screenshots were inspected; no OS window was opened. GIF qualification passed
seven focused controls and a 633-test non-JPEG snapshot, explicitly excluding
seven then-pending JPEG controls. See the individual frozen receipts for their
exact source and scope.

Subsequent JPEG qualification covered 54 independent backend cases and 13
profiles through all four consumers. BMP RLE covered 26 independent fixtures and
six profiles through all four consumers. TIFF production-browser qualification
covered 32 selected orientation pages, eight reviewed GrayA16 cases and actual
saved PCND validation. Exact receipts and retained failed attempts are under
`docs/validation/jpeg-source/`, `jpeg-consumers/`, `bmp-rle/`,
`bmp-rle-consumers/`, `tiff-orientation/` and `tiff-orientation-consumers/`.

These finite checks do not establish six-target stock-codec, original-application
or physical-device behavior. Wider source preservation, color management and the
full format roadmap remain in progress. PNG EXIF is historical source metadata;
it is not automatically treated as an instruction to rotate edited PNG pixels.
