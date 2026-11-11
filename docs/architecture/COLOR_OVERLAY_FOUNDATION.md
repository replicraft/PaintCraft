# Color Overlay numerical foundation

Color Overlay has a typed numerical policy, ordered document intentions, typed
commands, atomic editing, history preservation and native project persistence.
Rendering and frontend integration are in progress. Existing worker profiles provide no style transport;
the numerical policy remains separate from the filter catalog.

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

The initial Core and Effects library checkpoint passed 102 and 85 tests, including six new
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

## Prepared arithmetic for the active integration

An additive prepared implementation stores three exact affine coefficient pairs
in 24 bytes. It preserves the scalar contract and accepts private in-place tuple
or byte rows of at most 64 pixels. Length refusal precedes callbacks and writes;
nonempty rows check cancellation before transformation and after it. Callers must
discard a canceled private output. The original scalar and row APIs are unchanged.

Pure row quotations report 4,096 formation/query work units plus 256 per pixel
and a 1,024-byte inline control envelope. These are conservative logical source
actions, not CPU instructions, measured memory or execution authority. A consumer
must pay in its original realm before formation and writes, retain its account,
and finish query Results before forming coefficients. Supported target layouts
are checked explicitly. Rendering adoption and its full pipeline admission remain
pending; this helper alone grants no stored-style support.

All 89 Effects library tests and native all-target/Wasm strict lint pass. Actual
Wasm execution matches all 6,912 independent scalar vectors and exercises 6,912
row calls across both private views, plus 66 cancellation cases and six oversized
row refusals. Empty rows, canaries, unchanged source and alpha-zero inputs pass.
The Wasm named control layout is 484 bytes within the declared envelope. The
original scalar/row conformance also passes in that same module. The second fresh
attempt corrects the diagnostic's interpretation of a raw Wasm signed `i64`
sentinel as an unsigned Rust return; its first failure remains preserved. Sources
match before/after build and execution. Receipts are under
`docs/validation/color-overlay-prepared/wasm/` and
`docs/validation/color-overlay-product-engine/effects-kernel/root-final-native/`.
There is no comparative speed, physical-memory or browser support claim.

## Document vocabulary and remaining integration

Each `Layer` now owns an ordered `StyleStack`, capped at 256 unique nonnil stable
UUIDs scoped to that layer. It preserves enabled state and strict Color Overlay
semantic bytes, validates order and identity, and reports actual retained vector
capacity. Empty stacks use a constant cached digest without allocation or hashing.
Disabled and zero-opacity entries still retain their full semantic intentions.

The closed `paintcraft.batch/layer-styles-v1` command profile describes Add,
Update, SetEnabled, Move, Duplicate and Remove with explicit style references.
Existing command profiles remain closed. History preserves ordered intentions
through cold compression, spill, Undo and Redo while retaining source pixels.
The current Core, Document, Commands and History library suites pass 110, 58, 45
and 66 tests respectively; all-target strict lint and strict documentation checks
also pass. These model checks do not qualify rendering or frontend consumers.
Receipts are under
`docs/validation/color-overlay-product-model/root-leaf-validation/`.

Atomic editor transactions now preserve scoped aliases, exact NoOps, cancellation,
stale-request rejection and single-step Undo/Redo. Styled source copies retain
their admission through staging, including owned paste previews. Clear Canvas
removes stored styles even when the unprocessed background already matches its
requested color. The complete Editor/Tools suites pass 382 tests without ignored
cases; their new style integration group contains six tests. The resource-control
layout and cancellation-phase expectations were updated from the added fields
and callback trace, retaining the original limits.

Native PCND persistence selects version 9 only for nonempty style intentions,
including disabled and zero-opacity entries. It retains the complete version 8
grammar and requires `layer-styles.v1`. After each layer's own mask and before
its tile records, it stores a big-endian 16-bit style count and ordered 23-byte
records: UUID, enabled flag, closed style tag and five semantic parameter bytes.
Unknown tags, modes, nil/duplicate IDs and excessive counts refuse. Empty stacks
preserve the previous minimum versions and exact legacy specimens. Save preflight
includes all forced version 9 framing and style bytes. Retained vector payload
must fit its wire contribution, so the aggregate metadata bound also dominates
stored style payloads on supported target layouts.

The full Formats suite passes 1,056 tests with 28 existing ignored cases, including
three independently authored PCND9 specimens and exact/one-byte-over save limits.
Native all-target strict lint, formatting and source-length checks pass. Receipts
are under `docs/validation/color-overlay-product-model/root-expanded-checkpoint-1/`
and `docs/validation/color-overlay-product-engine/codec/root-full-formats-attempt-2/`.
This is native library qualification, not actual Wasm project decoding or app UI
acceptance. Existing layered foreign writers refuse styles; an approved appearance
copy still needs its compound export integration.

Browser host adoption and tool-preview guards explicitly refuse stored styles;
the old Source7/8 routes do not gain style support. The paid Source8 root walk
rejects nonempty intentions before source encoding, including disabled and
zero-opacity entries. Its single-layer shortcut requires an empty style stack.
The containing empty `Layer` now measures 408 bytes natively and 344 in Wasm.
Source8 named assembly controls are 2,976/2,524 bytes within the unchanged
4,096-byte slot; fresh map payload and work use the larger target stride. The
64 MiB metadata and 268,435,456 work ceilings are unchanged. Native Main and
Worker suites pass, as do native all-target and Main/both-Worker Wasm strict
lint. These are host/compilation checks; actual rebuilt Wasm service execution
and browser artifact qualification remain separate. Layout and strict-check
receipts are under `docs/validation/color-overlay-root-web/`.

Integration still needs priced rendering, cache invalidation and frontend
adoption. The intended first stage is complete filters, enabled Color Overlays
in order, own mask, layer opacity, then layer blending; exterior style families
need separate stage rules. Existing worker profiles grant no style
support. The current integration evidence is recorded in
[LAYER_BLEND_FOUNDATION.md](LAYER_BLEND_FOUNDATION.md#additional-portable-foundations).
