# PCND storage

PaintCraft uses sparse 256 × 256 raster tiles. Missing tiles use the layer's exact
default pixel. A blank canvas therefore needs no pixel chunks. A tile containing
even one changed pixel has a dense RGBA payload: 262,144 bytes for a full tile.
Boundary tiles contain only their valid rows and columns. Hidden RGB under zero
alpha counts as changed data and is preserved.

The original canonical PCND writer stores those nondefault blocks raw. The optional
compact writer compresses each block independently with Zstandard level 3 and a
256 KiB maximum window, retaining raw bytes when compression would be larger.
Chunks still use ordinary stored ZIP members, grouping blocks toward 4 MiB.

## Measured file sizes

These are actual serialized 1000 × 486 documents produced by
`compact_storage_sizes_and_exact_roundtrip_for_blank_one_pixel_sparse_and_dense`.
The sparse fixture changes one pixel in each of the eight canvas tiles. The dense
fixture contains deterministic xorshift RGBA noise. All figures are bytes.

| Document | Canonical file | Compact file | Raw pixel chunks | Compact pixel chunks |
| --- | ---: | ---: | ---: | ---: |
| Blank | 1,123 | 1,123 | 0 | 0 |
| One changed pixel | 263,591 | 1,482 | 262,144 | 38 |
| Eight sparse changed pixels | 1,946,936 | 3,189 | 1,944,000 | 304 |
| Dense noise | 1,946,931 | 1,946,931 | 1,944,000 | 1,944,000 |

The one-pixel total file is about 178 times smaller; its pixel chunk is about
6,899 times smaller. These examples measure storage, without a save-speed or
runtime-memory improvement claim. Photograph and artwork compression depends on
their content. Compressing persistence does not change dense runtime tile storage.

## Compatibility and safety

The existing [PCND v1 contract](temp/paintcraft-implementation-spec-v5-hyperfine/docs/features/NATIVE_PROJECT.md)
already permits a raw block or one bounded Zstandard frame and hashes the decoded
RGBA bytes. Compact output uses that existing encoding, preserving document IDs,
StateIds, layer metadata, defaults, off-canvas layer content and hidden RGB.
`write` and `write_to` remain the original canonical raw writer. Their specimen and
conformance digests remain unchanged. Reading compact output and writing it
canonically reproduces the original raw bytes.

Native builds expose `write_compact`, `write_compact_to` and a checked streaming
variant. The desktop's confirmed-save paths choose compact output. Encoding uses
the locked native Zstandard library. Compression runs twice so the writer can
prepare descriptors before streaming chunks while retaining only bounded block
scratch and one payload chunk. Compact bytes are reproducible with the pinned
encoder; future compressor upgrades need independent byte-level qualification.

Native and browser builds share the pure Rust `ruzstd` decoder. Before its window
allocation, PaintCraft validates the frame header, dictionary absence, window and
declared content size, every block envelope, exact frame boundary and absence of
trailing or concatenated frames. The decoder must emit exactly the declared block
length; frame checksums, when present, and decoded SHA-256 must match. Frame windows
above 256 KiB are refused. The complete decoded tile set is admitted before any
block inflation. ZIP/member, canvas, block and document limits still apply.

Checked reading observes cancellation between records/blocks and decoded 16 KiB
output chunks; checked writing observes it between bounded block compressions and
output phases. ZIP parsing and an individual codec call are not interruptible.
These checkpoints do not establish native wall-clock cancellation latency.
Failures leave the source document unchanged; callers must discard an unsuccessful
output sink rather than publish it as a saved file.

The browser reads compact files but retains its canonical raw writer. Browser
compact encoding is explicitly unavailable. Wasm compilation establishes decoder
availability, while live browser/native filesystem qualification remains separate.

Tests cover exact roundtrips and canonical digests, mixed raw/compressed boundary
blocks, negative layer origins, hidden RGB, incompressible fallback, decoded quota
denials, every observed checked cancellation, corrupted archive blocks, every
truncated frame, checksum errors, dictionaries, excessive windows, concatenated
frames, trailing bytes and unknown-size frames with short or excessive output.

Raw local logs are retained in ignored `docs/validation/pcnd-storage-2026-10-24/`.

## Validation

The native formats suite passes, including the canonical specimen and conformance
roundtrips. Strict formats Clippy passes. The portable decoder compiles in the
actual browser dependency graph with:

```sh
cargo check -p paintcraft-formats -p paintcraft-web --target wasm32-unknown-unknown
```

Including the browser supplies its existing `uuid/js` randomness feature. An
isolated formats-only Wasm check lacks that preexisting feature and is not claimed
as passing. No live browser import or save-speed benchmark is inferred from this
compile check. Native filesystem/recovery integration is validated separately by
the desktop tests.
