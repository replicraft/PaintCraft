# Bounded PAINT container inspection and observed-profile decoding

This private Rust crate provides physical container inspection and a separate observed-profile layered decoder. `inspect` returns borrowed payloads and a bounded preorder tree with parent indices; successful inspection alone is **not** layered decoding. A shared HEIF/MIAF brand cannot distinguish a Paint project from another ISO container; `ftyp` results remain inert brand hints. Use `decode_layers` only with its explicit semantic diagnostics below. No preserving writer is provided.

Implemented: big-endian 32-bit and extended 64-bit box sizes, UUID user-type headers, top-level zero-size boxes extending through EOF, checked parent/sibling ranges, duplicate `ftyp` refusal, bounded compatible-brand lists, iterative traversal with a hard maximum of 64 levels, source/box/metadata limits and cooperative cancellation. Only top-level `meta` version 0 with zero flags, its `iprp`, and their `ipco` contents are traversed by default. Other payloads remain opaque. An explicit layout callback can qualify additional container prefixes later; it cannot bypass physical range checks.

The separate extent helper resolves only file-relative bytes within a caller-admitted data scope. It rejects overflow and out-of-scope references. Unordered extent validation rejects partial overlap; identical aliasing requires an explicit policy. It does not interpret `iloc` construction methods, item identities, external data references or decompression sizes. A future item parser must prove those before resolving pixels.

Primary references:

- Apple's [Atoms](https://developer.apple.com/documentation/quicktime-file-format/atoms) documentation establishes total-size accounting, extended sizes, big-endian fields, nested containers, unknown-payload skipping and top-level-only zero-size atoms. It explicitly identifies the structural relationship with ISO boxes while distinguishing full format semantics.
- Mozilla's [mp4parse-rust ISO parser](https://github.com/mozilla/mp4parse-rust/blob/master/mp4parse/src/lib.rs) independently implements ISO box/UUID framing. No implementation text is copied.
- W3C's [ISO BMFF byte stream note](https://www.w3.org/TR/mse-byte-stream-format-isobmff/) describes `ftyp` brands as capability constraints, rather than proof of any particular application document.
- [`setiri/mspaintlib`'s authored format observations](https://github.com/setiri/mspaintlib/blob/master/PAINT-FORMAT-SPEC.md) motivate the initial `meta`/`iprp`/`ipco` structural traversal. These are reverse-engineering evidence, not a Microsoft format specification or a qualification of current Paint files.

All regression bytes are authored synthetic structures. Tests exercise nesting, ancestry, unknown payloads, UUID/extended/to-EOF sizes, truncation, malformed scopes, limits, cancellation, checked extents and alias policies. No downloaded Paint sample or external pixel decoder is required. Source payloads are not copied. Metadata vectors use fallible, byte-admitted growth; fixed-depth traversal avoids recursive stack exhaustion. Sorting admitted extent metadata is bounded, with cancellation before/after sorting and between checks.

The observed item/property/pixel subset of PAINT-02 is implemented below; broader metadata and native-build qualification remain unfinished. No names, visibility, opacity, channel order, overlay background or compression semantics are invented. Original pinned MS Paint files and reopen tests remain mandatory for any future writer or full fidelity claim.

## Observed-profile layered decoding

`decode_layers` now decodes the observed `mif1`/primary-`iovl`/`unci` subset into
format-owned DTOs. It reads actual `infe`, `iloc`, `ipma`, `dimg` and `iovl`
relationships, validates property references and every file/idat extent, and
requires one non-overlapping extent per item. Layers use raw DEFLATE, packed
unsigned RGBA8 components and declared full-range sRGB. Component positions come
from `cmpd` and `uncC`, rather than assuming byte order. Unknown metadata,
properties, relationships, protection, external references, compression, pixel
packing, precision and color configurations are refused.

The returned canvas fill retains all 16 bits. Item names remain literal item
names and are not asserted to be UI layer names. The returned mandatory
`LayerPropertiesUnverified` diagnostic records that visibility, opacity and layer
name extensions remain unqualified. A consumer must resolve those conversion
limits explicitly. This decoder does not enable preserving `.paint` Save, and
none of the resulting DTOs imply original-application writer qualification.

`DecodeLimits` bounds source/container metadata, items/layers, each dimension,
aggregate layer pixels and aggregate decoded output. All graph/geometry plans
are validated before decoding pixel buffers. DEFLATE input and output are
processed in bounded 64 KiB steps, with cooperative cancellation, exact output
length, stream EOF and no trailing bytes. Metadata vectors use fallible growth
under a conservative capacity allowance, including extent-validation scratch;
this is not an allocator/RSS qualification.

The [pinned independent sample](tests/fixtures/README.md) contains four 333 x 271
layers; complete decoded pixel hashes match a separately executed Python/zlib
oracle. Negative tests cover graph mutations, unknown properties, cycles,
aliases, metadata/data bounds, malformed DEFLATE, truncation, output limits and
cancellation. The sample does not establish support for unobserved Paint builds,
unknown layer properties or the writer exit gate.

## Internal fresh writer

The typed `encode_document` / `write_document_to` path now constructs the observed
`iovl`/raw-DEFLATE `unci` profile independently. It retains exact RGBA8 layers,
individual sizes, signed placement, literal item names and canvas fill16. Hidden
layers, non-full per-layer opacity and unsupported metadata are refused. It does
not claim UI layer-name semantics or preserving Save. Public writing remains
closed until actual Windows Paint qualification.

Owned fixtures were independently checked with Python structure/zlib and the
unchanged pinned C# reader compiled privately for net10. The C# reader retains
pixels/geometry but its high-level fill8 narrowing and UTF-8-name decoding
limitations remain explicit. See [writer provenance, commands and bounds](tests/fixtures/README.md).
