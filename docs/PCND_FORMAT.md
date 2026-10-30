# PCND format

PCND describes flat raster documents with the `paintcraft.core/default-v1`
compositing policy. Documents without retained source metadata use the initial
version 1 layout when all layer filter stacks are empty. Version 2 adds one
immutable original-source attachment. Version 3 preserves ordered point-filter
intentions without modifying source raster samples. Tile encodings remain the same. The unreleased ZIP prototype
is not a supported format.

This adopted contract supersedes the ZIP proposal in the original implementation
reference pack. Historical prototype benchmark artifacts retain their original
bytes; they do not define a current compatibility target.

All integers below are canonical unsigned LEB128 unless specified otherwise.
Redundant leading zero groups, overflow and truncated integers are invalid.
A signed origin uses ZigZag: `(value << 1) ^ (value >> 63)`. UUID bytes use UUID
network order, exactly as `Uuid::as_bytes()`. Strings are a byte length followed by
that many UTF-8 bytes; no terminator or normalization is applied.

The stream consists of:

| Field | Representation |
|---|---|
| Magic and version | Eight bytes: `50 43 4e 44 VV 0d 0a 1a`, where VV is 01, 02 or 03 |
| Document ID | 16 bytes |
| State ID | 16 bytes |
| Canvas width, height | Two positive integers, each at most 1,048,576 |
| Title presence | One byte, exactly 0 or 1 |
| Title, if present | UTF-8 string, at most 4,096 characters and 16,384 bytes |
| Source attachment | Present exactly in version 2; in version 3 a strict 0/1 presence byte precedes the optional attachment |
| Root layer count | Integer bounded by the editor's layer-count policy |
| Layers | The following layer records, in root order |
| Integrity digest | 32 bytes of SHA-256 over every preceding byte |

A layer record consists of:

| Field | Representation |
|---|---|
| ID | 16 bytes; unique in the document |
| Name | UTF-8 string, at most 1,024 characters and 16,384 bytes |
| Visibility | One byte, exactly 0 or 1 |
| Opacity | One byte, 0 through 255 |
| Origin X, Y | Two ZigZag integers bounded by the coordinate policy |
| Width, height | Two positive integers, each at most 1,048,576 |
| Default pixel | Four straight RGBA8 bytes, including hidden RGB |
| Filter stack | Version 3 only: bounded ordered intentions, as specified below |
| Tile records | Repeated records, terminated by one zero byte |

A tile starts with a nonzero one-byte encoding tag, followed by tile X, tile Y,
stored byte count and the stored payload. Coordinates must be unique within their
layer and lie within its dimensions. Decoded bytes are derived from the clipped
256 × 256 tile extent, rather than redundantly stored. The stored count must be
positive and at most 262,144. A layer terminator cannot be confused with a tile:
encoding tag zero has no following fields.

| Tag | Payload |
|---|---|
| 1 | Raw RGBA; stored count must equal the derived decoded extent |
| 2 | RGBA runs: `u16le(count - 1)`, then four color bytes |
| 3 | One bounded, dictionary-free Zstandard frame |
| 4 | One LZ4 block over raw RGBA, without an allocation-size prefix |
| 5 | Four-byte little-endian intermediate run-byte count, then an LZ4 block over those RGBA runs |

Unknown tags are rejected. Runs must cover the derived extent exactly. The tag 5
intermediate count must be positive, divisible by six, and at most six times the
number of decoded pixels. LZ4 must produce exactly its admitted output length.
Zstandard uses the existing bounded-frame validation and exact output checks.

There is no optional feature block or arbitrary metadata extension in version 1.
Its fixed semantics imply sRGB, straight RGBA8, normal layer blending, finite
canvas, flat roots and 256-pixel tiles. New semantics require a new version, whose
reader must be explicitly admitted. Empty title and absent title remain distinct.
Unreferenced layers, duplicate IDs, invalid flags and trailing body bytes are
rejected. Total serialized metadata is capped at 8 MiB, and the whole decoded tile
plan must fit the caller's admission budget before any tile is inflated.

## Version 2 source attachment

Version 2 has schema `paintcraft.document/v2` and requires `source-metadata.v1`
in addition to the version-one raster, finite-canvas and flat-layer features.
Readers that only support version 1 must refuse it. Version 1 output remains
byte-identical when no attachment is present. A version 2 stream cannot omit its
attachment, and unknown versions remain unsupported.

The attachment starts immediately after the title. Its fixed 192-byte header
uses big-endian integers, followed by the exact original ICC payload when present.
All reserved bytes are zero; absent fields require their associated bytes to be
zero. These framing rules are independent of the little-endian tile encodings.

