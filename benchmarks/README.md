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
