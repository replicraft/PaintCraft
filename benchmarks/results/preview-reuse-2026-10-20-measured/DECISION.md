# Preview snapshot reuse: local process comparison

Retain revision-keyed weak snapshot reuse for unchanged desktop previews. The
measured process medians improve for the tested idle inputs. Changing-input cost
is inconclusive, and one cached changing run is substantially slower. This is a
preview capture/comparison measurement, not brush rasterization, compositor
throughput, native frame latency or a physical memory guarantee.

## Work and correctness

The optimized library-test driver calls the real controller's `collect_preview`
and `preview_snapshot`. The baseline freezes pre-a21d733 collect/compare behavior;
the contender uses Arc identity before comparing changed values, as native
`sync_texture` does. Both retain the displayed snapshot. Process lifetime includes
fixture construction, initial capture, every iteration, and destruction of the
snapshot, tool/editor state and the queue fixture's idle native worker pool.
No window, font rasterization, GPU work or document publication is timed.

Fixtures are a 32-point live path, a 65,536-point live path, 31 queued paths of
8,192 points each, and a 65,536-point path extended before each capture. Preflight
runs compare the complete preview values, check generated point/path counts and
origin independently, confirm idle allocation identity, and verify cancellation
removes the preview. The ordinary cache tests cover file boundaries, completion,
text/shape changes, expiry and old-snapshot immutability. These are not historical
Paint reference fixtures or pixel-rendering benchmarks.

Source is 8a7d1fd401837c57a5b2780b4f109df370baba05, with defining source, launcher,
optimized test executable and Cargo.lock hashes in every run configuration.
Host: Apple M1 Pro, macOS 14.7.2 arm64, Rust 1.99.0, Hyperfine 2.0.0. Release defaults,
no incremental compilation. Three independent sessions, forward/reverse/forward,
20 measured processes and 3 warmups per variant/case; 60 samples per table row.
Power, affinity and thermal state were not controlled. No other benchmark ran
concurrently. Raw samples are all retained, including outliers.

## Pooled observations

Durations and RSS are medians of complete processes, not per-event timings.
The speed ratio is baseline median divided by cached median.

| Input | Captures/process | Baseline ms | Cached ms | Ratio | Baseline / cached peak RSS MiB |
|---|---:|---:|---:|---:|---:|
| Short, unchanged | 100,000 | 25.79 | 19.11 | 1.35× | 7.94 / 7.89 |
| Long, unchanged | 512 | 43.75 | 18.26 | 2.40× | 15.13 / 10.00 |
| Queued, unchanged | 128 | 82.46 | 25.99 | 3.17× | 41.79 / 17.14 |
| Long, changing | 512 | 29.87 | 30.07 | 0.99× | 16.31 / 16.29 |

Median process CPU time falls from 18.88 to 12.47 ms (short), 36.63 to 11.38 ms
(long), and 74.34 to 18.93 ms (queue). Changing CPU medians are 22.77 / 22.82 ms.
[summary.csv](summary.csv) retains throughput, mean, standard deviation, min/max,
CPU cycles/instructions and process peak RSS. RSS includes allocator, fixture and
test runtime; it does not establish steady native-editor reclamation or budgets.

The maximum changing process is 37.20 ms baseline versus 84.78 ms cached; the
cached sample remains in session 2. Its cause is unqualified. Corresponding maxima
for short are 28.44 / 21.59 ms, long 50.34 / 20.49 ms, queue 199.44 / 37.13 ms.
The changing-input control is a tie/inconclusive result, with no tail-latency win
claimed. Unchanged captures benefit in all three sessions; do not extrapolate
these ratios to moving brushes, text, GPU output or input-to-display latency.

## Evidence and reproduction

`session-*.tar.gz` preserve original Hyperfine v2 JSON, Markdown, preflight output,
execution records and resource logs byte-for-byte. Their internal manifests are
verified by the existing summarizer. Supplemental `/usr/bin/time -l` measurements
are separate from the 60 timed samples. `MANIFEST.sha256` covers these archives and
the other retained evidence. Build messages, strict desktop/all-target Clippy and
workspace test logs are included. All 613 workspace tests pass; 21 explicit ignores
include the new opt-in process driver. The production-source changes were already
validated with Metal parity and a native window in the preceding slice.

Build the release library test binary first, discover its `compiler-artifact`
`executable` from Cargo JSON, and use that exact path:

```sh
CARGO_INCREMENTAL=0 cargo test --release -p paintcraft --lib --no-run --message-format=json > /tmp/preview-build.json
python3 -B benchmarks/tools/run_preview.py --binary <prebuilt-test-executable> --output <new-dir> --execute
python3 -B benchmarks/tools/summarize_composite.py <new-dir> --output <new-dir>/summary.csv
```

The first preparation omitted the `/usr/bin/env` launcher hash. The runner refused
before timing; `excluded-launcher-preflight.tar.gz` retains that configuration and
error. After correcting it, session-1-short timing completed in the sandbox, but
its supplemental resource command exited 1 because `sysctl kern.clockrate` was
blocked. That log remains in the session archive. Only the supplemental command
was rerun outside the sandbox; the completed timing was not restarted. Remaining
sessions ran with resource counters accessible. `sandbox-resource-failure.log`
retains the interrupted orchestration traceback. No timing samples were excluded.

Criterion allocation samples, native event/frame traces, sustained interaction,
background interference and cross-platform qualification remain. Changed previews
still collect/compare paths on the UI thread. T13/T37 and the wider BF29 experiment
remain partial; this process comparison does not complete those gates.
