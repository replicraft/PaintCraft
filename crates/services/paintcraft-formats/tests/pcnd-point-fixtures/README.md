# Point-algorithm registry specimens

`generate.py` writes 29 PCND3/4 specimens using only Python's standard library.
It does not import or call PaintCraft. Twenty-three are expected to open; six
reject invalid scalar values or unknown algorithms, including disabled ones.

The source plane, explicit descriptors and IDs, full and fractional-mask
appearance, canonical stack identity and export-inventory identity are frozen in
`oracle.json`. Exact `Fraction` expressions compute candidates before mixing each
filter's coverage. The left source pixel alone receives coverage 128 in masked
specimens; the right remains outside the mask support. Alpha and hidden RGB are
preserved in evaluated planes; rendering normalizes fully transparent output.

Cases cover all four additional point profiles, parameter boundaries, disabled
intentions, PCND4 masks, ordering, all six algorithms together, retained original
ICC data and hidden RGB. A separate fixture directory under `paintcraft-effects`
contains the larger numerical oracle. These bytes specify expected local format
behavior; they do not establish original-application or GPU compatibility.

Regenerate from the repository root:

```sh
python3 crates/services/paintcraft-formats/tests/pcnd-point-fixtures/generate.py
```
