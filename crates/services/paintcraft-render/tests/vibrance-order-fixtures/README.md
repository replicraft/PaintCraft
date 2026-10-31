# Independent ordered Vibrance fixtures

Independent expected values; product runtime qualification is recorded separately. Run with
`PYTHONDONTWRITEBYTECODE=1 python3 crates/services/paintcraft-render/tests/vibrance-order-fixtures/generate.py --check`.

Twelve ordered cases reuse three frozen independent Python references: Fraction
HSL Vibrance, Gaussian-elimination/Horner Curves and direct full-square Fraction
Box sampling. No product kernel/FFI is imported. References and hashes are in the
oracle; original references and their oracles are unchanged.

Curves uses free nonlinear outputs [23,9,240,73,201], rather than assuming the
inverse curve establishes noncommutation. Vibrance amount is +100; Box radius1.
Both orderings are evaluated for each family at per-stage masks0/127/255, with
assertions that each nonzero pair differs. Point candidate rounding precedes
one mask interpolation; Box uses the existing independent premultiplied spatial
mix exactly once against that stage's incoming prefix. Alpha0 hidden RGB remains
in point expectations, while Box deliberately follows premultiplied sampling.

Each record contains source, all full stage prefixes, final bytes/hash and signed
global partition coordinates. The 131×2 source crosses local32/64/128 boundaries;
its negative origin is [-67,-5]. Box samples the true full finite source with CLAMP.
Partition and interior-ROI expected arrays are crops of the global staged output,
not reblurred or clamped at ROI edges. These finite literals can test product
partitioning but do not independently prove a product scheduler/admission path.

No runtime, source publication, pricing, cancellation, whole-catalog parity or
new feature support is claimed.

Clean-checkout regeneration imports only the tracked Effects fixture generators
under `tests/{vibrance,curves,box}-fixtures/generate.py`. The original oracle's
`references` retains its historical ignored Vibrance path as a provenance label
only; portable_path resolves the byte-identical tracked file. That ignored file
is never read. The portable manifest records actual source locations and hashes.
