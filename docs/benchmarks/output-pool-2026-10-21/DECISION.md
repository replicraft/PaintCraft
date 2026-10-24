# Completed compositor output pool: explicit option, fresh default

Keep fresh allocation as the default. The 64 MiB completed-buffer pool is implemented
and correct, and reduces compositor buffer creations from 16 to one in these sequences.
It does not establish a generally faster native path: small views regress in all
process sessions, 4K results are mixed, and statistical/process results disagree.
Hosts may explicitly configure retention; native default admission/retention is zero.
This is a measured rejection of global enablement, not a claimed performance win.

## Source and reproduction

The measured implementation/runner is frozen at
`94939e417a4a8f6968625c1af76c34d442ae6b5e`. The `benchmarks/results/output-pool-2026-10-21/manifest.json` (local result) records
source/runner/shader/manifests and executable identities, compiler/build and environment.
`benchmarks/results/output-pool-2026-10-21/identity-verification.json` (local result) confirms all 349 source hashes
against that commit and the executable after measurement. The subsequent policy change
sets the public default from the candidate 64 MiB to zero; explicit 0/64 MiB comparison
paths are unchanged. Reproduce the frozen evidence from the recorded source revision.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --suite output-pool --binary <reported-lib-test-executable> --output <new-result-directory>
```

Ignored test `gpu_display::region_benchmark::native_viewport_process` accepts
`PAINTCRAFT_VIEWPORT_CASE=quarter|large|resize` and
`PAINTCRAFT_VIEWPORT_VARIANT=unpooled|pooled|pool-verify|pool-criterion|calibration`.
The runner also sets `PAINTCRAFT_GEOMETRY_BASE=dense5` and requires a real GPU.

Each variant stages the same actual desktop growing translucent size-16 brush,
composes/converts a completed native GPU texture, accepts it through App's ordinary
region/key/epoch checks and destroys all outputs/caches. Each sequence has 16 frames
with eight new points per frame. Device/renderer/document construction, operation
staging, source-cache handling, explicit clears, GPU completion, native registrations
and teardown are included. Display textures are freshly owned in both variants;
only the private compositor buffer is pooled. Unpooled buffers use the former fresh
STORAGE/COPY_SRC usages and guaranteed zero initialization; pooled-capable buffers
add COPY_DST and explicitly clear logical output on reuse. No completed work is
replaced with enqueue-only timing. No OS window/swap/font rendering is timed.

`quarter` is a fixed 128×96 viewport over the existing 512×256 dense five-layer
fixture. `large` uses an entire 4096×2160 canvas with an implicit white sparse layer
and the same growing brush. `resize` alternates that large region and a 128×96 region
at (129,33); it intentionally retains the large capacity rather than hiding that cost.
The 4K fixtures are sparse, not dense multilayer photographs or reference Paint files.

## Correctness and counters

Verification before and after each process session independently executes commands,
composes scalar reference pixels and compares every required visible premultiplied
pixel. Published source pixels and document tickets remain unchanged. Verification
readback/cropping is outside timing. Real adapter and limits are logged.

| Case | Output bytes per sequence, both modes | Source bytes uploaded, both modes | Buffer creations fresh / pooled | Retained after last frame fresh / pooled |
| --- | --- | --- | --- | --- |
| Fixed 128×96 | 786,432 | 975,360 | 16 / 1 | 0 / 49,152 B |
| Full 4096×2160 | 566,231,040 | 66,846,720 | 16 / 1 | 0 / 35,389,440 B |
| Alternating large/small | 283,508,736 | 283,508,736 | 16 / 1 | 0 / 35,389,440 B |

The pool reuses its buffer 15 times. Alternating views finish on the small output,
but keep 33.75 MiB of compositor capacity. These counters describe logical buffer
creation/retention, not physical driver allocation, zero-copy claims or RSS release.
Source-stage costs are equal within each comparison and are deliberately included.

## Timing and memory

Criterion retains 20 flat raw samples per mode/case (120 total), with 300 ms warmup
and requested one-second measurement windows. Three Hyperfine sessions each retain
20 runs and three warmups per mode/case; session two reverses variant order. All
360 comparison observations and 60 calibration observations remain, without exclusions.
Times are milliseconds per complete 16-frame sequence including cold setup/teardown,
not individual frame latency, OS presentation or input p99.

| Case | Criterion means fresh / pooled | Process medians fresh, sessions 1–3 | Process medians pooled, sessions 1–3 | Paired fresh / pooled |
| --- | --- | --- | --- | --- |
| Fixed 128×96 | 98.27 / 63.31 | 152.69, 94.78, 93.65 | 173.40, 107.48, 96.73 | 0.88–0.97×; pooled slower |
| Full 4096×2160 | 183.71 / 162.20 | 255.03, 228.60, 224.99 | 338.54, 216.36, 211.05 | 0.75–1.07×; mixed |
| Alternating large/small | 199.33 / 248.19 | 269.36, 251.41, 259.49 | 220.89, 237.44, 250.11 | 1.04–1.22×; process gain, statistical regression |

Large host/GPU variability is visible. For example a small pooled process reaches
372.68 ms versus a 107.48 ms session median, and a large pooled process reaches
654.20 ms versus a 338.54 ms median. No observation is discarded or attributed to
an unverified thermal/background cause. Strong apparent statistical gains for tiny
views do not survive process confirmation; alternating-view statistical/process
results even disagree in direction. These data do not justify general dispatch
thresholds, global enablement or a claim that fewer creations means faster painting.

Calibration medians are 12.34, 12.84 and 12.36 ms, never subtracted. Process RSS median
ranges fresh / pooled are 25.70–25.98 / 25.44–25.90 MiB for fixed views,
87.27–87.63 / 86.81–87.53 MiB for full 4K, and 90.47–91.06 / 89.85–91.04 MiB for
alternating views. Maximum RSS is respectively 27.13 / 27.41, 87.91 / 87.98 and
92.03 / 91.80 MiB. Peak process RSS does not measure post-eviction GPU residency;
there is no demonstrated process-wide memory improvement from the pool.

## Evidence and remaining work

Original Hyperfine bytes are preserved in gzip with adjacent equal-valued compact
JSON and hashes of both forms. Criterion raw samples/estimates, summaries, correctness
pre/postflights, all session logs and source/executable identities are retained. The
Apple M1 Pro developer host had no affinity/power/thermal/background-load controls.
No builds, other tests or other measurements were intentionally concurrent with timing.
No failed timed run or timed observation was excluded.

750 workspace tests pass (24 explicit ignores), strict workspace/all-target Clippy
and release desktop/CLI builds. Required real Metal renderer checks pass (16),
including output retention limits, fresh/oversize streaming, smaller/larger transparent
clears, cancellation at observed warm checkpoints, retry and retired epochs. Native
registration/draw checks pass (3): an old registered texture still draws exact CPU
reference pixels after later compositions reused its private scratch. The final
zero-default policy has its own workspace/native checks and release smoke evidence.

The pool is private derived working data; it does not mutate documents/history or
recycle public display textures. Native admission can declare an explicitly configured
retention allowance, but all-service physical budgets and GPU pressure-driven eviction
remain open. Reusing egui/public texture views requires a separate ownership protocol.
Multiresolution/tiled output, asynchronous transfers, device recreation, native event/
display tails, other adapters/platforms and full roadmap acceptance remain incomplete.
