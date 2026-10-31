# PCND format

PCND describes flat raster documents with the `paintcraft.core/default-v1`
compositing policy. Documents without retained source metadata use the initial
version 1 layout when all layer filter stacks are empty. Version 2 adds one
immutable original-source attachment. Version 3 preserves ordered point-filter
intentions without modifying source raster samples. Version 4 adds persistent
per-filter A8 masks. Version 5 adds persistent spatial Box Blur intentions. Tile encodings remain the same. The unreleased ZIP prototype
is not a supported format.

This adopted contract supersedes the ZIP proposal in the original implementation
reference pack. Historical prototype benchmark artifacts retain their original
bytes; they do not define a current compatibility target.

All integers below are canonical unsigned LEB128 unless specified otherwise.
Redundant leading zero groups, overflow and truncated integers are invalid.
Signed origins and v4/v5 mask endpoints use ZigZag: `(value << 1) ^ (value >> 63)`. UUID bytes use UUID
network order, exactly as `Uuid::as_bytes()`. Strings are a byte length followed by
that many UTF-8 bytes; no terminator or normalization is applied.

The stream consists of:

| Field | Representation |
|---|---|
| Magic and version | Eight bytes: `50 43 4e 44 VV 0d 0a 1a`, where VV is 01, 02, 03, 04 or 05 |
| Document ID | 16 bytes |
| State ID | 16 bytes |
| Canvas width, height | Two positive integers, each at most 1,048,576 |
| Title presence | One byte, exactly 0 or 1 |
| Title, if present | UTF-8 string, at most 4,096 characters and 16,384 bytes |
| Source attachment | v1 absent; v2 required; versions 3/4/5 use a strict 0/1 presence byte before the optional attachment |
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
| Filter stack | Versions 3/4/5: bounded ordered intentions; v4/5 add per-instance masks; only v5 admits spatial tag 7 |
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
`layer-point-filters.v1`. The header reports grammar capabilities, not actual optional source inventory. The source-presence byte immediately
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
| Algorithm/version tag | One byte, from the additive registry below |
| Parameters | Exactly the fields associated with that tag; no parameter defaults are inferred |

The registry applies to both version 3 and version 4 containers:

| Tag | Algorithm | Following parameter bytes |
|---|---|---|
| 1 | `invert-rgb-v1` | None |
| 2 | `brightness-contrast-v1` | Two canonical ZigZag integers: brightness −100…100, contrast −99…99 |
| 3 | `black-white-rgb-v1` | One unsigned byte, strength 0…100 |
| 4 | `threshold-luma-v1` | One unsigned byte, level 0…255 |
| 5 | `posterize-rgb-v1` | One unsigned byte, levels 2…32 |
| 6 | `solarize-rgb-v1` | None |

Tags 3–6 are an explicit additive algorithm-registry extension within existing
PCND3/4 framing, not a container-version change. Tags 1/2 and their parameter
bytes remain exactly unchanged, as do existing PCND1/2/3/4 specimens containing
only the original kinds. A reader lacking a known algorithm implementation must
return a typed refusal, even for a disabled instance; it must not reinterpret,
drop or skip that intention. Unknown tags and invalid parameter bytes likewise
refuse. [Point profiles](architecture/POINT_FILTER_PROFILES.md) define exact
encoded-RGB arithmetic, unchanged alpha/hidden RGB and once-per-instance masking.
These algorithms share `layer-point-filters.v1`; they add no feature-list names.

Order, IDs, disabled settings and parameters are persistent meaning. The reader
rejects unknown algorithm/version tags even when disabled, duplicate instance
IDs, malformed flags or noncanonical/out-of-range parameters. More than 256
instances is a resource refusal before allocation. Filter bytes count against the
same aggregate 8 MiB metadata limit; existing decoded tile and caller-owned source
metadata admission still apply before pixel inflation. The raw and selected
compact preflights count the same stack encoding.

Version 3 is emitted only when a point-only layer stack is nonempty and no
instance stores a mask, including a stack containing only disabled instances. An all-empty version-three stream is
noncanonical and refused. Version-one/two readers initialize empty stacks. When
all stacks are empty, writers retain the exact original version-one/two byte
layout; clearing the final stack therefore returns to that layout. Readers limited to versions one/two
refuse version three rather than silently ignoring filters. Native filter
preservation is distinct from rendering qualification or foreign-format baking.

