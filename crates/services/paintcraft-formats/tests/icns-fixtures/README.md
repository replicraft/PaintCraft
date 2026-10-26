# ICNS resource qualification, first backend increment

The internal ICNS module preserves explicit ordered entry choices, logical size,
Retina scale and actual pixel dimensions. It uses pinned `icns` 0.5.0 only for
container read/write and type descriptors, with all optional features disabled.
The dependency tree adds only that crate, using existing `byteorder`; PNG pixels
use the existing image codec. No JPEG 2000 library or global image feature is
activated. No public registry, Classic Open, preserving Save or frontend workflow
is enabled in this increment.

## Admitted reader and source retention

- At most 64 MiB encoded input, 64 records, 8 MiB per recognized image payload,
  4,194,304 aggregate declared image pixels and 256 KiB aggregate inert
  metadata/mask payloads. Every record must fit the exact declared file extent.
- Optional single `TOC ` must match every other record's type, length and order.
  Directory records retain exact source ranges and tags; the original borrowed
  source remains unchanged. The selected container reader is called only after
  complete admission, and its ordered tags/payload bytes/consumption are checked.
- Eleven modern PNG types: `icp4/icp5/icp6`, `ic07/ic08/ic09/ic10` and
  `ic11/ic12/ic13/ic14`. Equal physical sizes do not merge logical sizes or scale.
  Duplicate types retain ordered indices and never implicitly choose the first.
- Noninterlaced eight-bit PNG profiles with complete chunk extents, CRCs and
  declared type dimensions. ICNS shares the existing ICO/CUR PNG admission and
  additionally allows a single valid pre-image `sRGB` declaration and one inert
  pre-image `eXIf` payload through 64 KiB, at most 8192 chunks. ICO/CUR's narrower
  ancillary-chunk profile is unchanged. Unsupported ICC/high-depth/extension
  profiles are refused rather than silently converted.
- Recognized legacy/JPEG 2000 entries are listed as unqualified and retained as
  inert source payloads; selecting one fails without fallback. Unknown metadata
  and masks do not trigger a decoder, filesystem access or resource adoption.
- Selecting a PNG explicitly reports discarded ICNS metadata, unselected entries,
  indexed-palette expansion, EXIF loss and Retina logical-density loss where
  applicable. RGBA pixels retain alpha and hidden RGB. The resulting flat
  document uses actual pixel dimensions, with entry index in its identity.

The pinned container reader allocates from record lengths and does not itself
prove exact whole-file consumption; its writer sums `u32` sizes. PaintCraft's
complete preflight and bounded output precede those paths. No upstream image
encoding/decoding method is used. Cooperative checks surround upstream work and
run during owned loops/output copies. These are input/allocation allowances,
not allocator/RSS interception or a hard within-library cancellation-tail bound.

## Fresh writer profile and negative native observations

Fresh PNG output currently admits eight explicit types:

| Type | Logical size | Scale | Pixels |
| --- | ---: | ---: | ---: |
| `ic07`, `ic08`, `ic09` | 128, 256, 512 | 1 | 128, 256, 512 |
| `ic11`, `ic12`, `ic13`, `ic14`, `ic10` | 16, 32, 128, 256, 512 | 2 | 32, 64, 256, 512, 1024 |

All eight match exact RGBA hashes through independent Pillow 13.0.0 and Apple
`iconutil` readback on macOS 14.7.2 arm64, including transparent hidden RGB.
Fresh output has a checked TOC and the requested types; it performs no implicit
resize, density substitution or source metadata preservation. Duplicate output
types and mismatched buffers are refused before encoding.

The separately owned isolated PNG probes are important exclusions. On that same
host, `icp4` extraction changed all 256 pixels and 192 alpha values; `icp5`
changed all 1024 pixels and 768 alpha values. A file containing only `icp6`
produced a 48x48 image rather than the requested logical 64x64 output. These
three types remain readable with exact independent Pillow evidence but are
refused by this fresh writer. A request for 16/32 at 1x is never relabeled as a
Retina entry. The retained [native report](native-report-macos-14.7.2.json)
records every positive/excluded observation, OS/architecture, source/output
hashes and `/usr/bin/iconutil` hash
`716676e3fc6a731e8904c36d672edbc5403a15597e2ba696c301661e3fe494b6`.

This is an initial qualified increment. A separately admitted legacy raw/RLE RGB
plus alpha-mask path is required for canonical 16/32 at 1x before claiming a
complete fresh application icon set. Additional encodings, metadata semantics,
original-application display behavior and frontend transactions remain work.

## Sources and independent fixtures

Primary implementation references are the selected crate's
[container reader/writer](https://github.com/mdsteele/rust-icns/blob/master/src/family.rs),
[element handling](https://github.com/mdsteele/rust-icns/blob/master/src/element.rs),
[type/density descriptors](https://github.com/mdsteele/rust-icns/blob/master/src/icontype.rs),
and the independently implemented
[Pillow ICNS plugin](https://github.com/python-pillow/Pillow/blob/main/src/PIL/IcnsImagePlugin.py).
[Chromium's icon documentation](https://chromium.googlesource.com/chromium/src/+/refs/tags/124.0.6338.1/docs/mac/icons.md)
also records historical problems with small modern icon types. That is additional
evidence, not a substitute for the isolated native probes above.

`generate.py` uses only Python standard-library PNG/ICNS wire packing and owned
pixel formulas. All fixture content is repository-authored under the repository
MIT license. No application icon, third-party image or implementation source is
copied. The eight source specimens total less than 36 KiB. `sources.json` records
lengths, hashes, logical/physical entry properties and expected pixel hashes.
Duplicate-type oracle checks decode each PNG separately because Pillow's
convenience container lookup retains only its last same-type entry. Legacy and
inert JPEG 2000 payloads have no positive decode claim.

```sh
python3 crates/services/paintcraft-formats/tests/icns-fixtures/generate.py
PAINTCRAFT_ICNS_ORACLE_DIR=/tmp/paintcraft-icns-check \
    cargo test -p paintcraft-formats --lib icns:: --offline --locked
/path/to/pillow13-python crates/services/paintcraft-formats/tests/icns-fixtures/oracle.py \
    /tmp/paintcraft-icns-check
/path/to/pillow13-python crates/services/paintcraft-formats/tests/icns-fixtures/native_oracle.py \
    /tmp/paintcraft-icns-check /tmp/paintcraft-icns-native-new-directory
cargo test -p paintcraft-formats --test icons --test icon_admission \
    --test icon_write --test icon_prepare --test icns_gate --offline --locked
cargo clippy -p paintcraft-formats --lib --tests --offline --locked -- -D warnings
```

Native probes use only command-line `iconutil`, without launching a window. The
qualification environment used Python 3.14.0/Pillow 13.0.0. Seven ICNS tests cover
independent pixels/types/density, duplicate entry identity, inert metadata,
unsupported selection, whole-source/TOC/PNG validation, resource limits, writer
refusal, cancellation at every exposed checkpoint and deterministic mutations.
The ICO/CUR regression suite retains its existing independent fixture coverage.
