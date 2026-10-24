# Completed exact GPU scaled display: retain CPU automatic dispatch

Keep the cached CPU scaled display as the automatic desktop default. Retain the
correct GPU implementation behind `PAINTCRAFT_GPU_SCALED=1` for explicit native
experimentation, using 64-sample output tiles. Native-resolution GPU rendering is
unchanged. No general CPU/GPU crossover follows from these four fixtures.

## Reproduction and required work

Measured source/runner: `c3dc4b0e701802bad1c48ab620ac59797346774c`.
`benchmarks/results/gpu-scaled-display-2026-10-21/manifest.json` (local result) records compiler, executable hash, source hashes,
adapter environment, exact test identity and settings. The independently checked
`benchmarks/results/gpu-scaled-display-2026-10-21/identity-verification.json` (local result) confirms the executable and all
source blobs matched that commit after measurement, before the final default-policy
follow-up. The measured candidate attempted GPU scaling automatically; the final
policy rejects global enablement based on the results below.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --suite gpu-scaled --binary <reported-lib-test-executable> --output <new-result-directory>
```

Each sequence includes eight real desktop edit/preview frames. Brush cases add
eight points per frame to a translucent size-16 brush, through actual incremental
preview staging. Uniform input instead publishes eight typed layer-opacity edits
(128 through 135), preserving implicit white raster storage with zero resident tiles.
Cold device, renderer, font, document construction and teardown are included.

`cpu` uses the established cached CPU multiresolution renderer, App ColorImage
acceptance and completed native egui managed-texture upload. `gpu32` and `gpu64`
use exact ordered native GPU composition followed by wide-integer box reduction,
completed GPU submissions and App native texture registration. Only the output tile
edge differs between GPU variants. Private compositor retention is explicitly zero.
GPU source caching retains its existing policy. No GPU readback occurs in timed
display production. Verification readback is outside timing.

Every case has independent command/native-composition/direct-box verification
before measurements and after each process session. These assert every required
output byte, sampling bounds, unchanged authoritative brush source/ticket or eight
actual uniform-case publications. The runner checks that exactly one oracle test
executes and requires a real adapter. This measures completed texture readiness,
not OS presentation, input latency or physical GPU budgeting.

## Results on Apple M1 Pro / Metal

Each table entry is the range of process medians across three sessions, in ms per
complete eight-frame sequence. Ratios divide CPU by GPU: below one means GPU slower.
Session two reverses variant order. All observations, including slow samples, remain.

| Case | CPU ms | GPU32 ms | GPU64 ms | CPU / GPU64 |
| --- | --- | --- | --- | --- |
| Tiny 32-square, one layer, level 1 | 52.74–63.81 | 68.80–94.18 | 74.59–122.77 | 0.43–0.86 |
| Dense 512×256, five layers, level 1 | 62.58–92.99 | 639.45–678.84 | 206.68–260.09 | 0.29–0.36 |
| Sparse 4096×2160 brush, level 2 | 68.20–81.45 | 82.65–105.42 | 71.97–79.42 | 0.95–1.03 |
| Uniform 4096×2160 opacity edits, level 2 | 77.98–90.42 | 60.75–88.47 | 61.75–75.33 | 1.20–1.36 |

Criterion's 20 raw flat samples per variant give CPU/GPU32/GPU64 medians of
21.64/36.71/31.81 ms for tiny, 28.54/633.90/237.14 ms for dense,
44.43/72.85/48.05 ms for sparse 4K and 35.09/31.95/23.38 ms for uniform 4K.
These are within-process sequence samples with their own setup/teardown; process
startup is not subtracted from Hyperfine results. Process calibration is retained.

The dense GPU source-upload total is 20 MiB per sequence in both layouts. Changing
tile edge reduces source-buffer creations from 1,280 to 320, but there are no
unchanged source hits: existing changed-view retirement does not preserve a tiled
working set. Multiple completed submissions per native block remain expensive.
This is evidence for further residency/scheduling work, not a cache-size-only fix.

Sparse 4K native visits increase from 262,144 to 524,288 with larger tiles, while
source allocations fall from 16 to one and strip patches increase uploaded bytes
from 1 MiB to 1.125 MiB. Uniform 4K visits zero native pixels and accounts for
70,778,880 implicit source pixels across eight frames. Less allocation does not
necessarily mean less source work or faster rendering.

Process RSS medians are approximately 45–48 MiB CPU versus 25–26 MiB GPU for sparse
4K, and 44–45 MiB versus 23–24 MiB for uniform 4K. Tiny/dense GPU RSS is higher.
These are host process metrics, not proof of physical GPU allocation reclamation.
The tiny GPU32 maximum is 775.84 ms, dense GPU32 reaches 964.69 ms, and uniform CPU
reaches 295.18 ms. No outlier is excluded; host drift and slow observations limit
generalization. There are no affinity, thermal or background-load controls.

## Artifacts, limitations and final policy

`benchmarks/results/gpu-scaled-display-2026-10-21/summary.json` (local result) retains per-session medians/min/max/RSS and all statistical
sample summaries. There are 240 raw statistical samples and 780 process observations
(720 comparison plus 60 calibration), all with zero exit status. Hyperfine original
JSON bytes are gzip-preserved alongside compact equal-valued JSON and SHA sidecars.
Long diagnostic logs are gzip-preserved rather than truncated to satisfy file limits.

The final policy keeps CPU scaled display by default and leaves GPU available for
native-resolution views. The explicit environment option exercises the real native
worker/registration/fallback integration; it is not a promise of acceleration.
The uniform advantage does not justify an unmeasured general dispatch threshold.
The final policy's tests/build evidence is recorded separately from the frozen
benchmark identity. The expanded renderer oracle includes 164 analytic huge-canvas
cases, both output tile edges, dense carry, cancellation and old-frame ownership.

During development, two standalone test invocations omitted the renderer's `gpu`
feature and executed zero tests; neither is counted as validation. Corrected
feature-enabled Metal invocations explicitly execute the four scaled tests and
the complete renderer suite. This failure is retained with the validation logs.

Single-layer source bypass, sparse GPU working tiles/output caches, asynchronous
completion, native device reconstruction, global physical admission, dense giant
documents, cross-platform conformance and OS input/display tails remain open.
This completes a measured GPU scaled-display baseline, not the full roadmap.
