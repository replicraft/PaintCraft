# Color Overlay numerical foundation

Color Overlay now has a pure typed policy and scalar/row implementation. It is
not yet a stored document style, rendering stage or frontend control. No filter
catalog entry, worker capability, execution allowance or resource price is added.

## Exact policy

`paintcraft_core::types::styles::ColorOverlayRgbV1` stores RGB bytes, byte opacity
and a separate `EffectBlendMode`: Normal, Multiply or Screen. Effect blending uses
the layer's own incoming RGB; it does not use another layer's backdrop. The five
semantic bytes retain color and mode even at zero opacity. Unknown mode tags refuse.

For incoming channel `x`, effect channel `c` and opacity `t`, the exact candidate
numerator `b` is `255*c`, `x*c` or `255*(x+c)-x*c`, respectively. Each output channel
is `(255*x*(255-t) + b*t + 32512) / 65025` with integer division. There is one final
rounding, with no intermediate byte rounding. All intermediates fit `u32`.
Incoming alpha is copied exactly, including hidden RGB under alpha zero.

The Effects row helper checks matching lengths before callbacks or writes, checks
cancellation initially, before each chunk of at most 64 tuples, and finally.
Mid-row or final cancellation can leave private output partially or completely
written; callers must discard it. The source remains immutable. This helper
creates no allocation owner, counter or quota.

## Evidence and remaining work

An independent Python Fraction generator was frozen before the kernel. Its
portable [fixtures](../../crates/services/paintcraft-effects/tests/color-overlay-fixtures/README.md)
contain 6,912 RGBA vectors, 155 causal intermediate-rounding controls and 216
future-stage vectors. Current tests execute the overlay primitive in those stage
vectors; their later mask/opacity columns are expectations, not executed document
integration. Actual scalar and partitioned row results match both packed hashes.

Native Core and Effects library suites pass 102 and 85 tests, including six new
groups. Native all-target strict lint, Main/both-Worker Wasm strict lint, formatting
and source-length checks pass. Separate actual Wasm execution through the real
generated glue also passes all 6,912 scalar vectors, 216 primitive-stage inputs
and 155 single-round controls. It exercises 184,320 row calls by repeating the
existing independent literals across partitions, plus 15 cancellation cases,
six empty-row cancellations and 272 refusals. Repetition adds boundary coverage,
not independent numerical cases. Source and output guards remain intact.

All 119 defining source/config/harness inputs match before and after build and
execution. The actual module has nonshared memory, 2 MiB initially and 16 MiB
maximum, and only the real glue's externref initializer import. The first build
driver wrongly required Cargo's emitted root-package libraries to be under
`release/deps`; its failure and corrected fresh attempt remain separate. No kernel
change was required. This Node Wasm result does not establish browser UI, Worker
support, resource admission or physical memory use. The aggregate receipt is
`docs/validation/color-overlay-foundation/wasm/final-freeze.json`, with defining
sources and actual libraries preserved. Original sources,
oracle-first receipt and native gate logs are retained under
`docs/validation/color-overlay-foundation/`. The scalar baseline has no comparative
performance or RSS claim, and its source-action ledger has no adopted work price.

The verified Palette source interpolates Normal overlay RGB before its layer mask.
Its configured effect-blend dispatcher is absent from the pinned tree, so the
integer contract above is independently specified and does not claim upstream
floating-point or configured-mode parity.

The requested persistent model is a bounded ordered style stack with stable IDs,
not an extra numerical filter. Adding it still requires document/history semantics,
conditional project persistence, typed commands, measured allocation weights,
priced rendering, cache invalidation and frontend adoption. The intended first
stage is complete filters, enabled Color Overlays in order, own mask, layer
opacity, then layer blending; exterior style families need separate stage rules.
Existing project and worker profiles remain unchanged and grant no style support.
