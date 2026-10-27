# Native sRGB8 to DDS linear8 preparation

`generate.py` authors `linear8.json` using exact integer comparisons, then checks
all 256 entries independently using Python Decimal at 120-digit precision.
The byte table SHA256 is
`c16a22cd55eb77171a0645075ae171e6a8544bcee84283337b583c656cd4baec`.
The production lookup is checked against all entries by the DDS export tests;
there is no runtime `powf`, Python dependency or build-time generation.

The transfer definition is [W3C CSS Color 4, section 10.2, pinned 2026-10-26](https://www.w3.org/TR/2026/CRD-css-color-4-20261026/#predefined-sRGB).
Native default-v1 rendering produces straight RGBA8 encoded sRGB. DDS sRGB output
keeps that private composite unchanged. Linear output uses the same sRGB
primaries and D65 white, applies the inverse transfer and quantizes to unsigned
linear8, nearest with ties up. Alpha is never passed through the RGB curve.
This is conversion, not changing a header while leaving encoded sRGB samples.

For integer input `s <= 10`, the scaled linear value is `25*s/323`. For the
remaining codes it is `255*((200*s+2805)/53805)^(12/5)`. Comparing each positive
half-integer rounding threshold can be reduced to arbitrary-precision integer
fifth-power comparisons. Every threshold and exactness comparison is decided
without floating point. Decimal provides an additional independent check.

The only codes whose continuous linear values are exactly represented are 0
and 255. `dds-srgb-linear8-quantization` is reported if any RGB sample uses a
nonendpoint value. This means a difference from the exact continuous color,
not merely a changed byte or failed 8-bit round-trip. Endpoint-only pictures do
not acquire fictitious precision loss. Examples: 128→55, 188→128, 255→255.

The facade performs one render after resource admission, converts only the
private composite in place and moves the same admitted texture plan to the
existing encoder. The texture plan already prices that one RGBA buffer. Source
resident tile payloads are reported separately; bounded facade metadata adds
2 KiB. This is logical payload accounting, not a process RSS ceiling. The host
retains its resource lease and owns source/destination/session guards through
publication. Prepared output includes the actual losses, transfer receipt,
source state and immutable DDS plan; generated mip levels are explicit derived
data, not a claim to preserve custom source mips or discard native level zero.

`Document` has no retained ICC/high-depth field to assign or drop here. Common
flatten/hidden-RGB behavior comes from the existing flat-export compositor.
Block compression includes possible color and alpha quantization; wholly opaque
BC1 is enforced. Unsupported source alpha is refused, never silently matted.
The old generic DDS flat path still emits its original single-level RGBA8 bytes.

Reproduce into a fresh file (existing evidence is never replaced):

```sh
python3 generate.py /absolute/new/linear8.json
cargo test -p paintcraft-formats --locked --offline --lib export::dds::tests
```

The seven focused facade tests include the complete transfer table, exact
sRGB/default byte parity, linear pixels and unchanged alpha, resource denial
before rendering, one render/one plan inventory, missing actual loss consent,
derived mip information, source/session immutability and cancellation. Separate
backend tests and all 96 independently measured DDS specimens must continue to
pass after the internal admitted-plan move refactor. No frontend or registry
promotion follows from this prerequisite.
