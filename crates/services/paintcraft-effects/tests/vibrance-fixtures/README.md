# Independent Vibrance fixtures

The generator uses Python standard-library `Fraction` arithmetic for explicit
RGB to HSL conversion, saturation adjustment and separate hue-sector HSL to RGB
reconstruction. It imports no PaintCraft code or simplified product formula.

Reproduce from the repository root:

```sh
python3 crates/services/paintcraft-effects/tests/vibrance-fixtures/generate.py --check
```

Palette's [point evaluator](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L255)
establishes the feature intent. This independent exact policy does not promise
equality with upstream floating-point output or proprietary Vibrance algorithms.
The [numerical contract](../../../../../docs/architecture/VIBRANCE_FILTER_FOUNDATION.md)
specifies the candidate and mask rounding boundaries.

The frozen corpus contains 5,929 RGB/amount samples, 768 grayscale controls,
46 exact half-channel ties and 4,860 RGBA/mask records. Samples include 539
distinct RGB tuples, eleven signed amounts, named primaries/secondaries,
max/min ties, near grays and 512 deterministic LCG samples. Alpha and coverage
use six representative bytes, including zero/full; this is not exhaustive over
all RGB, amount and alpha combinations.

Round the exact clamped candidate once, nearest with halves upward. Apply the
existing A8 blend once to that rounded candidate as a separate stage. Alpha is
unchanged, including when hidden RGB is transformed under alpha zero.

`oracle.json` SHA256:
`447b959c646862afa28bb1212d809794b5a65636777ac108863c7e04bdec8dec`.
The generator is an exact copy of the independently prepared source under
`docs/validation/vibrance-oracle/`; its SHA256 is
`c77dd70bbea25ea7eacad1db4bceaedb07478ac9272af0993c29c6046c09f30d`.
Planning and arithmetic-bound experiments remain separate from this oracle.
