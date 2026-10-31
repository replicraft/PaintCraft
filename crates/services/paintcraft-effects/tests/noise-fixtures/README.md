# Independent finite Add Noise mathematics

These 4,380 rows and 26,280 mask candidates are independent mathematical specimens,
not product-generated goldens or a software-libm intermediate-bit oracle. Uniform
uses exact Fraction arithmetic. Gaussian uses Decimal log/sqrt, independent
Machin π/cosine series and exact binary64 TAU as a rational, without intermediate
binary64 rounding. Both 96/160-digit computations (+20 guard) agree on the frozen
bytes; this is not a certified interval or global real-rounding proof.

Run `PYTHONDONTWRITEBYTECODE=1 python3 generate.py --check` from this folder to
reproduce every specimen. Only `generate.py` is the entrypoint. It imports local
stdlib-only `reference.py` and `boundary_reference.py`, never a product/library
kernel or ignored validation directory. The local reference is a byte-exact copy
of the independent original generator; the boundary copy changes only its local
reference filename. The generator authenticates both complete original mathematical
outputs by SHA256 before compacting them. Historical original oracles and sources
remain immutable; their hashes are retained in this portable manifest.

The first 4,320 records retain original order: 12 coordinate cases × six RGB tuples
× three seeds × five amounts × mono/color × Uniform/Gaussian. The final 60 retain
independently inverted-hash boundary cases (first draw 0/MAX, second draw 2^30).
Both distributions include alpha 0/1/127/128/254/255. All rows retain the six
coverage expectations 0/1/63/127/254/255, applied once after candidate rounding.
Coordinates are source-local; layer placement never enters the hash. Second
Gaussian draw mixes the first word; no independently reseeded stream is used.

`oracle.json` intentionally uses compact machine JSON. `reference_rgba` is finite
mathematical byte agreement evidence; `six_mask_rgba` is independent integer
coverage. Whole packed streams and original base/boundary streams are hash-bound.
Software-binary64 reference-graph observations and actual kernel native/Wasm
comparisons belong to separate runtime receipts. No full state-space, upstream
float byte-parity, statistical, timing or memory-bound claim follows.
