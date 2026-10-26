# Fresh MSP2 row-RLE writer qualification

The files here are repository-authored exact monochrome pixels, encoded by the
Rust MSP2 writer. They contain no third-party artwork or original Paint file.
Normal Rust tests re-encode and compare every byte to these independently verified
specimens. File export is optional, create_new only; it never overwrites fixtures.

## Framing evidence and limitations

The [Encyclopedia of Graphics File Formats MSP chapter](https://www.fileformat.info/format/mspaint/egff.htm)
provides the 32-byte little-endian header, LinS keys, XOR checksum, one u16 encoded
byte length per scanline and literal/repeat packet syntax. This published author
reference is not a complete normative Microsoft specification. Its bitmap-size
summary omits odd-width rounding; the admitted existing reader profile and pinned
independent Pillow consumer use ceil(width/8) bytes per row. We retain the full
packed row, including unused last-byte bits.

[Pillow's MSP reader](https://pillow.readthedocs.io/en/stable/_modules/PIL/MspImagePlugin.html)
independently implements the same row-map and packet syntax. Qualification used
Python 3.14.0 and Pillow 13.0.0; `verify.py` checks the installed plugin SHA256
`8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6` before decoding.
The script independently parses each complete row and compares every packed byte,
then asks the untouched Pillow reader to verify every visible RGBA pixel. Its
`pillow.json` report records file/packed/pixel hashes and packet/row counts.

No original Windows 1/2 Paint consumer was used. These results qualify the bounded
writer and independent Pillow profile; they do not establish original-application
compatibility, enable public MSP2 export, or enable foreign preserving Save.
Existing MSP1 public export remains unchanged.

## Specimens

| File | Coverage |
| --- | --- |
| `tail-1.msp` | Width 1, different unused tail bits in both rows |
| `tail-9.msp` | Width 9, mixed literals and nonzero unused bits |
| `runs-2048.msp` | 256-byte solid rows split at the 255-byte run limit; all 256 literal values |
| `wide-65535.msp` | Maximum admitted row width, nonrepeating literals across packet boundaries |

The production writer uses deterministic greedy repeats of at least three bytes
and literal groups of 1..255 bytes. It does not claim optimal compression. All map
entries are nonzero; the ambiguous zero-length white-fill variant is never emitted.
Each encoded packet stays inside its source row. Fresh header device/aspect fields
use the existing MSP1 writer policy (four aspect fields equal 1, printer dimensions
equal image dimensions, correction/reserved fields zero); source device metadata
is not preserved or treated as physical resolution.

## Bounds and cancellation

The existing geometry policy requires positive u16 dimensions and at most
4,194,304 pixels. Packed length must exactly equal ceil(width/8)×height. A complete
packet walk computes exact nonzero u16 row lengths and total output before output
allocation. Total output is limited to 16MiB. One shared packet walker then emits
that exact plan directly into the single reserved output vector, without an
additional row or image codec buffer. The conservative per-row packet bound is
2×packed-row bytes (at most 16,384 bytes at the largest width).

`V2WritePlan.transient_bytes` prices the borrowed packed source plus encoded
output; it is logical payload pricing, not an allocator/RSS ceiling. The exact
RGBA convenience function adds its borrowed RGBA source and one packed buffer;
it never clones RGBA. It accepts only opaque black/white and zeroes unused bits
of newly packed RGBA. Direct packed input preserves unused bits exactly. No
thresholding, dithering, alpha removal, palette or scaling is inferred.

Checks occur before geometry/source validation, per row and every 256 packets
in both admission and encoding. Errors return no partial output and never mutate
the source. The codec performs no filesystem or document publication; consumers
remain responsible for captured identities and atomic output. Focused tests also
cover maximum height, 1/2/3/254/255/256/257/510/511-byte repeats, truncated frames,
invalid geometry/source/alpha/color and cancellation at every exposed checkpoint.

## Reproduce

From the repository root, with the pinned Pillow environment:

```sh
cargo test -p paintcraft-formats --test msp2_write --offline --locked
/path/to/pillow-13-python crates/services/paintcraft-formats/tests/msp2-writer-fixtures/verify.py \
    crates/services/paintcraft-formats/tests/msp2-writer-fixtures
mkdir /private/tmp/paintcraft-msp2-owned-output
PAINTCRAFT_MSP2_WRITER_OUTPUT=/private/tmp/paintcraft-msp2-owned-output \
    cargo test -p paintcraft-formats --test msp2_write writer_matches --offline --locked
/path/to/pillow-13-python crates/services/paintcraft-formats/tests/msp2-writer-fixtures/verify.py \
    /private/tmp/paintcraft-msp2-owned-output
```

Optional Python tooling is qualification-only, not a product dependency.