Independent standard-library Python fixtures under
`crates/services/paintcraft-formats/tests/pcnd-filter-fixtures` pin source samples,
wire bytes, disabled settings, source attachment preservation and malformed cases.
Their generator does not call the PaintCraft writer. Tests compare exact raw and
compact output against those bytes, including original hidden RGB and profile
bytes. These are native-format wire controls, not external application or GPU
qualification.

## Version 4 persistent filter masks

Version 4 has schema `paintcraft.document/v4`. Its header feature order is
`raster.rgba8`, `canvas.finite`, `layers.flat`, `layer-point-filters.v1`,
`layer-filter-masks.v1`. Optional source presence is validated by the full reader,
not added to this version-derived header capability list. The title/source-presence/attachment and layer framing remain as in
version 3. The shared additive algorithm registry above applies before each
mask field. Original tags 1/2 retain their exact parameter bytes; tags 3–6 do not
require a new mask framing or container version.

The writer selects v4 when no spatial intention is stored and any filter instance stores a mask, including a
mask on a disabled filter, a disabled mask, or a field with zero coverage.
A v4 stream with no stored mask is noncanonical. Once no masks remain, the
writer returns to the unchanged v3 profile for nonempty stacks, or v1/v2 for
empty stacks. Existing unmasked v1/v2/v3 byte profiles and fixtures are unchanged.
Readers limited to those versions must refuse v4 instead of discarding masks.

Immediately after **each instance's algorithm and parameters**, v4 stores:

| Field | Representation |
|---|---|
| Mask presence | One byte, exactly 0 or 1; 0 ends this mask record |
| Mask enabled | Present only with a mask; one byte, exactly 0 or 1 |
| Mask codec/version | Present only with a mask; byte 1 is `a8-bands-v1` |
| Domain | Four canonical ZigZag integers: `x0,y0,x1,y1` |
| Band count | Canonical unsigned integer; zero represents an explicit zero field |
| Each band | Unsigned `y0-domain.y0`, positive height, positive run count |
| Each run | Unsigned `x0-domain.x0`, positive width, one coverage byte in 1–255 |

The domain is positive and finite, with endpoints bounded by the existing
absolute coordinate policy (140,737,488,355,327). Coordinates are local to the
layer source raster. A domain need not equal its raster bounds: negative,
smaller and latent off-layer fields are legal, while evaluation is confined to
the finite layer extent. All omitted samples, including outside the domain,
have coverage zero. A full-coverage rectangle uses a 255 run, not a dense plane.
No mask and a disabled mask instead mean whole-layer coverage; neither is
interchangeable with an enabled stored zero field.

Bands are ordered, nonoverlapping and inside the domain. Runs are ordered,
nonoverlapping and inside their band/domain; they have nonzero coverage and
positive widths. Adjacent equal-coverage runs merge. Adjacent bands with equal
run lists merge. Empty bands, zero-width/height records, zero-coverage runs,
noncanonical integers, unknown codecs and invalid flags are refused. Unknown
codecs remain unsupported even on disabled masks or disabled filter instances.

Reader and writer enforce **aggregate document** ceilings of 262,144 bands and
262,144 runs. Sharing one host payload between duplicate instances does not
reduce serialized counts: each instance is written and decoded independently.
Coverage work also has a document-wide checked traversal allowance; default
reader limits use 67,108,864 work units. Callers can inject lower limits.
All mask framing and coverage records count toward the unchanged aggregate
**8 MiB serialized metadata ceiling**, alongside layer fields, descriptors and
original-source attachment bytes. No extra metadata headroom is reserved or
invented for masks. Raw and selected-compact save preflights use the matching
record sizes and count guards. Application persistence validators apply these
checks before publication; rejected edits leave the document and history
unchanged. Library sessions require the matching host validator.

Library v4 reads require `read_with_admission_checked` with explicit
`ReadAdmission { metadata, coverage }`. `CoverageAdmission` carries the caller's
shared byte realm, byte allowance and aggregate band/run/work limits. Each
mask's borrowed records are validated before allocating charged parse scratch
and immutable A8 storage. Peak admission includes previously retained masks and
current decode temporaries. Existing metadata-only reader wrappers deliberately
refuse v4; they do not silently allocate a private coverage pool. Applications
use the explicit admission API for normal Open.

Default binary selections and retained A8 masks share one process-wide 256 MiB
coverage realm. Explicit caller realms remain available. Payload clones/views
share their unique reservation, and history deduplicates resident coverage
charges; mask storage remains resident when RGBA history tiles spill. This is
logical resource accounting, not allocator/RSS interception or qualification of
mask spilling. The independent source attachment retains its separate realm.

