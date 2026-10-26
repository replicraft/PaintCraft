# ABR resource fixtures

The internal reader uses selected `ag-psd` 0.3.0 after a complete structural
preflight. ABR resources are not image documents, and this implementation does
not register public Open, import, brush adoption or export operations. Parsed
presets alone do not implement Photoshop brush behavior in PaintCraft.

## Admitted profile

- ABR major versions 6, 7, 9 and 10, with minor version 1 or 2.
- `samp` sections containing 8-bit or 16-bit raw/PackBits masks.
- `patt` version-1 records containing RGB, grayscale or indexed patterns, with
  bounded VMAL version-3 arrays. Eight-bit, sixteen-bit and finite f32 channel
  values remain exact, including low bits and negative-zero float bits. Raw and
  PackBits channels, partial rectangles, absent arrays, empty-written arrays,
  palettes and optional palette words retain their original slot positions.
- `desc` and `phry` version-16 Action Descriptors, including bounded nested
  descriptors/lists, references, finite numbers, units, text, enums, integers,
  Booleans, class structures, aliases, raw data, object arrays and file-path
  values. Unknown types, unsupported units and malformed extents are refused.
- Original signed sample bounds, null-free ASCII sample IDs, exact mask values
  and complete ordered section payloads remain in the resource. Descriptor and
  hierarchy bytes include fields omitted or defaulted by the backend's parsed
  convenience model. File-path/alias metadata is parsed as data; it never opens
  an external file or activates a resource.

The backend narrows 16-bit mask values to their high eight bits. PaintCraft
cross-checks that decoded view but separately retains every original `u16`,
including the low bits. There is no implicit color conversion, raster-document
publication, preset installation or brush-engine mutation.

For 16-bit PackBits, complete preflight expands each byte-oriented row to exactly
twice its sample width and retains the big-endian `u16` values. Because pinned
Rust `ag-psd` lacks this compression mode, PaintCraft builds a private equivalent
raw-16 decoder view capped at 48 MiB. It copies descriptor/hierarchy sections and
all unaffected sample records verbatim; only compressed 16-bit records change.
IDs, opaque header bytes, signed bounds and depth stay exact. Each replacement
record is decoded and compared with its full-precision source before the selected
library verifies metadata and high-byte alpha. Original section payloads in the
returned resource remain compressed and unchanged. This is internal decoder
normalization, not ABR export or mutation of the acquired source file.

Pattern resources retain exact channels separately from `converted_preview()`.
The [Adobe VMAL specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/)
places two mask slots after the declared color slots. Previews require canonical
RGB/gray/indexed color slots, no additional written color slots and no written
user-mask slot; only the last sheet-mask slot supplies optional alpha. Missing
or empty color planes, extra color/spot planes and user masks keep their exact
resource data but return a `PreviewUnavailable` reason and no RGBA thumbnail.
This avoids guessing roles by written ordinal or shifting green into red.

Optional indexed palette words remain opaque rather than being treated as
padding. Adobe's Patterns table does not define those four bytes, while its
separate Color Table (`.ACT`) section assigns color-count and transparency-index
semantics to an optional four-byte tail. That does not establish those semantics
for patterns. Until pattern-specific interpretation is qualified, any present
tail, including all zeroes or the word 3, returns `PaletteMetadata` and no preview.
No-tail indexed resources can use the documented palette plus sheet-mask layout.

Canonical previews explicitly reduce 16-bit values by rounded division by 257
and finite float values by clamping to 0..1, scaling by 255 and rounding.
Precision reduction and clamping are reported. These are unmanaged informational
straight-RGBA8 values, not color-managed display or native Paint/Photoshop
rendering claims. Original alpha and hidden RGB remain in their source planes.
The selected pattern reader omits alpha and mishandles some precision/layout
variants, so it sees only an admitted converted RGB8 view built by its selected
writer. All original pattern sections are omitted from the whole-file backend
view and remain unchanged in the resource. No pattern is installed or adopted.

Admission allows at most 16 MiB of source, 64 sections, 256 samples, 16,384 pixels
per dimension, 4,194,304 pixels per sample and 16,777,216 aggregate sample pixels.
Descriptor allowances are 32,768 aggregate counted items/values, nesting depth
16, 4,096 UTF-16 units per string or ASCII bytes per key/alias, 1 MiB of aggregate
descriptor text and 4 MiB of aggregate raw descriptor blobs. PackBits packets
must consume complete payloads and produce exactly one admitted row. Bounds and
pixel arithmetic are checked before the backend allocates its mask buffers.
Patterns add allowances of 64 records, 4,194,304 pixels each, 16,777,216 aggregate
pixels, 66 array slots per record, 16,777,216 aggregate channel values and 64 MiB
of decoded channel bytes. Names allow 1,024 UTF-16 units and 256 KiB aggregate
name/ID bytes. Private converted pattern views are admitted before upstream work
at no more than 36 MiB each and 144 MiB aggregate conservative encoded bounds;
they are processed one at a time. Actual source VMAL and channel extents, depth
agreements and complete decoded row lengths are checked independently.

These are structural/input allowances, not a measured hard allocator, resident
memory, parser confinement or cancellation-tail guarantee. Checks surround
upstream calls and run throughout preflight; upstream decoding has no internal
cooperative cancellation callback. CMYK/other pattern color models, one-bit or
ZIP/predictive pattern channels, ambiguous preview layouts, legacy ABR 1/2,
actual brush rendering/adoption and original-application
qualification remain unimplemented or unqualified. ABR export is outside the
requested import-only asset policy.

