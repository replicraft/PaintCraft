# PAINT-01: bounded ISO box and extent inspection

This private Rust crate implements the physical container boundary needed for future `.paint` qualification. It returns borrowed payloads and a bounded preorder tree with parent indices. It is **not** a `.paint` layer decoder or writer. A shared HEIF/MIAF brand cannot distinguish a Paint project from another ISO container; `ftyp` results remain inert brand hints.

Implemented: big-endian 32-bit and extended 64-bit box sizes, UUID user-type headers, top-level zero-size boxes extending through EOF, checked parent/sibling ranges, duplicate `ftyp` refusal, bounded compatible-brand lists, iterative traversal with a hard maximum of 64 levels, source/box/metadata limits and cooperative cancellation. Only top-level `meta` version 0 with zero flags, its `iprp`, and their `ipco` contents are traversed by default. Other payloads remain opaque. An explicit layout callback can qualify additional container prefixes later; it cannot bypass physical range checks.

The separate extent helper resolves only file-relative bytes within a caller-admitted data scope. It rejects overflow and out-of-scope references. Unordered extent validation rejects partial overlap; identical aliasing requires an explicit policy. It does not interpret `iloc` construction methods, item identities, external data references or decompression sizes. A future item parser must prove those before resolving pixels.

Primary references:

- Apple's [Atoms](https://developer.apple.com/documentation/quicktime-file-format/atoms) documentation establishes total-size accounting, extended sizes, big-endian fields, nested containers, unknown-payload skipping and top-level-only zero-size atoms. It explicitly identifies the structural relationship with ISO boxes while distinguishing full format semantics.
- Mozilla's [mp4parse-rust ISO parser](https://github.com/mozilla/mp4parse-rust/blob/master/mp4parse/src/lib.rs) independently implements ISO box/UUID framing. No implementation text is copied.
- W3C's [ISO BMFF byte stream note](https://www.w3.org/TR/mse-byte-stream-format-isobmff/) describes `ftyp` brands as capability constraints, rather than proof of any particular application document.
- [`setiri/mspaintlib`'s authored format observations](https://github.com/setiri/mspaintlib/blob/master/PAINT-FORMAT-SPEC.md) motivate the initial `meta`/`iprp`/`ipco` structural traversal. These are reverse-engineering evidence, not a Microsoft format specification or a qualification of current Paint files.

All regression bytes are authored synthetic structures. Tests exercise nesting, ancestry, unknown payloads, UUID/extended/to-EOF sizes, truncation, malformed scopes, limits, cancellation, checked extents and alias policies. No downloaded Paint sample or external pixel decoder is required. Source payloads are not copied. Metadata vectors use fallible, byte-admitted growth; fixed-depth traversal avoids recursive stack exhaustion. Sorting admitted extent metadata is bounded, with cancellation before/after sorting and between checks.

PAINT-02 remains the item/property graph and actual layer decoder. No names, visibility, opacity, channel order, overlay background or compression semantics are invented here. Original pinned MS Paint files and reopen tests remain mandatory for any future writer or fidelity claim.
