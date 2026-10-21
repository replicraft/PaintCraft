# Matched CLI confirmation of ordered paint replay

Retain operation-level tile admission and row replay. Real CPU-only CLI apply
commands confirm local horizontal-pencil, smooth-brush and smooth-eraser gains.
Diagonal pencil remains a tie/slight regression control. This qualifies these
commands and fixtures on the recorded macOS arm64 host, not native input latency.

## Frozen comparison

Baseline d479023 and candidate f23e5c3 were both built with
`CARGO_INCREMENTAL=0 cargo build --release --locked -p paintcraft-cli`.
The manifest records distinct executable SHA-256 identities, build logs and
feature trees; neither CLI enables the render GPU feature. A shared-target cache
initially returned a baseline artifact for the candidate. Before any included
measurements, workspace entrypoint mtimes were refreshed without changing bytes,
the candidate was rebuilt, and its identity and new diagnostic marker verified.

Each command reads and hash-checks a PCND, validates and executes an atomic
100-operation batch, publishes history, encodes PCND and atomically replaces its
output. Fixtures use a 1000x1000 canvas with transparent hidden RGB and an opaque
painted band. Pencil paths span 1000 pixels; brush/eraser use smooth-round size 32.
Three sessions use 20 runs and three warmups per command, reversing implementation
order in session 2. All 480 observations are retained, with no analyst exclusions.

| Batch | Baseline median ms across sessions | Planned median ms | Paired baseline/planned ratio |
|---|---:|---:|---:|
| Horizontal pencil | 23.73–25.14 | 20.87–21.52 | 1.12–1.20x |
| Diagonal pencil | 27.06–28.07 | 27.12–29.01 | 0.97–1.00x |
| Smooth brush | 35.50–36.10 | 32.46–33.61 | 1.07–1.10x |
| Smooth eraser | 38.02–39.21 | 22.61–23.59 | 1.66–1.73x |

Entire canonical output archives and every ZIP member match byte-for-byte between
implementations, checked before timing and after each session. `correctness.json`
and compressed outputs retain this evidence. `summary.json` records reported RSS:
median values are approximately 8–9 MiB and diagonal peak RSS is higher for the
candidate (10.13 versus 9.84 MiB). No general memory reduction is claimed.

## Reproduction and preserved controls

Build each frozen commit with the same CPU-only command in separate target
directories, retaining binary identities and feature graphs. Decompress
`fixtures/input.pcnd.gz`. For each fixture JSON, run each frozen executable as:

```text
paintcraft-cli --json apply input.pcnd --operations CASE.json --output OUTPUT.pcnd --overwrite --expect-input-sha256 3a9fadd2b402440be058dfd0324299c7f031d510596f9f18dbf8da604ff746ec
```

Use Hyperfine `--shell=none --warmup 3 --runs 20 --export-json FILE` for both
commands, with independent output paths. Compare complete canonical outputs.
Original exports are byte-preserved in `raw_exports/*.gz`; compact JSON has
identical parsed values and both hashes are recorded in `raw-exports-sha256.json`.

The [mixed-build pilot](../paint-plan-cli-2026-10-21/README.md) is preserved and
excluded because its candidate came from a desktop/CLI combined build. Earlier
slower replay prototypes remain in the [kernel decision](../paint-plan-rows-2026-10-20/DECISION.md).
These CLI ratios include geometry, history and file IO, and must not be replaced
by the much larger synthetic partial-coverage replay ratio. Sustained native
input/display tails, scheduling interference, other platforms/workloads and
whole-batch/effect/metadata admission remain unqualified.
