# PCND filter catalog

Status: **Implemented and locally qualified for ten profiles.** PCND6 preserves
the existing seven numerical profiles, complete five-parameter Levels, Exposure
and five-point Curves.
Typed descriptors, document/history, prepared rendering, catalog automation and
Studio controls are implemented. Independent positive/negative format specimens
and exact metadata boundaries are tested. Existing PCND1–5 layouts and historical
automation schemas remain closed. Native controls, CLI/MCP, production Source5
previews, Studio lifecycle and DDS/MSP7 browser exports have separate finite
execution evidence. This grammar alone does not qualify a frontend or release.

Numerical contracts: [Levels](LEVELS_FILTER_FOUNDATION.md),
[Exposure](EXPOSURE_FILTER_FOUNDATION.md) and [Curves](CURVES_FILTER_FOUNDATION.md).

[Palette integration](PALETTE_INTEGRATION.md) tracks the wider inventory. Its 132
conventional filters include embedded LUTs, displacement rasters, geometry and
other ownership requirements. This grammar does not promise all 132 algorithms,
asset-bearing filter persistence, layer styles or arbitrary plugin execution.

## Required profiles and framing

PCND6 uses magic `PCND\x06\r\n\x1a` and document schema
`paintcraft.document/v6`. Its framing feature is `layer-filter-catalog.v1`.
The required algorithm catalog is physically encoded immediately after the magic,
before document scalars, original source metadata, masks or raster payloads.

All counts and lengths use the existing minimal unsigned LEB128 encoding.
Lengths count bytes. Fixed-width integers below use big-endian order.

```text
magic: 8 bytes
prelude_length: uvar
prelude[prelude_length]:
    profile_count: uvar
    repeat profile_count:
        profile_id_length: uvar
        profile_id: ASCII bytes
        codec_id_length: uvar
        codec_id: ASCII bytes
# exact prelude consumption

# existing PCND5 scalar ordering and field encodings:
document_uuid; state_uuid; canvas_width; canvas_height; title_presence/title
source_metadata_presence; optional unchanged source metadata
layer_count
repeat layer:
    unchanged layer scalar fields
    filter_count: uvar
    repeat filter:
        filter_uuid: 16 bytes
        enabled: u8
        profile_index: uvar
        parameter_length: uvar
        parameter_bytes[parameter_length]
        unchanged PCND4/5 mask-presence and optional canonical A8 field
    unchanged bounded tile records and zero terminator
SHA256(all preceding bytes): 32 bytes
```

Prelude entries are strictly sorted by profile-ID bytes, with one entry and one
transport codec per profile per file. Indexes are zero-based. Every stored
instance uses a declared index; every declaration must be used. Disabled filters,
hidden layers, identity parameters and zero or disabled masks retain full intent
and count toward requirements. Unknown algorithms are never skipped merely
because they are currently inactive.

Identifiers are case-sensitive ASCII: a lowercase letter followed by lowercase
letters, digits or hyphens, ending in a positive `-vN` suffix without a leading
zero in N. There is no case folding, Unicode normalization, executable payload,
URL lookup, generic extension table or unknown-profile passthrough.

## Registered profiles and parameter codecs

Every current transport codec is named `params-v1`, **scoped by its numerical
profile ID**. A codec name alone does not identify its semantics. Parameter bytes
must have the exact indicated length and pass the profile's checked constructor;
wire defaults, extra bytes, missing fields and float coercion are forbidden.

| Numerical profile ID | Exact parameter bytes | Bytes |
| --- | --- | ---: |
| `invert-rgb-v1` | Empty | 0 |
| `brightness-contrast-v1` | Brightness i16BE, contrast i16BE | 4 |
| `black-white-rgb-v1` | Strength u8 | 1 |
| `threshold-luma-v1` | Level u8 | 1 |
| `posterize-rgb-v1` | Levels u8 | 1 |
| `solarize-rgb-v1` | Empty | 0 |
| `box-blur-v1` | Radius u8 | 1 |
| `levels-rgb-v1` | Input black u8, input white u8, gamma hundredths u16BE, output black u8, output white u8 | 6 |
| `exposure-srgb-v1` | Stops hundredths i16BE | 2 |
| `curves-rgb-v1` | Five output u8 values for fixed inputs 0, 64, 128, 192, 255 | 5 |

Existing parameter ranges and arithmetic versions do not change. Levels requires
input black 0–254, input white 1–255 strictly above black, gamma 10–999, and byte
output endpoints, including equal and inverted outputs. Exposure requires signed
stops hundredths -800 through 800, including disabled/default zero intentions.
Curves permits every five-byte tuple, including identity, constants, decreasing
and nonmonotone outputs. Its exact Hermite arithmetic is fixed by the numerical
profile; LUT bytes are never serialized in place of those five parameters.
Prepared LUT bytes are
not parameters, persistent intent or authority. Old PCND numeric tags are not
reinterpreted as catalog indexes.

Additional registered numerical profiles fitting this grammar can use PCND6
without an outer bump. Their numerical contract, codec, admission and evaluator
still require independent implementation and qualification. A future model
needing resource sections or different ownership may require new framing.

## Bounds, admission and assessment

