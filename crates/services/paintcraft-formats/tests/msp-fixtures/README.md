# Historical MSP fixtures

This backend handles an initial monochrome profile of Windows Paint `.msp`,
independently of modern layered `.paint`. The shared facade now supports explicit
Studio import and exact MSP1 export in desktop/browser, CLI and MCP workflows.
Headless Classic profiles retain an explicit legacy CLI import route; normal
Classic Open/Save filters and preserving MSP Save remain unavailable. See
[workflow qualification](../../../../../docs/FORMAT_INTEROPERABILITY.md#historical-msp-conversion-workflows-2026-10-25).

## Evidence and admitted profile

The primary reference is [Pillow's MSP implementation](https://github.com/python-pillow/Pillow/blob/main/src/PIL/MspImagePlugin.py).
Its reader identifies `DanM` and `LinS`, checks the XOR of sixteen little-endian
header words, and interprets byte-aligned MSB-first monochrome rows. Its MSP2
row map holds encoded byte lengths followed by literal/repeat packets. Its fresh
writer emits MSP1. PaintCraft's separately authored implementation and owned
wire fixtures are checked against installed Pillow 13.0.0, plugin SHA-256
`8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6`.

Pillow explicitly describes zero-length MSP2 rows as an inferred white-fill
behavior. PaintCraft refuses that variant. The historical IBM publication linked
by Pillow could not be retrieved during this qualification, and the linked
PRONOM endpoint returned HTTP 403. Neither an original Windows 1/2 Paint consumer
nor a complete normative Microsoft specification was available. These results
qualify the admitted codec profile and independent Pillow interoperability;
they do not establish complete historical application compatibility.

The reader admits:

- `DanM`: exact 32-byte header followed by exactly `ceil(width/8)` bytes per row.
- `LinS`: exact header and one nonzero encoded-length word per row. Literal and
  repeated byte packets must consume that row's full payload and produce exactly
  its packed row size. Zero-count repeats, truncated packets, cross-row output,
  over/under-expansion and unclaimed trailing bytes are refused.
- Positive dimensions through 65,535, at most 4,194,304 pixels and 16 MiB of
  encoded input. Complete preflight precedes decoded-pixel allocation.
- All original sixteen header words and every decoded packed byte, including
  unused final-byte bits. Non-geometric header words remain opaque metadata;
  physical device/aspect scaling is not inferred or applied.

Bit zero expands to opaque black and bit one to opaque white. The separately
callable RGBA expansion does not change the retained packed resource. Fresh
MSP1 writers accept exact packed bits or already opaque black/white RGBA only.
Colors, gray values and alpha require an explicit future conversion policy;
they are never thresholded, dithered or flattened implicitly. Fresh output uses
canonical header values compatible with the pinned independent writer; it does
not preserve source header metadata or MSP2 packet layout. MSP2 writing and
zero-length-row handling remain unqualified.

All codec APIs take borrowed bytes and return owned values. They perform no
filesystem writes or document publication, so callers retain the original source
unchanged on failures or cancellation. Native and MCP consumers attach existing
frozen-source save guards to the same opened source used for decoding; CLI holds
that identity for import/placement commands. All imports require named metadata
consent and use converted-copy receipts. Browser exports download fresh copies.
No implicit original-file association or preserving MSP Save is created. These are bounded input/allocation declarations and cooperative
checkpoints, not allocator/RSS interception or a measured cancellation-tail bound.

## Fixture provenance

All pixels and fixture contents are repository-authored under the repository's
MIT license; no third-party images or proprietary application files are copied.
`sources.json` records source length/hash, expected packed bytes and RGBA hashes.

| Fixture | Coverage |
| --- | --- |
| `raw-1.msp`, `raw-7.msp` | Odd widths, alternating black/white, solid rows |
| `raw-9.msp` | Independent explicit pixel rows, non-default device/header words, nonzero unused tail bits |
| `raw-257.msp` | Multi-byte odd rows and row boundaries |
| `rle-9.msp` | Literal/repeat packets matching `raw-9` pixels |
| `rle-2048.msp` | 255-byte runs, literal tail and adjacent encoded rows |
| `pillow-7.msp` | Separately generated independent Pillow MSP1 writer specimen from owned 7x3 pixels |

`generate.py` uses only standard-library wire packing for the first six files.
`pillow_fixture.py` independently writes the seventh. Regenerate with:

```sh
python3 crates/services/paintcraft-formats/tests/msp-fixtures/generate.py
/path/to/pillow-13-python crates/services/paintcraft-formats/tests/msp-fixtures/pillow_fixture.py
python3 crates/services/paintcraft-formats/tests/msp-fixtures/generate.py
```

The final generator pass includes the independently written fixture in the hash
manifest. Optional Pillow tooling is not a product/runtime dependency. The
qualification environment used Python 3.14.0 and Pillow 13.0.0.

## Reproduce qualification

From the repository root, provide a fresh temporary output directory:

```sh
PAINTCRAFT_MSP_ORACLE_DIR=/tmp/paintcraft-msp-check \
    cargo test -p paintcraft-formats --lib msp:: --offline --locked
/path/to/pillow-13-python crates/services/paintcraft-formats/tests/msp-fixtures/oracle.py \
    /tmp/paintcraft-msp-check
cargo clippy -p paintcraft-formats --lib --tests --offline --locked -- -D warnings
```

Seven Rust tests cover independent exact pixels and metadata/tail retention,
fresh writers, all source prefixes, header checksums, pixel/input extents,
malformed maps and packets, explicit conversion refusal, cancellation at every
exposed checkpoint and deterministic single-bit mutations. The optional oracle
checks all seven source specimens and all seven fresh Rust outputs through the
independent reader. Source fixtures stay unchanged; writer outputs go only to the
explicit temporary directory. Original-application qualification remains pending.
