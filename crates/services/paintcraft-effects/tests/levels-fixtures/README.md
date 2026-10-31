# Exact Levels numerical specimens

This is an independent numerical foundation for the proposed complete
`LevelsRgbV1`, not a registered Descriptor, persisted algorithm, command or UI
feature. Existing seven-profile fixtures and bytes are unchanged. Gamma is real
user intent: the pinned Palette implementation has input/output endpoints plus
gamma; this corpus does not substitute a linear remap for full Levels.

The generator uses only Python stdlib integer powers, binary threshold search,
SHA-256 and JSON. It never imports product code, Palette, image libraries, native
binaries or FFI. It is deliberately separate from the product's proposed fixed
multiword monotone-sweep LUT builder. Exact power inequalities define expected
bytes; agreement with upstream floating `powf` is not a byte-parity claim.

Run `python3 generate.py` to verify `oracle.json` byte for byte. Use `--write`
only after reviewing an intentional contract change. No large binary streams
are retained: bounded stream counts, byte framing and SHA-256 are frozen in JSON.

## Contract

Parameters are ordered `[input_black,input_white,gamma_hundredths,output_black,
output_white]`. Input black is 0–254, input white 1–255, with strict black<white.
Gamma is an integer 10–999 representing the exact rational g/100. Output endpoints
are independently 0–255, including equal and inverted. Defaults are 0/255/100/0/255.
The mathematical candidate for encoded channel v is:

```
d = input_white - input_black
q = clamp(v - input_black, 0, d)
u = (q/d)^(100/g)
y = output_black + (output_white-output_black)*u
```

Round y once, nearest with ties upward. Do not quantize u or a normalized byte
before final rounding. Compare u with a half-byte threshold a/b using exact
`q^p*b^r` versus `d^p*a^r`, where p/r is 100/g reduced by its GCD. Each final
byte is independently binary-searched; no monotone sweep or approximate root
is shared with the product. Inverted output uses the reversed inequality while
an exact final tie still rounds upward.

Gamma 100 is exact rational linear mapping. Full defaults preserve every RGB
byte. Input clamping maps to exact output endpoints. Equal endpoints produce
constant RGB. Output 255→0 with default input/gamma is exact inversion. Invalid
parameters are refused, not swapped or clamped; Python booleans/floats are not
integers for the oracle's domain validator.

The candidate acts on each incoming RGB byte independently, including hidden
RGB at alpha 0. Alpha is byte-exact. Future point masks mix the rounded candidate
with this stage's incoming RGB once using integer coverage, leaving alpha exact;
coverage 0/255 return incoming/candidate exactly. This is the existing point mask
rule, not the alpha-changing spatial rule. These RGBA/mask specimens are future
integration expectations; the current parameter foundation registers no filter.

## Bounded corpus and record order

All channel streams use exactly eight bytes per record:
`u8 ib, u8 iw, u16le gamma, u8 ob, u8 ow, u8 input, u8 expected`.
The source generator defines the full deterministic loop order.

- `every-gamma`: all 990 gamma values, three input/output endpoint sets and 12
  input samples spanning clamp edges, dark/mid/high values; 35,640 records.
- `all-byte-cases`: six input intervals, 11 gammas, four output pairs, all 256
  inputs; 264 parameter sets and 67,584 records.
- `all-input-interval-edges`: all 32,640 legal input intervals at gamma 100,
  output 19→237, deduplicated clamp-adjacent/interior values; 226,444 records.
- Ten full 256-byte LUT literals cover identity, inversion, compression, normal
  and inverted half ties, gamma square/root/min/max, and constant output.
- Six explicit half-tie literals include nonneutral normal/inverted examples:
  input 0…4/value 1/gamma 200/output 0…255 and 255…0 both yield 128;
  input 0…2/value 1/gamma 50/output 0…2 yields 1 and 2…0 yields 2.
- 200 literal RGBA/mask cases span four colored/hidden/alpha tuples and coverage
  0/1/128/254/255. A separate 1,536-record/6,144-byte RGBA stream covers every
  alpha at coverage 0/128/255 for two nonneutral normal/inverted parameter sets.
- Fourteen invalid specimens include endpoint/domain relationships and strict
  type failures. Core tests separately exercise its typed i16 constructor bounds.

A product oracle consumer should construct one admitted LUT per parameter set,
not rebuild it for each selected byte. The fixture itself makes no runtime,
latency or allocator claim. Exact source preservation, stack order, canceled
publication, history, PCND/automation and actual UI remain integration gates.
