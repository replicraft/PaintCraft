# PaintCraft backend measurements

This directory selectively adopts the reference pack's existing two-tier workflow.
`tools/run_hyperfine.py` and `tools/benchmark_support.py` come from
`docs/temp/paintcraft-implementation-spec-v5-hyperfine/benchmarks/tools/`; the only
support adaptation is the registry path, now `benchmarks/cases.json`. Only the
currently active BF29 case is adopted. The runner preserves artifact hashes,
correctness logs, raw results and failed runs; it never builds or installs tools.

The real production CPU renderer is compared with its frozen 95db5d0 scalar
baseline in `paintcraft-render/examples/support/reference.rs`. Deterministic
fixtures are shared by the process driver and Divan. Both variants perform the
same full render; output allocation and destruction are included. Fixture
construction is outside Divan's timed section and inside process measurements.

```sh
cargo build --release -p paintcraft-render --example composite
target/release/examples/composite verify
cargo bench -p paintcraft-render --bench composite -- --sample-count 100
target/release/examples/composite baseline sparse20 10
target/release/examples/composite production sparse20 10
python3 benchmarks/tools/run_hyperfine.py --config <config.json> --output <new-dir> --execute
```

Divan is a development-only statistical harness selected for parameterized inputs
and allocation profiling. Criterion is an acceptable alternative; this selection
does not claim the harness itself is faster. See the [official API](https://docs.rs/divan/0.1.21/divan/).
Application runs require at least 20 repetitions and 3 warmups, with three
independent sessions including reverse order before a selection decision. Record
optimized binary, lockfile and generator identities and all applicable memory
metrics. A single machine cannot qualify all supported CPU/GPU targets.

The first [CPU span decision](results/cpu-spans-2026-10-19/DECISION.md) retains
three native process sessions for each case. To reproduce, build the example and
run `python3 benchmarks/tools/run_composite.py --output /tmp/new-comparison --execute`.
Use a new output directory; existing evidence is never overwritten. Derive CSV
metrics with `tools/summarize_composite.py`. Stored session archives preserve the
original unmodified Hyperfine JSON, Markdown, execution records and resource logs.
For example, `tar -xOf <session.tar.gz> <session>/hyperfine.md` reads a table without
extracting files. Archive compression does not change the original timing bytes.

The regional cache replay uses the same runner and manifests, with `--suite cache`:

```sh
cargo build --release -p paintcraft-render --example cache_replay
target/release/examples/cache_replay verify
cargo bench -p paintcraft-render --bench cache_replay -- --sample-count 100
python3 -B benchmarks/tools/run_composite.py --suite cache --output /tmp/new-cache-comparison --execute
```

This suite compares full regional composition with 64/128/256-pixel output caches
and lower-layer prefix reuse. Every timed step admits an atomic transaction,
retains recorded history, renders exact output and drops the result. The first
render/cache preparation is inside the process lifetime but outside Divan's steady
edit section. Sparse 1/5/20/100 layers, five dense layers, top- and bottom-layer global opacity
changes and a cropped viewport are distinct cases. Use `--case global-bottom5`
to run the invalidated-prefix control separately; repeated `--case` arguments
select multiple cases. Cache limits, history limits
and fixtures are identical for all granularity contenders. This does not measure
GPU dispatch or actual input-to-display latency. Raw process RSS includes the
fixture, history, returned output and persistent caches; cache weights are a
separate explicit accounting contract. See [the regional cache contract](../docs/REGIONAL_COMPOSITOR.md).


The bounded CPU partition comparison uses `--suite parallel` and BF06. Build
`--example partitioned` first; the same runner records serial, 1/2/4-worker row
bands and four-worker borrowed tile views. Each process checks live output probes
in addition to independent scalar preflight. All candidates render identical
RGBA8, and output destruction is included. Zero-work calibration is retained for
each candidate, including its own thread-pool setup/teardown.

`cargo bench -p paintcraft-render --bench partitioned` uses Criterion 0.8.2,
100 flat samples, 1-second warmup and 2-second measurement per case. Set a new
`CRITERION_HOME` to preserve raw sample/CSV output. Criterion's optional Rayon
analysis and plotting features are disabled. It is development-only; the existing
Divan targets retain allocation profiling. Divan 0.1.21 lacks raw sample export,
so prior Divan tables do not fulfill that part of the protocol. See the
[active protocol](../docs/development/HYPERFINE_PROTOCOL.md) and
[Criterion's raw output documentation](https://bheisler.github.io/criterion.rs/book/user_guide/csv_output.html).

The reverse statistical order can reuse the exact prebuilt Criterion binary:

```sh
python3 -B benchmarks/tools/run_partition_samples.py --configs <parallel-run-dir> --binary <prebuilt-bench-path> --output <new-sample-dir> --order reverse --execute
```

The default is plan-only. This uses exact-filtered Criterion invocations in
reverse parameter order, checks 100 JSON/CSV samples each, records command order
and binary identity, and retains failed output. Run it after other measurements
finish to avoid overlapping benchmarks. The first forward session and the reverse
session retain the same source/binary; no rebuild is needed to reverse order.


The desktop preview snapshot process comparison uses the production controller and
an opt-in library test driver. Build with `cargo test --release -p paintcraft --lib
--no-run --message-format=json`, select that artifact's `executable`, then run:

```sh
python3 -B benchmarks/tools/run_preview.py --binary <prebuilt-test-executable> --output <new-dir> --execute
python3 -B benchmarks/tools/summarize_composite.py <new-dir> --output <new-dir>/summary.csv
```

Only the named ignored driver executes. Preflights compare captured paths and
cancellation before each session. Three forward/reverse sessions compare unchanged
short/long/queued previews and a changing-input control, including setup and
cleanup. See the [local decision](results/preview-reuse-2026-10-20-measured/DECISION.md).
This measures preview collection/comparison, not native brush/frame latency.


Growing smooth-path coverage uses the optimized desktop lib test driver
`cpu_rendering::geometry_benchmark::preview_geometry_process`. Build the desktop
lib test in release, then set `PAINTCRAFT_GEOMETRY_VARIANT` to `verify`, `criterion`,
`baseline`, `cached` or `calibration` and run that exact ignored test. Process
fixtures use `PAINTCRAFT_GEOMETRY_CASE=tiny|growing|retrace`. The driver runs actual
Editor batch construction, checked preview staging and desktop CPU composition.
[The decision and reproduction record](results/preview-geometry-2026-10-21/DECISION.md)
retain tiny non-win controls and distinguish these complete-frame sequences from
native OS input/display latency.


Regional output patching compares the existing `cache_replay` example at full,
tile64, tile128 and tile256 configurations. The `regional_reuse` Criterion target
adds raw 20-sample complete construction/eight-edit/teardown sequences. The
[compositor record](results/regional-patches-2026-10-21/DECISION.md) retains global,
single-layer, viewport and cold controls. The desktop preview driver additionally
accepts `PAINTCRAFT_GEOMETRY_BASE=sparse1|sparse5|dense5` and
`PAINTCRAFT_GEOMETRY_COMPOSITOR=full|regional`; keep geometry caching enabled for
both when isolating the compositor. See [consumer confirmation](results/regional-patches-app-2026-10-21/DECISION.md)
for exact commands, retained host drift/memory costs and native latency limits.


Resident GPU sources use the same optimized desktop preview driver with
`PAINTCRAFT_GEOMETRY_GPU=streamed|resident`, `BASE=sparse1|dense5|dense65`,
`CASE=growing` and `VARIANT=cached` (all names carry the `PAINTCRAFT_GEOMETRY_`
prefix). `VARIANT=gpu-criterion` retains 20 flat complete-sequence samples per GPU
mode, while `VARIANT=verify` checks complete pixels/source preservation on a required
real adapter. [The record](results/gpu-sources-2026-10-21/DECISION.md) includes
completion/readback, cold setup/teardown, source-stage bytes, over-entry-limit
behavior and retained timing variability. It does not qualify CPU/GPU crossover,
sparse atlas layout or native input/display latency.


Direct native texture readiness uses the optimized desktop lib test
`gpu_display::benchmark::native_display_process`. Set
`PAINTCRAFT_GEOMETRY_BASE=sparse1|dense5|dense65` and
`PAINTCRAFT_DISPLAY_VARIANT=readback|texture|verify|criterion|calibration`.
[The record](results/direct-display-2026-10-21/DECISION.md) compares completed
readback/ColorImage/backend upload with completed texture conversion/native
registration, retains mixed/tied controls and separates this boundary from OS
input/display latency. The first failed preflight is preserved separately.

Native viewport texture readiness uses the optimized desktop lib test
`gpu_display::region_benchmark::native_viewport_process` and the committed runner:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --binary <reported-lib-test-executable> --output <new-result-directory>
```

[The decision](results/native-viewport-2026-10-21/DECISION.md) retains stationary,
panning and full-canvas controls, exact independent visible pixels, native App
acceptance, completed GPU work and three process sessions with raw statistical
samples. The full-output control remains mixed/tied; cropped work reductions do not
imply proportional latency gains. OS presentation and large zoomed-out mipmap
behavior remain separate qualification work.

The same native runner accepts `--suite output-pool` for explicit fresh versus
64 MiB completed compositor-buffer retention. It records small, 4096×2160 and
alternating-size cases, with the same raw statistical/process and independent
pixel gates. [The decision](results/output-pool-2026-10-21/DECISION.md) rejects global
enablement: fewer logical buffer creations did not establish a general native gain.
Fresh allocation stays the default; display textures are never recycled by this pool.

`--suite multires` compares the existing cached full-native ROI plus direct box
filter against bounded and cached CPU box sampling, actual App acceptance and
completed native managed-texture upload. The exact test is
`cpu_rendering::display_benchmark::native_multires_process`. See
[the source-scoped decision](results/multires-display-2026-10-21/DECISION.md) for tiny
controls, dense cache costs, the sparse-4K gain, raw 180 statistical samples/600 process
observations, and the distinction from GPU scaled composition or OS frame latency.

The `single-layer` suite compares direct cached-source native display with the
existing ordered intermediate compositor. Build the optimized desktop library test
executable as above, then run `run_native_viewport.py --suite single-layer --binary
<executable> --output <new-dir>`. It checks actual App registration and independent
command/scalar pixels before timing. Six cases retain cold setup/teardown, raw
Criterion samples, three process sessions, calibration and process RSS, including
multiple-layer and partial-coverage fallback controls. See
[the complete decision](results/gpu-single-layer-2026-10-21/DECISION.md).


The bounded stroke-preparation comparison uses a frozen CPU-only baseline CLI,
its source/hash manifest and feature tree, a separately built CPU-only candidate,
and the `paintcraft-raster` `stroke_union` optimized Criterion executable:

```sh
python3 -B benchmarks/tools/run_stroke_preparation.py --baseline <baseline-manifest.json> --baseline-features <baseline-features.log> --candidate <prebuilt-cli> --kernel <prebuilt-bench> --output <new-dir> --execute
```

Without `--execute` this prints a plan and writes nothing. Capture the baseline
before editing source, build both CLIs with `cargo build --release --locked -p
paintcraft-cli`, and keep the candidate executable outside Cargo's output path.
The runner requires identical CPU-only feature trees and verifies artifact hashes
and exact canonical PCND/member output before and after its sessions. Do not run
builds/tests alongside timing. The [local decision](results/stroke-preparation-2026-10-21/DECISION.md)
retains all 160 statistical samples and 720 process observations, including slower
checked unions, calibration and variable controls. It qualifies preparation costs,
not native input/display or cancellation wall-clock latency.
