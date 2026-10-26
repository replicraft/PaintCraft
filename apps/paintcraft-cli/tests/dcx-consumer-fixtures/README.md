# Production single-page DCX qualification

The real CLI and MCP production routes import a captured PCND document/session,
then export exactly one DCX composite page using explicit RGB24, exact indexed8 or
requested quantized256 PCX mode. The six files recorded in `imagemagick.json` were
produced by `tests/dcx_export.rs` through the actual binary and stdio tool router,
not by calling the typed backend directly. All modes retain the six authored RGB
samples exactly in this four-color specimen; indexed modes retain the first-seen
four-color palette and raw indices `[0,1,0,2,3,1]`, with a black unused suffix.

`verify.mjs` passes the entire files to untouched ImageMagick 7.1.2-32 Q8 from
`@imagemagick/magick-wasm` 0.0.44. It verifies one page, dimensions 3×2, DPI 72/72,
all samples, all 256 palette entries and indices. Node was 24.10.0. The package is
qualification-only, not a product dependency. Exact package archive provenance,
SHA512 integrity, wrapper/Wasm SHA256 and the broader mixed multipage qualification
are in `crates/services/paintcraft-formats/tests/dcx-fixtures/writer/README.md`.
The script verifies the untouched wrapper/Wasm hashes before loading them.

From the repository root, with that verified archive extracted privately:

```sh
mkdir /private/tmp/paintcraft-dcx-production-owned
PAINTCRAFT_DCX_CONSUMER_OUTPUT=/private/tmp/paintcraft-dcx-production-owned \
    cargo test -p paintcraft-cli --test dcx_export --offline --locked
node apps/paintcraft-cli/tests/dcx-consumer-fixtures/verify.mjs \
    /private/tmp/paintcraft-dcx-production-owned /path/to/private/oracle/package
```

The Rust tests check actual loss refusal/consent and white matte, explicit mode
requirements, unknown losses, unchanged source bytes and MCP document/dirty state,
Classic refusal, hardlinked sources, and successful unrelated copies before/after
renaming the retained native source. A separate private staged-route regression
creates an alias at the publication barrier and proves refusal without changing
source bytes or leaving staged files. Deterministic checks cancel during preparation
and at the final publication barrier, preserving session semantics and the old
destination with no staged residue. The actual stdio client also cancels a large
export RPC immediately and confirms unchanged state/output; it does not assert
that cancellation arrived after rendering started. MCP uses the existing RPC
lifetime and request token through conversion, staged chunks and publication;
filesystem identity protection remains a separate coarse callback. The registry
matrix excludes every other
frontend/platform/operation/variant, including generic Open and preserving Save.

This qualifies the native headless Studio fresh-copy route only. No desktop or
browser DCX export controls, multipage document model, layers-as-pages conversion,
or additional Classic stock capability is claimed. Typed multi-page output remains
available separately inside the formats backend.

## Whole-operation delivery baseline

`benchmarks/tools/run_dcx_exports.py` reuses the deterministic raster-codec PNG
fixtures and existing optimized-build receipt checks. It freezes a release CLI,
imports four source projects once, records exact fixture/project SHA256 and state
IDs, and independently checks every DCX pixel before and after measurements with
`verify_benchmark.mjs`. Indexed cases also check all palette entries and indices.
Each timed execution includes Python wrapper startup, fresh CLI startup, captured
project read/admission, composition, PCX/DCX encoding, durable publication, JSON
completion and output unlink. The Python startup cost is included, not subtracted.
It is a delivery baseline, not a codec microbenchmark or optimization comparison.

The 2026-10-26 arm64 macOS run used two warmups and twenty measured runs per case.
All eighty measured exits succeeded; output destruction completed. Median elapsed
milliseconds were 137.3 (64² palette/indexed), 168.6 (1000×486 sparse/RGB), 176.1
(1000×486 dense/RGB) and 156.4 (1000×486 palette/indexed). All samples are retained,
including a sparse-case 1403.7 ms observation. External user load, power and thermal
state were uncontrolled. Agent builds were explicitly idle during measurements.
The separate first-in-session observations occurred after correctness preflight;
neither those nor the repeated fresh processes imply cold OS/filesystem caches.
No comparison, speedup or responsiveness conclusion follows from these numbers.

Raw Hyperfine exports, first observations, independent reports, frozen binary,
source/build receipts, hashes and complete sample output remain ignored under
`benchmarks/results/dcx-delivery-2026-10-26`. Source HEAD at build start was
`0de7a373`; run HEAD was `2c8393bd`, with no committed changes to the CLI/formats/
foundation/editing/Cargo source paths between them. The retained observation
records dirty paths and this scope; the build is not claimed hermetic. Driver
unit controls exercise cleanup after success/failure/timeout and refusal to remove
an already existing sample file. Each CLI subprocess has a 30s tooling timeout;
no signal/cancellation latency or product deadline is measured.

Prepare an optimized CLI and a fresh fixture directory using the existing
`raster_codecs` bench (`PAINTCRAFT_BENCH_FIXTURES`), then run the driver first
without `--execute` to inspect its plan. The fixture manifest must match its PNG
hashes. Existing qualified fixture directories can be reused without regeneration.
After coordinating an idle build window:

```sh
paintcraft_dcx_build_head=$(git rev-parse HEAD)
printf '%s\n' "$paintcraft_dcx_build_head" > /private/tmp/dcx-cli-build-head.txt
cargo build -p paintcraft-cli --release --locked --message-format=json > /private/tmp/dcx-cli-build.jsonl
python3 benchmarks/tools/run_dcx_exports.py run \
    --cli target/release/paintcraft-cli --build-jsonl /private/tmp/dcx-cli-build.jsonl \
    --build-source-head "$paintcraft_dcx_build_head" --fixtures /path/to/raster-codec-fixtures \
    --oracle-package /path/to/private/oracle/package \
    --output benchmarks/results/dcx-delivery-new --execute --agent-builds-idle
python3 -m unittest discover -s benchmarks/tools/tests -p test_dcx_exports.py
```

The driver creates a new result directory and refuses reuse; it never installs
build tools, flushes OS caches or changes product dependencies. Independent oracle
work and fixture/project generation stay outside timed samples.
