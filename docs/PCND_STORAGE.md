# PCND storage

Normal desktop, browser and CLI project saves now use **PCND 2**, a compact binary
stream. It stores canvas dimensions and layer defaults rather than allocating a
canvas-sized pixel buffer. Only changed sparse tiles are written. There is no
`document.json`, ZIP directory, repeated field names, chunk filename, or hexadecimal
UUID/hash string in a normal save.

The editor still uses sparse 256 × 256 tiles. Missing tiles use the layer's exact
RGBA default. A blank document therefore has no pixel payload. Touched boundary
tiles store only their valid rows and columns. Hidden RGB under zero alpha is
preserved, along with document/state IDs, layer order, names, origins, visibility,
opacity and the optional document title.

## Measured sizes

Actual serialized 1000 × 486 test documents, in bytes:

| Document | Previous normal compact save | PCND 2 |
|---|---:|---:|
| Blank white canvas | 1,123 | 118 |
| One changed pixel | 1,467 | 140 |
| Eight isolated pixels in eight tiles | 3,066 | 294 |
| Dense pseudorandom RGBA | 1,946,931 | 1,944,161 |
| 920-point sinusoidal pencil fixture | 6,281 | 3,322 |

The blank document is 89.5% smaller, and the one-pixel document is 90.5% smaller
than the previous compact writer. The last row uses hidden RGB test ink, not the
user's screenshot file. A separate black-ink sinusoidal fixture measures 3,345
native bytes and 3,361 bytes with the portable browser encoder. The tests print
these measurements and compare the reopened document's canonical bytes exactly.
These are storage measurements, without a save-speed or peak-memory claim.

## Pixel compression

Each tile selects the smallest supported encoding. Every platform tries:

- Raw straight RGBA, for data that does not compress.
- RGBA runs: a little-endian 16-bit count minus one, then four exact color bytes.
- LZ4 over raw RGBA.
- LZ4 over the RGBA run descriptors, including a bounded intermediate length.

Native writers additionally try Zstandard level 3 with a maximum 256 KiB window.
The browser can read those frames through the shared pure Rust `ruzstd` decoder.
A tile with one isolated changed pixel normally needs only three RGBA runs, or
18 payload bytes. Compressing repeated run descriptors helps pencil lines whose
pixel rows otherwise repeat background runs. Encodings are lossless and chosen
only when they save bytes. Runtime tile storage is unchanged.

## Integrity, bounds and streaming

The stream has a versioned magic header and a trailing binary SHA-256 over all
preceding bytes. The reader verifies integrity before decoding pixel payloads.
Integers use canonical unsigned LEB128; origins use ZigZag encoding. UUIDs occupy
16 bytes. Text has explicit byte and character bounds. The fixed semantic schema
is flat raster layers, normal blend, finite canvas, straight sRGB RGBA8, 256-pixel
tiles and `paintcraft.core/default-v1`. An incompatible semantic change requires a
new container version. See [the wire layout](PCND_V2.md).

The reader validates dimensions, UTF-8, flags, layer IDs, duplicate coordinates,
encoded lengths, metadata limits and the complete tile allocation plan before
inflation. It borrows stored payload slices rather than copying chunks. LZ4 uses
caller-sized output, and run-LZ4 scratch is limited to six bytes per decoded pixel.
Zstandard header/window/content-size checks precede decoder allocation; exact
output length, frame checksums and absence of trailing frames are required.

Checked writing streams one tile at a time and honors sink backpressure. It no
longer performs a second pixel-compression pass to prepare JSON offsets. Failed
sinks and cancelled operations never report success. Callers must discard an
unsuccessful private output. Checked reading observes cancellation while hashing,
parsing records and decoding. Individual codec calls remain bounded rather than
being interruptible midway through a call; no cancellation-latency claim is made.

## Canonical fixtures and qualification

`write` and `write_to` retain the original canonical PCND 1 ZIP output for the
published conformance fixtures. `write_compact`, `write_compact_to` and the checked
variant produce PCND 2. All normal application save paths select compact output.
The reader still accepts the supported PCND 1 encodings. Reading a PCND 2 file and
writing it canonically reproduces the original canonical fixture bytes exactly.
Unpublished older PaintCraft builds cannot open PCND 2.

Tests cover every field, Unicode and hidden RGB, boundary tiles, negative origins,
incompressible fallback, portable/native interoperability, every byte corruption
and truncation of a small fixture, forged invalid codec payloads, resource admission
before inflation, malformed integer/string lengths, trailing data, partial writes,
failing sinks and cancellation at every observed checkpoint. The canonical
specimen and conformance digest tests remain unchanged.

Raw measurements and production browser evidence are retained in ignored
`docs/validation/ui-storage-polish-2026-10-24/`. Broader format conformance and
representative performance studies remain roadmap work.

## Production browser check

The production Wasm build downloaded a blank 1000 × 486 project at 118 bytes and a
short horizontal pencil line at 140 bytes. An independent Python wire reader
verified each whole-file SHA-256 and recovered exactly 192 contiguous black pixels
on the white default, with an 18-byte run payload for the one touched tile.
Reopening that download through the real browser file picker left the document
clean; saving it again produced byte-identical output. The page title updated to
`pencil.pcnd - PaintCraft`. This is a dedicated headless Chromium check, without
physical mobile hardware or cross-browser qualification. The scripts, downloaded
files, screenshots and oracle results are in the ignored evidence directory above.