The prelude is at most 65,536 bytes and contains 1–256 distinct entries. Profile
IDs are at most 96 bytes; codec IDs are at most 64 bytes. At those maxima, 256
identifier pairs and their length fields still fit below 64 KiB. This covers the
actual 132-entry inventory without reserving an unnecessary 1024-entry table;
it does not promise unlimited simultaneous algorithm versions.

The generic parameter ceiling is 65,536 bytes per instance. A known codec's exact
bound is stricter: the current ten never allocate that generic maximum.
Prelude framing, identifier bytes, parameter lengths/bytes and mask metadata all
count toward the **existing aggregate 8,388,608-byte PCND metadata ceiling**.
There is no additional allowance. Existing per-layer 256-filter, canvas, layer,
staged/resident tile, block codec and A8 aggregate limits remain in force.

After the existing outer SHA check, validate the bounded borrowed prelude before
retaining document profiles, masks, layers or inflated tiles. Reader capability
validation resolves every profile/codec pair, including inactive intentions.
Unknown required capabilities return CapabilityUnsupported. Malformed framing,
noncanonical counts, duplicate or unused declarations, invalid typed parameters,
out-of-range indexes and trailing bytes return CodecFailed. Caller quota denials
remain ResourceLimit. All traversal and admission are cooperatively cancellable.

Continue the current two-stage reader: scan bounded metadata and borrowed tile
extents, admit exact retained and temporary control/profile/mask allocations,
validate canonical final inventory, then inflate tiles. Wire-byte caps do not
substitute for expanded storage quotes. In particular, prelude identifiers should
be borrowed for validated lookup rather than allocated through generic document
string decoding. No cancellation or quota failure may publish partial state.

Keep the existing public `Header` struct compatible. Add a separate API:

```rust
catalog_manifest(bytes: &[u8]) -> Result<Vec<Requirement>>
catalog_manifest_checked(
    bytes: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<Requirement>>
```

The convenience wrapper supplies a no-op callback. The checked variant checks
cancellation during authentication and bounded manifest traversal; the adopting
reader uses that checked path with its own job callback.

`Requirement` exposes the actual profile ID and transport codec ID from the
bounded authenticated prelude. The helper is assessment, not adoption authority:
structurally valid unknown requirements may be reported; the adopting reader must
resolve and support them all. The returned bounded storage is not a tile decode.
PCND1–5 return no catalog requirements. Header version6 assessment advertises
`layer-filter-catalog.v1`; `require_supported` must require that framing feature,
while the byte reader independently validates the actual manifest and instances.
A caller-created Header can never authorize unvalidated source bytes.

## Semantic identity and minimal legacy output

Canonical semantic parameter bytes are defined **independently of transport**.
For the current nine profiles they equal the bytes in the table. Accepting a
future alternative lossless codec must not change the semantic hash when it
constructs identical typed parameters. Transport codec IDs, file compression,
JSON formatting, allocation identities and derived LUTs are excluded.

A stack containing any catalog-only profile uses domain
`paintcraft.filter-stack/v3\0`, followed by count u16BE and these ordered records:

```text
filter_uuid: 16 bytes
enabled: u8
numerical_profile_id_length: u16BE
numerical_profile_id: ASCII bytes
semantic_parameter_length: u32BE
canonical_semantic_parameter_bytes
mask_presence: u8
if mask present:
    mask_enabled: u8
    canonical_A8_digest: 32 bytes
```

Old-only stacks retain byte-exact v1 hashes when unmasked and v2 when any stored
mask exists. Catalog v3 always includes mask presence for every instance. Stable
IDs, order, disabled/default parameters and retained mask intent remain semantic.
Cold history keeps typed FilterStacks; no JSON/archive codec version is invented.
Any future owned parameter payload needs explicit admission and unique hot/cold
storage accounting rather than being hidden behind inline instance size.

Writers select PCND6 only when at least one stored catalog-only profile requires
it, including disabled or identity Levels, Exposure and Curves. Old-only documents retain their exact
minimal PCND1–5 output, even if they arrived in a catalog request. Reject PCND6
with only legacy-expressible intentions after metadata inventory and before tile
inflation. Removing the last catalog-only profile restores the applicable legacy
writer and old stack-hash domain without dropping editable state.

## Integration and qualification gates

The new reader/writer must share manifest parsing, exact profile codecs, raw and
selected-compact metadata accounting, and early unknown-profile refusal. Registry
advertisement follows complete integration. Existing export Worker4/5 and preview
source2/3 remain closed to PCND6 until new prepared-program admission profiles are
qualified. A parser's ability to read6 cannot grant an unpriced evaluator access.

Old public `Descriptor::from_value` and frozen batch/inspect/result schema unions
remain closed. New catalog-aware construction and negotiation must be explicit;
inspection selects a new compatible profile before legacy branches. New result
versions are required when output shape or vocabulary changes, not per algorithm.
No existing JSON schema file silently acquires a larger descriptor union.

Required proof includes independent Python fixtures and raw/evaluated pixel
oracles; authenticated unknown/disabled/duplicate/unused/index/length negatives;
exact parameter and metadata boundaries; admission/cancellation/lease release;
byte-identical PCND1–5 regressions; codec-independent semantic hashes; typed
hot/cold history and Undo/Redo; genuine closed CLI/MCP schema checks; and complete
renderer, prepared-resource, preview/export, native and actual browser lifecycle
qualification. A wire contract alone establishes none of those capabilities.
