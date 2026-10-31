# Proposed Exposure sRGB oracle

This new, independent standard-library Python generator specifies a numerical
foundation. It does not register a Descriptor, implement a PaintCraft filter,
change old schemas or qualify any frontend. Historical Levels and point fixtures
remain untouched. No upstream implementation is imported or ported.

Proposed `exposure-srgb-v1` intent is one **strict integer**
`stops_hundredths` from −800 through 800, default 0. It represents exact EV
`s/100`. The candidate decodes normalized sRGB, multiplies linear light by
`2^(s/100)`, clamps to `[0,1]`, encodes sRGB, then rounds once to the nearest
RGB8 byte with half ties upward. Zero EV is exact identity. Alpha is unchanged;
hidden RGB is transformed even when alpha is zero. The existing point mask
mix applies once, in encoded RGB, after the candidate; this is not a spatial or
alpha-changing mask contract. This is not a Palette floating-point byte-parity,
Photoshop, active ICC conversion or full-catalog compatibility promise.

The pinned intent is backed by
`docs/validation/palette-source/aefc84fe9233e66344147078de866860326d8cfa/`:
`crates/palette-core/src/document.rs:824–827` declares linear-light Exposure;
`processing.rs:499–502` supplies EV controls −8..8 in steps of 0.01;
`processing/point.rs:6–19,234–239` defines the transfer and exposure application.
The author inventory lists Exposure under Adjustments in
`docs/temp/IMPLEMENTED_FUNCTIONS.md:331`. Those upstream facts do not define
PaintCraft's independently proposed exact-integer implementation.

## Exact classification

Let `D(x)` be sRGB decode: `x/12.92` when `x <= 0.04045`, otherwise
`((x+0.055)/1.055)^(12/5)`. Every decimal here denotes its exact rational value.
For input byte `v` and output boundary `j+0.5`, candidate rounding crosses that
boundary exactly when

```
D(v/255) * 2^(s/100) >= D((2*j+1)/510),  j = 0..254.
```

Equality crosses upward. Raising both nonnegative sides to 100 yields rational
powers: the linear decode expression has exponent 100 and the nonlinear base
has exponent 240. Python `Fraction` derives the rationals directly from the
transfer constants. Integer cross-products and signed shifts by `s` perform the
comparison. There is no `float`, `powf`, decimal approximation, interpolated
transfer table or product arithmetic module supplying expected bytes.

The standardized transfer's two knee constants are not perfect mathematical
inverses. The closest output half-byte thresholds are `9.5/255` and `10.5/255`;
both lie outside that tiny discontinuity. The generator proves their ordering
and inverse-branch separation with rational inequalities and integer powers.
It independently derives and checks all 256 zero-EV identities.

## Finite corpus and resources

The full corpus covers **1,601 complete 256-byte LUTs: 409,856 channel results**.
Record order is EV ascending −800..800, then channel ascending 0..255.
`oracle.json` freezes each whole-LUT SHA256, selected literal samples and a
SHA256 over the concatenated complete LUTs in that order. It also contains
literal exact half ties, knee/clipping controls, alpha/hidden-RGB/mask controls,
and invalid type/domain controls. Alongside 420 literal RGBA/mask records, 105
ordered streams cover **all 256 alpha values** for each selected EV/RGB/coverage
combination: 26,880 RGBA pixels, 107,520 output bytes. EV order is
`[-800,-100,-1,0,1,100,800]`, RGB order is
`[[0,10,11],[1,64,255],[128,16,200]]`, then coverage order is
`[0,1,128,254,255]`. Within each stream alpha runs from 0 through 255; output
RGBA bytes are concatenated in that order. Each stream and the ordered aggregate
have SHA256 hashes, so the JSON stays compact. These controls independently
check alpha preservation, hidden RGB, zero-coverage identity, EV-zero identity
and candidate-once encoded point mixing. Hashes do not claim complete LUT payloads are
stored in JSON: they are independently reproducible from this generator.

Generation holds the 409,856 result bytes plus one input byte's 255 large-integer
comparison pairs; it does not retain a 256×255 cross-product matrix. Every EV
uses its own binary threshold classification. It does not copy a prospective
product monotone sweep or fixed-limb implementation. Python object overhead is
not claimed as a fixed RSS bound. The manifest is finite; no sampling replaces
the full input/parameter domain.

Reproduce the completed independent generation checkpoint:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B generate.py --write
PYTHONDONTWRITEBYTECODE=1 python3 -B generate.py
```

The second command requires byte-identical regeneration. The generator and
output hashes are recorded with the qualification evidence. The full corpus was generated and byte-identically verified in the checkpoint
below. These results qualify the independent oracle only. Code syntax can also
be checked without executing the generator or creating `__pycache__`.


## Independent generation checkpoint

Full generation and byte-identical verification passed locally on 2026-10-30.
The corpus covers 1,601 LUTs / 409,856 channels, ten literal half ties,
420 literal RGBA/mask controls and 26,880 all-alpha RGBA controls. These are
independent numerical oracle checks, not product compiler or frontend tests.

- `oracle.json` SHA256: `777c5c7a8306d7ce0ee1c6facfbe1f50aaca6bd20579969d17266c8b6a02fd0f`.
- Ordered complete LUT aggregate: `f2c073aea270bc6e4f7fa5491d32ad93fa5d9d9edc328d0f51cab86d8a0ef8da`.
- Ordered all-alpha RGBA aggregate: `4c56dea25674e05b1a10b59ff33f224c2c32ed6fea772184ebda31bd101abc3c`.

The first generator completed successfully, but `/usr/bin/time -l` subsequently
failed a sandbox-denied `sysctl kern.clockrate` and returned nonzero. Original
stdout and timing failure are preserved. A direct standard-library measurement
wrapper then generated the exact same output hash and verified byte-identical
regeneration. Its observed peak RSS was 37,486,592 and 38,240,256 bytes; those
measurements include Python and wrapper overhead and are not an algorithm RSS
limit. The second generation rewrote the same path before a preference to retain
it untouched arrived; retained first stdout proves byte identity, and the exact
matching bytes are copied into evidence. No product library was built or tested.

Evidence: `docs/validation/exposure-foundation/oracle/final-freeze.json`.
