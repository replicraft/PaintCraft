# Proposed point-profile oracle

These are **independent expected specimens only** for the proposed
[point-filter contracts](../../../../../docs/architecture/POINT_FILTER_PROFILES.md).
No product Core/Effects change, Rust test registration, renderer integration or
frontend completion is implied.

Run from the repository root, using stdlib Python only:

```sh
python3 crates/services/paintcraft-effects/tests/point-fixtures/generate.py
```

This recomputes every stream, checks literal and rational controls, and requires
byte-identical `oracle.json`. To intentionally regenerate after reviewing a
contract change, use `--write`. Neither mode imports PaintCraft or fetches tools.
The normal verification mode writes no files.

The fixture's generic `parameter` slot (null for Solarize) is oracle data,
not a proposed command descriptor shape.

`oracle.json` records six SHA-256 streams. Record ordering and byte framing are
specified beside each hash; concatenation has no separators or text encodings.
Unary inputs and parameters are implicit in that ordering. The chromatic
Threshold stream explicitly hashes `[level,R,G,B,output_gray]` byte records so
its selected boundary inputs are bound as well as their answers. Streams are
hashed incrementally and do not materialize whole images.

The full unary and gray domains are exhaustive for their stated parameters.
Chromatic Threshold brackets and Black & White color grids are disclosed finite
samples, not an exhaustive RGB-cube claim. Four alpha values include hidden RGB
under alpha zero. Literal controls, order and per-instance mask samples remain
readable JSON for future product tests.

Expected outputs use exact integers and nearest rounding with ties upward. A
second stdlib `Fraction` expression cross-checks selected domains. The generator
also checks invalid integer domains/types (including bools, floats and NaN).
Python `ValueError` is only oracle validation; it does not qualify future Rust
error codes, cancellation, allocation limits or atomic publication.

Pinned Palette source provides reference intent. These independently specified
integer profiles do not claim byte parity with its floating point implementation
or Photoshop, and no upstream source was copied into this generator.