Independent stdlib Python specimens in
`crates/services/paintcraft-formats/tests/pcnd-mask-fixtures/` pin canonical
bands, fractional/zero/disabled masks, negative domains, shared-instance counts,
source attachment retention and malformed records. Their writer does not call
the product codec. See [the mask contract](architecture/FILTER_MASKS.md) for
selection capture, source transformations, identity, history, renderer and
frontend qualification. CLI/MCP summary and conditional result profiles are
specified in [editable filter commands](FILTER_COMMANDS.md).

## Version 5 persistent spatial intentions

Version 5 has schema `paintcraft.document/v5`. The header requires these grammar
capabilities in exact order: `raster.rgba8`, `canvas.finite`, `layers.flat`,
`layer-point-filters.v1`, `layer-filter-masks.v1`, `layer-spatial-filters.v1`.
Point and mask support are mandatory grammar capabilities even in an all-Box,
maskless file. The bounded probe derives this list from the version, without
scanning stacks or allocating masks. This is distinct from result/inspection
`document_features`, whose mask feature occurs only when a mask is actually stored.

The writer selects v5 iff any stored instance is `box-blur-v1`, including disabled
or radius-zero instances. Title, optional source attachment, layer, point-stack,
per-instance optional mask and tile framing reuse the existing layouts. Every
instance has the strict mask-presence byte used in v4, even when all are absent.
The algorithm registry adds **tag 7 followed by one radius byte 0–32 only in v5**.
Tags 1–6 retain exact encodings. PCND3/4 tag 7 remains unsupported, even disabled;
there is no silent registry widening in those versions. Unknown algorithms and
invalid radius bytes are refused before pixel inflation.

A v5 stream without a stored spatial intention is noncanonical. Removing the
last Box returns to v4 when masks remain, v3 for point-only nonempty stacks, or
exact v1/v2 for empty stacks. Old v1–4 specimen bytes remain unchanged. The fixed
8 MiB metadata ceiling, aggregate mask counts/work, explicit `ReadAdmission`,
shared 256 MiB coverage realm and resident history-mask ownership continue to
apply. Spatial rendering additionally requires a caller-owned workspace; file
readability is not permission for an unquoted render.

Independent Python fixtures pin v5 raw/compact bytes, Box parameters, masks,
source attachments, canonical downgrades and refusal cases. The 58 PCND controls
and two recovery controls pass. Model, exact Effects, renderer/required Metal,
typed CLI/stdio and Native/Web host/save/reopen qualification also pass; DDS/MSP
protocol 5 workspace/binding and native/Wasm strict controls pass locally.
[Spatial qualification](architecture/SPATIAL_FILTER_INTEGRATION.md#current-qualification)
records exact scope. Actual production Worker3 passes 17 sources/104 finite
controls, and seven Box Studio checks include exact stack/source preservation,
Undo/Redo and fresh PCND5 reopen/save. MAIN DDS export5 actual UI plus nine fresh
replays (eight negative) and independently decoded pixels pass. MAIN MSP export5 also passes finite consent/replay checks with independently
decoded exact 2×1 opaque-white pixels. The 27-asset [production receipts](architecture/SPATIAL_FILTER_INTEGRATION.md#production-browser-qualification)
remain distinct from file-format admission. Historical six-point evidence is unchanged;
the broader catalog and external-application compatibility are not established
by these local checks.

## Six-profile extension qualification

Core/Effects/Document, Commands, actual CLI/stdio-MCP consumers, independent
closed-schema controls and Native/Web host UI checks are qualified for the four
additional profiles. The full Formats suite passes 769 tests, and all 108
renderer tests pass, including 36 required offscreen Metal controls. The actual
production Chrome probe passes 305 render Worker and 32 DDS/MSP protocol 4 export
cases against independent sources and planes; nine positive artifacts independently
decode to the expected pixels. Four actual Chrome Studio flows also pass Add,
parameter changes for the three numeric kinds, Cancel, Apply, exact Undo/Redo and
fresh reopen/save. Independent default screenshots are causal for all four;
changed Threshold's uniform plane has Worker-only exact-pixel evidence. Fresh
reopen verifies strict committed BASE binding against the saved PCND source and
state after Apply legitimately promotes its admitted private texture. Existing
Edit, duplicate and preview-off are not newly qualified by these four runs.
Registering tags and passing these local checks do not
establish reader deployment, upstream application compatibility or full Palette
catalog coverage. Existing older byte-profile evidence retains its original scope.
