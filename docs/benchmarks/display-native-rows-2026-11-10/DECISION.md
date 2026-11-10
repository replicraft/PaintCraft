# Native-row display accumulation

The row API provides atomic validation and bounded work accounting. These
measurements do not establish a general CPU speedup. Its default-canvas result
changes direction when execution order is reversed, and small cases regress.

Criterion 0.8.2 compared the scalar and row APIs on macOS 14.7.2 arm64 using
rustc 1.99.0. Each condition used 30 samples, a one-second warmup and a requested
two-second measurement window. Forward and reverse runs used separate processes
and result directories, reversing both case and contender order. All 720 raw
samples and outliers remain preserved. Each case verifies equal sample counts
and resolved pixels before timing. Fresh accumulator setup is excluded; row
preflight is included.

The table reports fitted slope estimates. Positive change means the row API
took longer. The default field is 1000 by 486 native pixels at step 16. Other
fields are 65 by 3 pixels, except step 128, which is 129 by 3.

| Case | Forward scalar / row (ns) | Row change | Reverse scalar / row (ns) | Row change |
|---|---:|---:|---:|---:|
| Default field | 1,769,282 / 1,706,710 | -3.54% | 1,723,202 / 1,762,679 | +2.29% |
| Step 1 | 590.5 / 590.4 | -0.01% | 572.3 / 589.5 | +3.00% |
| Step 2 | 646.0 / 739.1 | +14.41% | 666.8 / 752.5 | +12.85% |
| Step 16 | 655.5 / 689.7 | +5.21% | 660.2 / 705.5 | +6.88% |
| Step 64 | 706.0 / 694.2 | -1.68% | 682.5 / 703.4 | +3.06% |
| Step 128 | 1,403.0 / 1,416.8 | +0.98% | 1,378.9 / 1,517.2 | +10.03% |

These are accumulation-kernel measurements, not whole-renderer or browser
latency, GPU timings or physical-memory measurements. Logical work reductions
describe the admission policy and must not be presented as measured speed gains.
Further CPU optimization needs a measured improvement on representative renders.

The tracked reproduction target is
`cargo bench -p paintcraft-render --bench display_native_rows`.
Set `PAINTCRAFT_ROW_BENCH_ORDER=forward` or `reverse` and use a separate
`CRITERION_HOME` for each run. The frozen executable was run with `--bench`;
without that switch Criterion only exercises its test mode.

Local evidence is under
`docs/validation/source8-display-native-rows/root-final-native/measurements`:
`summary.json` retains estimates, confidence intervals, source paths and host
details; the two `criterion` trees retain raw samples. The same fifteen source
hashes passed native regression and actual 887-case Node Wasm validation.
The first pre-promotion filtered test run selected no new tests and is excluded
from qualification. Initial formatting and harness-preflight failures are also
preserved separately from the successful runs.
