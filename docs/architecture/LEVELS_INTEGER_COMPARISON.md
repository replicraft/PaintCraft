# Exact Levels integer-backend comparison

Status: historical isolated experiment. Its prototype is not the selected code;
the later [shared-engine implementation and selection](LEVELS_BACKEND_SELECTION.md)
has separate actual-artifact measurements. The complete numerical policy and
fixture bytes are unchanged. The explicit u32 reference API is retained.

The candidate replaces 320 u32 limbs/u64 intermediates with 160 u64 limbs/u128
intermediates, retaining 10,240-bit capacity. Five caller-owned integer slots and
the private 256-byte table remain bounded. Candidate and baseline have different
word-operation accounting; a work-count ratio is not a speedup measurement.

## Correctness and execution scope

Native qualification compares the actual linked Effects artifact, byte-identical
u32 source compiled beside the candidate, and the candidate itself. All three
match 329,668 independent channel records, ten full LUT literals, six half ties,
RGBA/mask and all-alpha controls. Candidate admission, cancellation, capacity,
publication and scratch reuse controls pass before timing. The source-compiled
u32 control helps distinguish backend effects from cross-crate inlining.

Chrome qualification compiles the frozen u32 control and u64 candidate against
actual Core types as standalone Wasm benchmark modules. It matches the same
channel/LUT/tie corpus and failure controls before timing; host mask/alpha checks
remain distinct from a production renderer or browser UI qualification.

Actual Core's UUID browser context retains wasm-bindgen transform imports in
untransformed modules. The experiment preserves that failed zero-import attempt,
then uses installed wasm-bindgen 0.2.129 and its real generated glue. The final
modules permit exactly the generated externref-table initialization import;
there are no handwritten host stubs or benchmark hooks in the production worker.
Both instances remain at 2 MiB linear memory with a 16 MiB maximum. Native
scratch is 6,696 bytes for both; Wasm scratch is 6,676 bytes for u32 and 6,696 for
u64. These values describe scratch/linear memory, not physical process RSS.

## Measured results

Apple M1 Pro arm64, Rust 1.99.0, 15 rotated samples per backend/case. Times are
per complete 256-byte table. Browser results use Chrome 156; fast cases batch
calls to reduce JS crossing overhead. The p95 is the maximum at this sample size.

| Case | Native u32 artifact median / p95 ms | Native u64 median / p95 ms | Chrome Wasm u32 median ms | Chrome Wasm u64 median ms |
| --- | ---: | ---: | ---: | ---: |
| Gamma 9.99, full endpoints | 15.674 / 25.592 | 2.231 / 3.836 | 15.860 | 6.665 |
| Gamma 9.97, full endpoints | 15.101 / 22.779 | 2.206 / 2.492 | 15.910 | 6.710 |
| Gamma 3.33, inverted/cropped endpoints | 2.052 / 4.245 | 0.356 / 0.532 | 1.985 | 1.055 |
| Gamma 0.50, full endpoints | 0.013334 / 0.015333 | 0.015792 / 0.017584 | 0.024297 | 0.041484 |
| Gamma 2.00, full endpoints | 0.014458 / 0.014708 | 0.016500 / 0.016708 | 0.024219 | 0.041875 |

The native source-compiled u32 control measures 15.659 ms median at gamma 9.99,
close to the linked artifact's 15.674 ms. Large-gamma gains therefore are not
explained by that cross-crate control alone. Small-power regressions are real
in these samples, so an unconditional replacement is not justified. Identity,
constant and single-step browser cases are near the measured clock floor;
their submicrosecond differences are not evidence for a backend choice.

These are local elapsed observations with a no-op cancellation callback, not
whole-machine isolation, other CPUs/browser engines, UI latency or throughput
qualification. A broader endpoint/gamma crossover study must justify any
selection rule. A production change still needs the same oracle/resource gates,
source-matched native/Wasm measurements and truthful work/byte quotes.

## Retained evidence

All raw samples, exact source copies, artifact hashes and failed attempts remain
under the ignored validation directory:

- `docs/validation/levels-optimization/final-freeze.json`:
  `c66750c679b405dfd4d1ed88d650b0d98ae57efdd36d85040afece351c6b813a`.
- `docs/validation/levels-optimization/wasm/final-freeze.json`:
  `34cc692e0654ac0ea5b94778232d124c3b7dbe0bd6bb7ce668789ddd5968d843`.
- Native `timing-summary.json` and Wasm `timing-summary.json`, plus each executed
  harness/source snapshot and the raw browser `attempt-H9CmOs/final.json`.

No arithmetic fixture, product manifest, Descriptor, native container, protocol
or UI is changed by this experiment. Prepared reuse remains necessary even if a
faster constructor is eventually selected.
