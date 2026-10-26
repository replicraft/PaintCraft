# ABR resource fixtures

The internal reader uses selected `ag-psd` 0.3.0 after a complete structural
preflight. ABR resources are not image documents, and this implementation does
not register public Open, import, brush adoption or export operations. Parsed
presets alone do not implement Photoshop brush behavior in PaintCraft.

## Admitted profile

- ABR major versions 6, 7, 9 and 10, with minor version 1 or 2.
- `samp` sections containing 8-bit raw/PackBits masks or 16-bit raw masks.
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

Admission allows at most 16 MiB of source, 64 sections, 256 samples, 16,384 pixels
per dimension, 4,194,304 pixels per sample and 16,777,216 aggregate sample pixels.
Descriptor allowances are 32,768 aggregate counted items/values, nesting depth
16, 4,096 UTF-16 units per string or ASCII bytes per key/alias, 1 MiB of aggregate
descriptor text and 4 MiB of aggregate raw descriptor blobs. PackBits packets
must consume complete payloads and produce exactly one admitted row. Bounds and
pixel arithmetic are checked before the backend allocates its mask buffers.

These are structural/input allowances, not a measured hard allocator, resident
memory, parser confinement or cancellation-tail guarantee. Checks surround
upstream calls and run throughout preflight; upstream decoding has no internal
cooperative cancellation callback. Embedded `patt` sections, legacy ABR 1/2,
16-bit PackBits, actual brush rendering/adoption and original-application
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

The Rust tests also exercise all admitted major/minor combinations, malformed
PackBits packets, every non-section-boundary truncated prefix, appended garbage,
descriptor resource limits and malformed strings/numbers, aggregate sample and
section counts, every exposed cancellation checkpoint, and deterministic
zero/255 mutations at every position of the mixed fixture.

The optional `oracle.cjs` reads these owned source specimens with upstream
JavaScript [`ag-psd` 33.0.0](https://github.com/Agamnentzar/ag-psd), checking mask
values, names, bounds, brush shape parameters and references. The 2026-10-25
local run used Node.js **24.10.0** and passed all three fixtures. That reader and
the selected Rust port share implementation lineage, so this is cross-language
reader evidence rather than an independent algorithm or Adobe-application
qualification. Both upstream convenience models narrow the 16-bit sample; the
separate Rust assertion proves exact `[0,1,255,256,32769,65535]` retention.
The optional oracle does not qualify hierarchy interpretation, rendering parity,
hostile-file memory behavior, external file outputs or other platforms.

## Reproduction

```sh
python3 -B crates/services/paintcraft-formats/tests/abr-fixtures/generate.py
CARGO_INCREMENTAL=0 cargo test -p paintcraft-formats --lib abr:: --offline --locked
CARGO_INCREMENTAL=0 cargo clippy -p paintcraft-formats --lib --tests --offline --locked -- -D warnings

mkdir -p /tmp/paintcraft-abr-oracle
pnpm --dir /tmp/paintcraft-abr-oracle add --save-exact --ignore-scripts ag-psd@33.0.0
node crates/services/paintcraft-formats/tests/abr-fixtures/oracle.cjs \
    /tmp/paintcraft-abr-oracle/node_modules/ag-psd
```

The JavaScript package is optional development tooling in an isolated directory,
not a product dependency. Package installation needs registry access; Rust
tests and the fixture generator do not invoke Node or install anything.
