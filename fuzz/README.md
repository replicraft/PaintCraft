# Parser fuzzing

This standalone, unpublished Cargo workspace exercises existing parser entrypoints.
It adds no product dependency or file-format capability. `replay` builds without
libFuzzer; the seven coverage targets require the `fuzzing` feature and pinned
`libfuzzer-sys = 0.4.13`. `Cargo.lock` is checked separately from the product lock.

| Target | Entry and admitted work |
| --- | --- |
| `avif` | AVIF container/family assessment; no AV1 C decoder |
| `heif` | Bounded HEIF container inspection; no HEVC decoder |
| `paint` | PAINT container and admitted typed raster-layer decode |
| `pdn` | PDN envelope/NRBF graph and admitted typed raster-layer decode |
| `dcx` | Complete DCX directory admission, then first and last selected PCX pages |
| `radiance` | Bounded Radiance header and encoded RGBE packet inspection; no float decoding |
| `exr` | Bounded EXR metadata/window/chunk inspection; no decompression or float decoding |

No target publishes an editor document or writes a source file. The PAINT/PDN
parsers include their existing bounded decompression and raster paths; this is
not a native decoder sandbox or a test of every supported image codec.

## Historical five-target deterministic replay

The 2026-10-26 stable build completed 4,112 deterministic cases: each of sixteen
manifest seeds unchanged, followed by 256 mutations. Replaying the same manifest-listed bytes/mutation
stream, sorted by seed name per target, after adding `invariants-fail-v1` produced the same counts below without
an invariant finding. The original earlier logs remain a prior-policy baseline.
Mutation choices are
truncation, byte XOR, one-byte append and byte replacement; the initial PRNG state
is `0x5041494e54435241`, reset once per target process. The exact sorted seed list
and its order matter. This is replay/mutation smoke, not coverage-guided fuzzing.

| Target | Cases | Accepted | Returned errors |
| --- | ---: | ---: | ---: |
| AVIF | 1,028 | 109 | 919 |
| HEIF | 771 | 299 | 472 |
| PAINT | 771 | 18 | 753 |
| PDN | 771 | 23 | 748 |
| DCX | 771 | 98 | 673 |
| Total | 4,112 | 547 | 3,565 |

## Seven-target inspector extension

The reviewed inventory now has 24 seeds and 6,168 deterministic cases. The first
sixteen manifest rows are unchanged. The original five targets remain first in
`TARGETS`, with their seed-name sorting, PRNG initialization and mutation policy
unchanged; their five replay receipt lines matched the historical 4,112-case
record exactly. Eight existing independent fixtures are appended:

| Target | Exact seed-name order |
| --- | --- |
| Radiance | `legacy-zero-low-digit-257.hdr`, `literal-2x2.hdr`, `modern-rle-8x2.hdr`, `zero-modern-run.hdr` |
| EXR | `float-rgba-zip.exr`, `half-rgb-none.exr`, `half-rgba-rle.exr`, `mixed-rgba-zip.exr` |

Radiance fixtures are the owned scalar-reference inputs in the tracked
`radiance-fixtures` directory, including one explicit zero-run rejection. EXR
inputs are the independently generated/reopened official OpenEXR fixtures in
`exr-fixtures`, covering HALF/FLOAT, RGB/RGBA, RLE/ZIP and mixed channel types.
Original fixture provenance remains in those directories; `seeds.json` verifies
all source lengths and SHA256 values before staging/replay and after replay.

| Target | Cases | Accepted | Returned errors |
| --- | ---: | ---: | ---: |
| AVIF | 1,028 | 109 | 919 |
| HEIF | 771 | 299 | 472 |
| PAINT | 771 | 18 | 753 |
| PDN | 771 | 23 | 748 |
| DCX | 771 | 98 | 673 |
| Radiance | 1,028 | 95 | 933 |
| EXR | 1,028 | 193 | 835 |
| Total | 6,168 | 835 | 5,333 |

