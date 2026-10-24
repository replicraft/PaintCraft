# Native viewport texture readiness

Adopt exact regional desktop display requests. Required visible pixels match full
output cropped to the viewport, while the requested output and uploaded-source work
are smaller for cropped views. This is a scoped native GPU texture-readiness result,
not a universal rendering gain, CPU/GPU crossover, OS presentation rate or input p99.

## Reproduction and required work

Source commit `031eff29376606a944a07e84c2e129841a298e7c` includes the production
regional APIs, desktop integration, exact tests and measurement runner. The frozen
`benchmarks/results/native-viewport-2026-10-21/manifest.json` (local result) hashes Rust/WGSL/manifests, runner and optimized executable;
all source hashes were checked against that commit and the executable/current source
identities checked before/after sessions. Build and compiler details are recorded.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --binary <reported-lib-test-executable> --output <new-result-directory>
```

The runner invokes ignored test `gpu_display::region_benchmark::native_viewport_process`
with `PAINTCRAFT_GEOMETRY_BASE=dense5`, `PAINTCRAFT_REQUIRE_GPU=1`,
`PAINTCRAFT_VIEWPORT_CASE=all|quarter|pan` and
`PAINTCRAFT_VIEWPORT_VARIANT=full|region|verify|criterion|calibration`.

Both variants construct the same 512×256 five-layer synthetic document and a real
Metal device/egui renderer. They stage the same growing translucent size-16 brush
through the actual desktop incremental-geometry path, then complete ordered GPU
composition/premultiplication and install the accepted App texture. Each sequence
contains 16 frames, adding eight points per frame. Device/fixture/cache construction,
result lifetimes and teardown are included. No window/swap/font rendering is timed.
The full control computes the full canvas; the contender computes only required
visible bounds. Both use the same device-local source cache and numerical policy.

`all` requests the entire canvas for both variants and is the non-win control.
`quarter` is a stationary 128×96 region at (129,33), covering 9.375% of the canvas
(the name describes its quarter-width, not quarter-area). `pan` moves that same-size
region through 12 origins over 16 frames, including revisits. This small fixture
stays within source-cache entry limits; over-limit GPU qualification is separate.
Neither case represents photographic effects or a giant zoomed-out document.

Before and after each process session, verification independently executes commands,
uses scalar regional composition, checks every visible premultiplied pixel, validates
accepted App keys and unchanged authoritative source/ticket. The full control is
read back and cropped only in verification, outside timing. Required real-adapter
information and device limits are printed in these logs. Timing consumes and drops
the actual completed display results; no synthetic upload timing replaces them.

## Results

Times below are milliseconds per complete 16-frame sequence, including cold setup
and teardown. They are not individual frame latency. Criterion retained 20 flat raw
samples per variant/case (120 total), 300 ms warmup and a requested one-second
measurement window. Each of three Hyperfine sessions retained 20 observations and
three warmups per variant/case; session two reverses variant order. The 360 comparison
observations and 60 zero-work calibration observations are retained without exclusions.

| Case | Criterion mean full / region | Process medians full, sessions 1–3 | Process medians region, sessions 1–3 | Paired full / region |
| --- | --- | --- | --- | --- |
| Whole canvas control | 53.98 / 47.34 | 91.24, 90.42, 97.97 | 92.87, 91.41, 91.81 | 0.98–1.07×; tie/mixed |
| Fixed 128×96 | 50.22 / 46.22 | 90.92, 89.83, 88.95 | 85.45, 84.23, 84.29 | 1.06–1.07× |
| Panning 128×96 | 47.35 / 45.68 | 91.60, 92.57, 92.42 | 84.30, 83.55, 86.27 | 1.07–1.11× |

Whole-canvas paths perform equivalent work. Their different Criterion means and
mixed process ratios expose host/GPU variation; the apparent statistical improvement
is not adopted as a whole-canvas gain. Cropped process improvements are modest:
about 5–7% lower elapsed time for fixed views and 7–10% lower for pan (ratios above
express baseline/contender). The full raw maxima and slow observations stay in
`benchmarks/results/native-viewport-2026-10-21/summary.json` (local result) and original exports. No outliers were removed.

Calibration medians are 11.40, 13.55 and 13.18 ms; startup is material and is never
subtracted. CPU/GPU synchronization, native setup and complete operation staging
remain in both variants, so reducing output area does not imply proportional speed.

| Case | Full / regional output bytes per sequence | Full / regional source bytes uploaded |
| --- | --- | --- |
| Whole canvas | 8,388,608 / 8,388,608 | 6,553,600 / 6,553,600 |
| Fixed 128×96 | 8,388,608 / 786,432 | 6,553,600 / 975,360 |
| Panning 128×96 | 8,388,608 / 786,432 | 6,553,600 / 3,932,160 |

The cropped cases require 10.67× fewer output pixels. These are completed work/transfer
counters, not simultaneous allocations or physical GPU memory. Process RSS median
ranges are 25.02–26.60 / 25.30–25.72 MiB for the whole-canvas control,
25.23–25.71 / 24.26–24.66 MiB for fixed views, and 25.52–25.96 / 24.10–25.27 MiB
for pan. Maximum observed RSS is respectively 29.06 / 27.23, 27.88 / 25.86 and
28.45 / 26.83 MiB. These describe whole benchmark processes, not all-service budgets
or actual driver allocation/reclamation.

## Evidence and limits

`hyperfine-*.json.gz` preserves exact original Hyperfine bytes; adjacent compact
JSON preserves equal parsed values and identity sidecars hash both forms. Raw
Criterion samples/estimates, session logs, correctness pre/postflights, environment,
source/binary hashes and derived summary are retained. The Apple M1 Pro developer
host has no affinity/thermal/power/background-load control. No builds, other tests
or other benchmarks were intentionally concurrent with timing.

747 workspace tests pass, strict all-target workspace Clippy and release desktop/CLI
builds pass. Required Metal renderer tests pass (13), including regional integer
parity, invalid/cancelled requests, large-canvas small-region admission and later full
exports. Native registration/draw tests pass (3); actual GPU framebuffer pixels match
managed CPU images for full and cropped views. Two desktop regional tests run on
required Metal; CPU texture-limit recovery is also covered in the workspace suite.
A 30-frame release window smoke exits zero with no log output. GUI tests exercise a
16384-square canvas at multiple DPI/zoom values and forced layout passes. Their
explicit backend limit is 8192, while a separate test verifies safe 2048-limit failure.

`excluded-validation/` retains failed development checks: old whole-buffer GUI
expectations, test module visibility/path mistakes, rounded zoom reconstruction,
modal/shortcut setup, the oversized CPU texture assertion subsequently guarded, and
a Clippy test lint. None collected timing data; none of the successful timed data
is excluded. They remain evidence of development failures, not current limitations
silently hidden by the benchmark.

Regional output is not multiresolution rendering. Zoomed-out huge regions can still
exceed render/texture limits; output pools, tiled display, mipmaps, GPU sparse atlases,
asynchronous transfers, full incremental staging, physical all-service memory budgets,
native device recreation and event/display tails remain unqualified or unimplemented.
Windows/Paint.NET reference conformance and other platforms remain separate gates.
