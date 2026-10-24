# Completed native GPU texture readiness

Use direct completed GPU textures for connected native windows. The display pass
matches selected egui Color32 premultiplication exactly and removes normal host
readback, CPU ColorImage conversion and re-upload. Headless/CPU fallback and straight
readback/export remain available. This local comparison finds a modest sparse gain,
mixed dense5 results and a dense65 tie; it is not a universal throughput, OS display
latency or physical GPU-memory claim.

## Equivalent boundary and correctness

Both variants run actual desktop Editor synthetic input, preview batch construction,
checked private raster painting and ordered resident-source GPU composition. Each
512x256 sequence has 32 frames of 16 new brush points. The readback control completes
straight GPU readback, App::install_render CPU ColorImage conversion and egui backend
upload, then submits/waits for that upload. The texture candidate completes the exact
GPU premultiplication pass and App::install_gpu_texture native registration. Both
therefore reach a texture ready for egui; neither times submission alone.

Cold adapter/device, GPU/egui pipelines, font-atlas initialization/upload, source
fixture/App construction and final destruction are included. Source-cache limits,
geometry and pixel policy are identical. Native registration ownership is bounded
by the accepted frame and is released on replacement/fallback/drop. The sequence
publishes no edit or save. It does not draw the entire app or wait for an OS swap;
a separate real egui pixel test draws native/managed images together and verifies
exact nontrivial output over a background.

Real-device tests verify every color/alpha pair against independent integer math
and actual egui ColorImage conversion, clipped odd dimensions/padded strides,
cancellation, device retirement, stale keys/epochs and registration cleanup.
Preflight compares every complete verified frame against scalar output (premultiplied
through actual ColorImage for textures) and checks original source pixels unchanged.
Postflight repeats all cases after each process session and verifies source, WGSL,
manifest and executable identities. Required runs pass eleven renderer GPU tests
and three native registration/draw tests on Apple M1 Pro / Metal. The release native
window smoke exits after 30 frames with an empty log.

## Results, including controls and variability

Criterion retains 20 flat complete-sequence samples per mode/case (120 total),
300 ms requested warmup and a one-second requested interval, extended for slow cases.
Hyperfine retains three sessions, 20 runs and three warmups per mode/case, reversing
order in session 2. All 360 comparisons and 60 empty-process calibrations remain,
with no analyst exclusions.

| Case | Criterion readback / texture median ms | Process readback median ms across sessions | Process texture median ms | Paired readback/texture ratio |
|---|---:|---:|---:|---:|
| Sparse one layer | 186.20 / 148.31 | 110.03–221.78 | 97.49–210.36 | 1.05–1.13x |
| Dense five layers | 406.98 / 350.60 | 165.74–227.66 | 148.44–187.19 | 0.99–1.25x |
| Dense 65 layers | 1489.20 / 810.99 | 873.63–1076.08 | 875.98–1039.84 | 1.00–1.03x |

Sparse gains persist but vary. Dense5 reverses direction in session 3 and is not a
stable universal win. Dense65 is a process tie despite its statistical-tier result;
all data are retained. Host/GPU levels change substantially between sessions and
statistical/process tiers. CPU affinity, thermal/power state and background load
were not controlled and no cause is inferred. No build/test/other benchmark was
intentionally concurrent. Empty-process medians are 11.04–20.14 ms and are never
subtracted. Cold setup remains in each observation; these are not hot per-frame FPS,
GPU timestamp durations, queue-inclusive p95/p99 or input-to-display measurements.

Each ordinary texture frame has no host canvas result; the corresponding readback
control allocates/consumes straight bytes, converts them and completes backend
re-upload. This is a verified dataflow change, not a hardware bus-byte measurement.
Explicit verification texture readbacks run outside timed ordinary sequences.

Reported RSS medians (readback/texture) are 34.35/29.47 MiB, 38.07/33.08 MiB and
68.70/66.03 MiB. Maxima are 46.66/34.03, 42.92/36.30 and 73.78/69.30 MiB. These
complete-process observations do not establish GPU allocator/UMA residency, a global
memory bound or long-session reclamation. The accepted GPU output and unread worker
result have distinct lifetimes outside the source-cache size limits.

## Reproduction and preserved failures

Build the optimized desktop lib test with the command recorded in manifest.json.
Run its executable with:

```text
--exact gpu_display::benchmark::native_display_process --ignored --nocapture
```

Set `PAINTCRAFT_GEOMETRY_BASE=sparse1|dense5|dense65` and
`PAINTCRAFT_DISPLAY_VARIANT=readback|texture|verify|criterion|calibration`.
Use Hyperfine `--shell=none --warmup 3 --runs 20 --export-json FILE`, reversing the
middle session. Verification requires a real adapter and fails if none exists.
Original exports are byte-preserved in raw_exports/*.gz; compact JSON contains
identical parsed values and both hashes are recorded. Statistical samples/configs/
estimates/outliers, full-image checks, build/static/real-device logs and smoke records
are retained.

[The first failed preflight](../direct-display-preflight-2026-10-21/README.md)
collected no timings: it uploaded a zero-size font placeholder before the initial
egui pass. Both candidates now perform the same UI initialization. That manifest
and source override remain archived. Other excluded-validation logs record a caught
WGSL reserved identifier, unapplied test texture-delta acknowledgement and an existing
pressure-test teardown race. The latter now waits for actual abandoned maintenance
lease release, matching executor cancellation semantics; it changes no production
policy. None of these failures is a removed performance observation.

Output pooling, viewport/dirty-region/multiresolution textures, sparse GPU atlases,
CPU/GPU crossover choices, asynchronous services, physical all-service budgets,
automatic native device recreation and OS event/display latency remain open. The
full roadmap and release qualification remain partial.