The new unchanged-seed-only control completed 24 cases: twenty accepted and four
rejected, including three accepted Radiance and all four accepted EXR seeds.
Five Rust controls cover invariant classification, positive/malformed inputs,
input and pixel denial, and callback-budget denial on an otherwise valid Radiance
inventory. Five Python controls cover dependency identity, reviewed per-target
seed counts, provenance tampering, and refusal of replay receipts lacking positive
controls, correct counts or the invariant policy. Stable locked tests/strict Clippy and all seven
optional wrapper checks passed. Both new optional wrappers also built and linked
without being executed. No optional HDR/EXR/HEIF decoder features are enabled in
this isolated graph; neither new target performs pixel decoding. EXR acceptance
covers bounded structure, not the validity of decompressed samples.

This is deterministic mutation smoke, with no invariant finding in this stream.
It is not coverage-guided, sanitizer, original-application compatibility or
complete parser reliability/security qualification. The historical 4,112-case
results above and prior guided AVIF status below have not been relabeled.

An earlier original-seed-only control had sixteen cases, thirteen accepted and
three rejected, with at least one accepted seed for every target. Some seeds are
intentional unsupported/negative profiles. An accepted parse does not establish
compatibility with an original application.

Reproduce the historical five streams from the repository root using the product
toolchain and an explicit private target directory:

```sh
python3 fuzz/check_lock.py
python3 fuzz/seed.py
paintcraft_fuzz_target_dir=/private/tmp/paintcraft-fuzz-replay-target
cargo build --manifest-path fuzz/Cargo.toml --locked --bin replay \
    --target-dir "$paintcraft_fuzz_target_dir"
python3 - "$paintcraft_fuzz_target_dir/debug/replay" <<'PY'
import json
from pathlib import Path
import subprocess
import sys
seeds = json.loads(Path("fuzz/seeds.json").read_text())["seeds"]
for target in ["avif", "heif", "paint", "pdn", "dcx"]:
    paths = sorted(str(Path("fuzz/corpus") / target / row["name"])
                   for row in seeds if row["target"] == target)
    subprocess.run([sys.argv[1], target, "256", *paths], check=True)
PY
```

This deliberately selects only manifest seeds, excluding any later generated
coverage corpus. `seeds.json` records original repository paths, lengths and
SHA256 hashes; each original fixture directory retains its own provenance and
independent-oracle limitations. `seed.py` checks hashes/lengths and stages exact
copies into ignored `corpus/`; it does not invent inputs or copy source assets
into version control. `check_lock.py` compares every parser/transitive package's
version, source and checksum with the product lock. Only the harness and its
`libfuzzer-sys`/`arbitrary` tooling dependencies are exempted from that comparison.

## Resource and error policy

The harness refuses inputs larger than 256 KiB and declares a 16,384-pixel decode
allowance. AVIF/HEIF and both float inspectors receive that pixel allowance and a
callback budget. PAINT/PDN
also receive explicit container, nesting, metadata, object/item/layer and decoded
storage limits in `src/lib.rs`. The callback returns a resource error after 4,096
checks. These are logical parser policies, not a hard wall-clock or global heap/
RSS limit. They cannot interrupt work between callbacks. DCX directory inspection
has no callback; the harness's 256 KiB input limit applies, and its first/last page
decodes receive the pixel cap. The replay argument caps mutations at 100,000 per
seed; the recorded run used 256.

The shared `classify` helper now makes `ErrorCode::InvariantViolation` a panic
finding for replay and all seven libFuzzer targets. Focused unit controls prove
that this error cannot be counted as an ordinary rejection. Other returned errors
remain rejections without a complete error-code whitelist; these counts are not
a claim that every rejection is benign. Harness panics are not swallowed. The
unchanged 4,112-case replay passed under the new policy, but this is not parser
reliability/security qualification or coverage completion.

