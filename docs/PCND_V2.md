# PCND 2 wire layout

This is the current compact-save contract. It describes flat raster documents
with the `paintcraft.core/default-v1` compositing policy. It is distinct from the
PCND 1 ZIP fixture contract. Older unpublished builds need not read PCND 2.

All integers below are canonical unsigned LEB128 unless specified otherwise.
Redundant leading zero groups, overflow and truncated integers are invalid.
A signed origin uses ZigZag: `(value << 1) ^ (value >> 63)`. UUID bytes use UUID
network order, exactly as `Uuid::as_bytes()`. Strings are a byte length followed by
that many UTF-8 bytes; no terminator or normalization is applied.

The stream consists of:

| Field | Representation |
|---|---|
| Magic and version | Eight bytes: `50 43 4e 44 02 0d 0a 1a` |
| Document ID | 16 bytes |
| State ID | 16 bytes |
| Canvas width, height | Two positive integers, each at most 1,048,576 |
| Title presence | One byte, exactly 0 or 1 |
| Title, if present | UTF-8 string, at most 4,096 characters and 16,384 bytes |
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

There is no optional feature block or arbitrary metadata extension in version 2.
Its fixed semantics imply sRGB, straight RGBA8, normal layer blending, finite
canvas, flat roots and 256-pixel tiles. New semantics require a new version, whose
reader must be explicitly admitted. Empty title and absent title remain distinct.
Unreferenced layers, duplicate IDs, invalid flags and trailing body bytes are
rejected. Total serialized metadata is capped at 8 MiB, and the whole decoded tile
plan must fit the caller's admission budget before any tile is inflated.

The writer streams the header, metadata and tile payloads into a hashing sink,
then writes the digest. It never publishes a partially failed sink as a saved
file. The digest supplies corruption detection, not proof of origin or authenticity.