## Provenance and coverage

`generate.py` uses only Python standard-library big-endian packing to author
the fixtures under the repository's MIT license. No third-party specimen or
implementation source is copied. `sources.json` records byte counts and SHA-256.

| Fixture | Bytes | Coverage |
| --- | ---: | --- |
| `empty.abr` | 4 | Empty version-6/minor-1 resource |
| `samples-v6.abr` | 992 | Raw/PackBits 8-bit and raw 16-bit masks, negative origin, computed/sampled brush descriptors, Unicode/non-BMP preset name, unknown retained field and hierarchy metadata |
| `samples-v10.abr` | 316 | Minor-2 sample layout with its longer header |
| `rle16-v6.abr` | 1012 | Minor-1 16-bit PackBits, byte runs crossing sample boundaries, no-ops, low bits, nonzero opaque header data and multiple sample sections separated by metadata |
| `rle16-v10.abr` | 1776 | The same compressed-mask and preservation checks with minor-2 headers |
| `patterns.abr` | 2748 | Five RGB/gray/indexed patterns, raw/PackBits 8/16/f32 channels, alpha, negative zero, finite out-of-range floats, partial rectangles and Unicode metadata |
| `indexed-no-extra.abr`, `indexed-extra3.abr` | See `sources.json` | Both optional palette-word layouts, including a word equal to the VMAL version |
| `auxiliary.abr`, `missing-color.abr`, `empty-color.abr`, `user-mask.abr` | See `sources.json` | Exact retained slots with explicit unavailable-preview reasons |

The Rust tests also exercise all admitted major/minor combinations, malformed
PackBits packets, every non-section-boundary truncated prefix, appended garbage,
descriptor resource limits and malformed strings/numbers, aggregate sample and
section counts, every exposed cancellation checkpoint, and deterministic
zero/255 mutations at every position of the mixed fixture.

The optional `oracle.cjs` reads these owned source specimens with upstream
JavaScript [`ag-psd` 33.0.0](https://github.com/Agamnentzar/ag-psd), checking mask
values, names, bounds, brush shape parameters and references. The 2026-10-25
local run used Node.js **24.10.0** and passed the initial five mask/preset
fixtures; additional pattern evidence and exclusions are recorded below. That reader and
the selected Rust port share implementation lineage, so this is cross-language
reader evidence rather than an independent algorithm or Adobe-application
qualification. Both upstream convenience models narrow the 16-bit sample; the
separate Rust assertion proves exact `[0,1,255,256,32769,65535]` retention.
The JavaScript reader's public 16-bit channel helper additionally checks every
low bit in both raw and compressed specimens. When supplied an optional directory
of Rust-generated private raw-16 views, the oracle compares decoded preset
metadata, signed bounds, opaque sample headers, descriptor/hierarchy bytes and
every exact 16-bit mask value with the original compressed sources.
The optional oracle does not qualify hierarchy interpretation, rendering parity,
hostile-file memory behavior, external file outputs or other platforms.

`pattern_oracle.py` uses independent `psd-tools` **1.26.0** rather than the selected
library's implementation lineage. Its local source-fixture check passed 45 exact
array slots across 10 patterns in six specimens, including original 16/f32 bits,
alpha, palettes, partial bounds, empty/unwritten slots and auxiliary/user masks.
It qualifies channel/resource parsing rather than a general pattern compositor.
The JavaScript oracle additionally checks five qualified preview vectors. Its
strict missing-feature mode records a known refusal of the optional palette word
equal to 3; `psd-tools` verifies that specimen. Ambiguous channel-role previews
and all present palette tails are explicitly unqualified, even where JavaScript
guesses an image. The channel checks do not prove palette-tail opacity semantics.

## Reproduction

```sh
python3 -B crates/services/paintcraft-formats/tests/abr-fixtures/generate.py
CARGO_INCREMENTAL=0 cargo test -p paintcraft-formats --lib abr:: --offline --locked
CARGO_INCREMENTAL=0 cargo clippy -p paintcraft-formats --lib --tests --offline --locked -- -D warnings

mkdir -p /tmp/paintcraft-abr-oracle
pnpm --dir /tmp/paintcraft-abr-oracle add --save-exact --ignore-scripts ag-psd@33.0.0
node crates/services/paintcraft-formats/tests/abr-fixtures/oracle.cjs \
    /tmp/paintcraft-abr-oracle/node_modules/ag-psd

# Use a new directory. The opt-in test refuses to overwrite previous evidence.
PAINTCRAFT_ABR_NORMALIZED_DIR=/tmp/new-abr-private-views CARGO_INCREMENTAL=0 \
    cargo test -p paintcraft-formats --lib \
    abr::tests::rle16_masks_keep_every_bit_and_only_private_decoder_views_change --offline --locked
node crates/services/paintcraft-formats/tests/abr-fixtures/oracle.cjs \
    /tmp/paintcraft-abr-oracle/node_modules/ag-psd /tmp/new-abr-private-views

python3 -m venv /tmp/paintcraft-pattern-oracle
/tmp/paintcraft-pattern-oracle/bin/pip install psd-tools==1.26.0
/tmp/paintcraft-pattern-oracle/bin/python -B \
    crates/services/paintcraft-formats/tests/abr-fixtures/pattern_oracle.py
```

The JavaScript package is optional development tooling in an isolated directory,
not a product dependency. Package installation needs registry access; Rust
tests and the fixture generator do not invoke Node or install anything.
