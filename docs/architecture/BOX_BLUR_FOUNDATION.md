# Box blur kernel and integration boundary

This records the qualified scalar foundation now used by the seventh editable
filter, Box Blur. Persistent descriptors, regional rendering, PCND5, typed
commands and Native/Web host controls are implemented. Local tests and required
Metal presentation pass. Actual production Worker3, scoped Box Studio and MAIN
DDS/MSP export5 pass their finite appearance, consent and replay checks.
See [the current integration](SPATIAL_FILTER_INTEGRATION.md). The six
[point profiles](POINT_FILTER_PROFILES.md) keep their original contracts.

## Numerical policy

`BoxBlurV1` accepts integer radius 0 through 32, with default 2. Its fixed square
window has `(2r + 1)^2` taps. Clamp each tap to the **true finite source edge**,
including repeated edge samples. A tile, output rectangle or stripe is not an
image boundary. Output has the same finite extent as the source.

Radius zero copies straight RGBA bytes exactly, including hidden RGB. At positive
radius, let `Sa` be the sum of source alpha and `Sc` the sum of source channel
times source alpha over all taps. Compute:

- Alpha: nearest integer to `Sa / number_of_taps`.
- Each RGB channel: nearest integer to `Sc / Sa` when `Sa > 0`, otherwise zero.
- Nonnegative half ties round upward. Do not round between horizontal and
  vertical sums. A positive `Sa` can round to byte alpha zero while retaining
  nonzero hidden RGB; do not discard that RGB.

These are encoded RGB bytes, with no linear-light or ICC conversion. This
independently specified policy intentionally differs from Palette's per-axis
rounded [box passes](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/neighborhood.rs). The pinned upstream source review is evidence of design
intent, not byte parity or execution of Palette.

## Coverage after the candidate

First round the entire unmasked candidate. Then mix it **once** with this
stage's incoming pixel. Coverage zero returns incoming RGBA exactly; coverage
255 returns candidate RGBA exactly. At intermediate coverage `m`, incoming alpha
`Ap` and candidate alpha `Aq`, let `W = (255-m)Ap + mAq`.

- Alpha is nearest integer to `W / 255`.
- RGB is nearest integer to `((255-m)Cp*Ap + mCq*Aq) / W` when `W > 0`.
- When `W = 0`, interpolate hidden RGB directly as
  `((255-m)Cp + mCq) / 255`, rounded once.

The alpha-preserving point coverage function cannot define this operation.
Layer opacity and source-over composition follow all filter stages.

## Checked regions and caller-owned work

`BoxBlurPlan::new` validates nonempty finite source and output rectangles,
requires the output to lie inside the source and computes its clipped halo.
Input is exactly that tightly packed halo. Input/output lengths must match the
quote; scratch must cover the quoted prefix. Validation precedes callbacks and
writes. Checked signed coordinates support negative origins without wrapping.

The two sliding passes store unrounded horizontal sums in caller-owned `u64`
scratch. The kernel allocates nothing. Its quote includes simultaneous input,
output and used scratch bytes, plus a deterministic source/aggregate visit
count. The caller must separately charge actual allocation capacities, stored
source pixels, masks, prefix stages and concurrent jobs. A quote alone does not
admit a job. Partitioning the output bounds scratch without changing edge rules.

`required_input` and `box_affected_output` are separate read-dependency and dirty
propagation operations. Both conservatively expand by the radius and clip to
the finite source for this symmetric kernel. Cancellation checks occur during
both passes; failed private output and scratch must be discarded. There is no
millisecond cancellation guarantee.

## Integrated regional boundary

The renderer evaluates ordered prefix regions with cumulative true-source halos,
admitted buffers and shared per-job work. CPU composition, GPU uploads, display
reduction and host visual/copy callbacks use the evaluated appearance. Native
admission is installed on the actual executing thread; a separate codec process
does not inherit it. Production Worker3, scoped Box Studio and MAIN DDS/MSP export5 are qualified
separately with finite appearance, consent and replay controls.

PCND5 is conditional on stored Box, including disabled/radius zero, and adds
algorithm tag 7 plus one radius byte. Old PCND3/4 still refuse tag 7: their registry
and byte profiles are unchanged. PCND5 header grammar requires point/mask/spatial
capabilities, while actual result features include masks only when stored.
The [format contract](../PCND_FORMAT.md#version5-persistent-spatial-intentions)
and [automation contract](../FILTER_COMMANDS.md) define the closed profiles.

The independent Python/Fraction oracle under
`crates/services/paintcraft-effects/tests/box-fixtures` remains separate from
product output and host tests. The historical kernel checkpoint below is not an
actual browser or whole-catalog proof. Current integration qualification is
summarized in [SPATIAL_FILTER_INTEGRATION](SPATIAL_FILTER_INTEGRATION.md#current-qualification).

## Local foundation qualification

Core passes all 62 unit tests. Effects passes all 23 tests, including nine new
geometry/storage/cancellation groups and two groups comparing actual kernel
outputs with the independent Fraction oracle. Those comparisons consume 15
literal planes, 24 partition outputs, 45 mask literals, 12 rounded blur/mask
planes and 54,726 records across three frozen hash streams. Strict Core/Effects
checks and Wasm compilation in the real web dependency context pass. All owned
source files remain below 800 lines; no new dependency is required.

The retained evidence is `docs/validation/box-blur-foundation/`. Failed test-helper
coordinate arithmetic, a test iterator lint and an isolated Wasm invocation
without the web app's UUID randomness feature are retained alongside corrected
checks. None established a kernel numerical failure. That historical kernel checkpoint alone had no actual blur UI claim. Current
local host controls and the scoped production Box Studio/Worker3 flow now pass.
MAIN DDS/MSP export5 also pass actual consent/replay checks. No GPU blur shader,
performance or external-application parity is claimed.