| Header offsets | Field |
|---|---|
| 0..8, 8..10 | ASCII `PCSMETA1`, u16 version 1 |
| 10, 11 | Source format tag; selection-present flag |
| 12..20, 20..52 | Original source byte count and SHA-256 |
| 52..56 | Source selection index, zero when absent |
| 56..72 | Four u32 values: original width/height, conversion output width/height |
| 72, 73 | Original orientation, zero when absent; materialization policy |
| 74, 75 | Resolution-present flag and normalized unit |
| 76..108 | X and Y resolution: each signed i64 numerator, unsigned u64 denominator |
| 108, 109, 110, 111 | Original resolution-unit-present flag, ICC-present flag, original sample-model tag, reserved zero |
| 112..120, 120..152 | Exact ICC payload length and SHA-256, zero when absent |
| 152..184 | Attachment SHA-256 |
| 184..188, 188..192 | Original resolution-unit u32, reserved zero |

All presence flags are exactly 0 or 1. Source format tags are PNG 1, JPEG 2,
GIF 3, BMP 4, TIFF 5, WebP 6, JXL 7, AVIF 8, HEIF 9, JXR 10 and other 11.
Sample models are unknown 0, gray 1, RGB 2, CMYK 3, YCbCr 4, YCCK 5 and indexed 6.
Orientation policy is unchanged 0 or EXIF-materialized-v1 1. Resolution units
are unspecified 0, inch 1, centimetre 2 and metre 3. Other enum values are invalid.
Original source extent is positive, and all dimensions obey the canvas side
limit. Source facts are claims captured by an importer, not authenticated by the
attachment digest.

The attachment digest hashes this header with bytes 152..184 zeroed, followed by
the exact ICC payload. It detects mismatches rather than authenticating source
claims. ICC interpretation is structural header/tag-bound validation; it does not
activate a working profile or demonstrate a color transform. Original CMYK and
other sample-model facts remain separate from the current RGBA8 pixels.

Materialized orientation requires a source tag in 1..8. Conversion geometry swaps
axes for tags 5..8; retaining the tag never instructs the renderer to rotate again.
These dimensions describe the original conversion, so later canvas resizing does
not overwrite them. Signed or unknown resolution declarations remain provenance;
nonpositive values are not exposed as physical DPI. Nonzero denominators and
unit/declaration framing are validated.

The fixed header and ICC payload count toward the existing aggregate 8 MiB
semantic metadata limit. Caller-owned admission can impose a lower attachment
allowance. Already-read prelude/title bytes reduce the remaining allowance before
profile capture. Profiles retain an ownership lease across shared snapshots and
history; this is logical resource accounting, not allocator or RSS interception.
Default reader wrappers bound each read, rather than aggregate application tabs.

The independent Python writer in `scripts/pcnd_source_metadata_reference.py`
serializes three version-two specimens directly; Rust reads and raw writes agree
byte-for-byte. Their existing version-one specimen is unchanged. See
`crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/` for the source
facts, original profile identity and exact scope.

The writer streams the header, metadata and tile payloads into a hashing sink,
then writes the digest. It never publishes a partially failed sink as a saved
file. The digest supplies corruption detection, not proof of origin or authenticity.


## Version 3 live point filters

Version 3 has schema `paintcraft.document/v3` and requires
`layer-point-filters.v1`. When its optional source attachment is present, header
assessment also reports `source-metadata.v1`. The source-presence byte immediately
after the title is exactly 0 or 1; an attachment, if present, uses the unchanged
version-two framing. It remains original-source information, not an applied
working profile.

Every layer has the following stack immediately after its default RGBA pixel and
before its tile records:

| Field | Representation |
|---|---|
| Instance count | Canonical integer, 0 through 256; admitted before stack allocation |
| Instance ID | 16 UUID bytes; unique within its layer, including disabled instances; nil is allowed |
| Enabled | Exactly one byte, 0 or 1 |
| Algorithm/version tag | One byte: 1 is `invert-rgb-v1`; 2 is `brightness-contrast-v1` |
| Brightness, contrast | Tag 2 only: two canonical ZigZag integers in signed hundredths; brightness −100 through 100, contrast −99 through 99 |

Order, IDs, disabled settings and parameters are persistent meaning. The reader
rejects unknown algorithm/version tags even when disabled, duplicate instance
IDs, malformed flags or noncanonical/out-of-range parameters. More than 256
instances is a resource refusal before allocation. Filter bytes count against the
same aggregate 8 MiB metadata limit; existing decoded tile and caller-owned source
metadata admission still apply before pixel inflation. The raw and selected
compact preflights count the same stack encoding.

Version 3 is emitted only when at least one layer stack is nonempty, including a
stack containing only disabled instances. An all-empty version-three stream is
noncanonical and refused. Version-one/two readers initialize empty stacks. When
all stacks are empty, writers retain the exact original version-one/two byte
layout; clearing the final stack therefore returns to that layout. Older readers
refuse version three rather than silently ignoring filters. Native filter
preservation is distinct from rendering qualification or foreign-format baking.

Independent standard-library Python fixtures under
`crates/services/paintcraft-formats/tests/pcnd-filter-fixtures` pin source samples,
wire bytes, disabled settings, source attachment preservation and malformed cases.
Their generator does not call the PaintCraft writer. Tests compare exact raw and
compact output against those bytes, including original hidden RGB and profile
bytes. These are native-format wire controls, not external application or GPU
qualification.
