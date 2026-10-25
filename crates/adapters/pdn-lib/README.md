# PDN3 inspection and typed-raster conversion

This private Rust crate provides separate header inspection and narrow typed-raster decoding APIs. `inspect` returns inert metadata hints and borrowed input spans; inspection alone does not validate the object graph or pixels. `decode_layers` validates the supported inert graph and deferred buffers described below. No preserving writer is provided, and compatibility with arbitrary Paint.NET versions is not implied.

The verified envelope starts with `PDN3`, followed by a three-byte little-endian UTF-8 XML byte length. The XML root is `pdnImage`; recognized hints are `width`, `height`, `layers`, and optional `savedWithVersion`. After the XML, the `00 01` deferred-data marker is separate from the following NRBF serialization header. A `1f 8b` prefix identifies the historical gzip branch. Header hints must eventually be compared with the actual decoded graph before document creation or pixel allocation.

Primary evidence:

- Historical original Paint.NET `Document.cs`, preserved at [OpenPDN commit cca476b0df2a2f70996e6b9486ec45327631568c](https://github.com/rivy/OpenPDN/blob/cca476b0df2a2f70996e6b9486ec45327631568c/src/Data/Document.cs): `MagicBytes`, `FromStream`, `SaveToStream`, and `PrepareHeader`. This establishes the historical framing and attribute names. No implementation text or external fixture was copied into this crate.
- Current official [Document.FromStream](https://paintdotnet.github.io/apidocs/api/PaintDotNet.Document.FromStream.html) and [Header](https://paintdotnet.github.io/apidocs/api/PaintDotNet.Document.Header.html) API docs identify the document interface, but do not specify modern on-disk object layouts. They do not qualify a modern graph decoder.
- Microsoft's [MS-NRBF SerializationHeaderRecord](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-nrbf/a7e578d3-400a-4249-9424-7529d10d1b3c) defines the 17-byte serialization header and protocol version 1.0. Only these inert fields are inspected; no objects, constructors, type loading, CLR or BinaryFormatter are executed.
- [RFC 1952](https://www.rfc-editor.org/rfc/rfc1952) defines gzip framing. The inspector bounds optional header fields and requires space for compressed bytes and trailer, but does not validate compressed data, either CRC, concatenated members or decompressed NRBF.
- [XML 1.0 Fifth Edition](https://www.w3.org/TR/xml/) supplies character/name syntax. `quick-xml` parses structure and attributes; document types, external/custom entities and processing instructions are refused. No referenced path, URL, thumbnail or metadata payload is opened.

Injected limits cover source/header bytes, XML depth/elements/attributes, attribute size, dimensions, pixel count and declared layers. Cancellation runs during header processing and bounded gzip header scanning. The source remains borrowed; no full input or thumbnail copy is created. Application version text remains an untrusted string, not a compatibility assertion. Unknown root attributes and child XML are flagged and retained only in the original borrowed XML span.

Tests author synthetic envelopes directly from the documented framing, including Unicode metadata, complete NRBF headers, malformed XML, truncations, hostile entities, unknown variants, resource limits and cancellation. These synthetic bodies are deliberately not claims of valid editable Paint.NET documents. The typed graph/pixel decoder below now supplies a scoped PDN-02/03 implementation. Untouched files from pinned Paint.NET builds remain necessary for broader native qualification; original-application reopen remains mandatory for any future writer.

## Typed bitmap-layer conversion

`decode_layers` now supports a narrow deferred PDN3 bitmap graph: Document,
LayerList, BitmapLayer and its declared property/surface/memory records, system
Version and string-pair metadata, and Normal blend mode only. It reconstructs
layer order, Unicode names, visibility, opacity, background-role flags and full
canvas-sized tight-stride RGBA8 surfaces. Unknown classes, fields, methods,
libraries, blend modes, shared/subview buffers, padding, nondeferred pixels and
unsupported object types are refused rather than replaced with guessed defaults.

The NRBF parser bounds graph bytes, metadata capacity, object/record work, members,
array elements and nesting. It never loads types or executes constructors. Record
references and library bindings are validated after parsing; graph identity lookup
is bounded and does not recursively expand cycles. The exact MessageEnd position
locates the following deferred pixel blocks. Each buffer has its own graph
identity. Chunk indexes may arrive in any order, but duplicate/missing/out-of-range
indexes are rejected. Gzip headers, optional header CRC, data CRC, output size,
DEFLATE EOF and payload boundaries are checked. Raw and gzip chunks share the
same exact BGRA-to-RGBA conversion, preserving hidden RGB. Work yields at bounded
input/output/copy checkpoints.

XML canvas dimensions and layer count must agree with the graph. XML and graph
version values remain separate source claims because the independent writer's
graph templates retain a zero savedWith value. The returned mandatory
`MetadataUnretained` diagnostic requires explicit conversion consent: EXIF/custom
metadata, version/provenance and background lock-role semantics cannot be inferred
to survive in PaintCraft's current document model merely because raster properties
do. No preserving `.pdn` writer or opaque-source round trip is claimed.

The [fixtures](tests/fixtures/README.md) are produced by a pinned independent
writer using real Paint.NET-generated graph templates and original test pixels.
Tests compare all source properties and complete RGBA hashes, exercise raw and
out-of-order gzip chunks, every file truncation, unknown record/type/blend cases,
invalid graph references/null runs/depth, aggregate decoding limits, corrupt CRCs,
trailing data and cooperative cancellation. Strict Clippy and native/Wasm builds
qualify compilation only. Untouched native reference files, a broader Paint.NET
version corpus and original-application reopen/edit/resave tests remain required
before general fidelity or any writer promotion.