Initial five-target ordinary checks completed: two Rust controls, two dependency-identity controls,
locked stable replay build/test/Clippy, formatting, and a stable `cargo check`
compiling all five optional wrappers with `--features fuzzing --bins`. That optional
check did not run libFuzzer or enable sanitizer instrumentation. Lock controls
prove same-version/source checksum changes and missing checksums are rejected.

```sh
python3 -m unittest discover -s fuzz -p 'test_*.py'
cargo fmt --manifest-path fuzz/Cargo.toml --check
cargo test --manifest-path fuzz/Cargo.toml --locked
cargo clippy --manifest-path fuzz/Cargo.toml --locked --all-targets -- -D warnings
cargo check --manifest-path fuzz/Cargo.toml --locked --features fuzzing --bins
cargo build --manifest-path fuzz/Cargo.toml --locked --features fuzzing --bin radiance --bin exr
python3 fuzz/check_lock.py
```

## Coverage-guided status

One preliminary AVIF run completed with private cargo-fuzz 0.13.2 and
`1.101.0-nightly (76456a688 2026-10-25)`. The underlying invocation requested
`-runs=5000 -max_len=262144 -timeout=2 -rss_limit_mb=512` and used `-fork=2`;
its recorded aggregate was 16,166 iterations, coverage 910, features 1,230 and
reported OOM/timeout/crash counters 0/0/0. Forked aggregate iterations are not the
requested per-job run count. macOS symbolizer/atos warnings were retained.
This initial smoke predates the invariant-error classifier; no new guided run
was performed after that change. Guided qualification for the remaining four original targets and the new
Radiance/EXR inspectors remains pending, as do all-target runs under the invariant
policy and broader returned-error classification.
This is not an all-target coverage or sanitizer completion claim. The initial
smoke did not capture a clean source/build snapshot before compiling, so it is
not presented as a hermetic or independently reproducible build. An assistant
tooling interruption was not a parser finding.

The nightly and cargo-fuzz installation used private Cargo/Rustup/tool directories;
the product toolchain was not changed. No tool installation is performed by this
package. For reviewed future guided runs, use that pinned tool version and the
`fuzzing` feature, and keep build metadata locked: cargo-fuzz 0.13.2 does not forward
`--locked`, so `locked-cargo.sh` must be placed as `cargo` in a private PATH wrapper
directory with `PAINTCRAFT_FUZZ_CARGO` set to the absolute real Cargo binary. It
forwards build/metadata with `--locked`; run `check_lock.py` before and after.
Record the actual target, corpus hashes, tool/compiler versions, command line,
limits, complete output and terminal status. A time/RSS flag is a process-run
policy, not proof of bounded parser allocation or cancellation latency.

`target/`, `corpus/`, `artifacts/` and `coverage/` are ignored locally. Keep original
failure logs and crash inputs; reduce a finding and add a focused regression before
claiming it fixed. Do not silently relabel incomplete or failed runs as passing.


## Stable CI configuration

The Linux branch of `.github/workflows/ci.yml` is configured to check nested
formatting, locked stable Clippy/package controls and dependency identities, then
build `replay` into the existing root `target` directory. `replay_smoke.py` verifies
all 24 checked-in source lengths/hashes, selects manifest-listed seeds sorted
by seed name per target, checks the returned invariant-policy/count receipts and
requires all 6,168 cases and the reviewed per-target seed counts. It reads the original sources directly; extra ignored
corpus files cannot affect ordering and no corpus staging is needed. No nightly,
libFuzzer execution, sanitizer instrumentation or tool installer is added to CI.

Equivalent steps were validated locally using the existing private target cache;
that differs from Linux CI's root-target reuse. The workflow is configured only:
no GitHub Actions execution of these new steps is claimed here.

For the current seven-target, 6,168-case inventory, use the manifest-driven helper:

```sh
python3 fuzz/replay_smoke.py --replay /path/to/private/target/debug/replay
```
