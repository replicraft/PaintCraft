# Parser fuzzing

This standalone, unpublished Cargo workspace exercises existing parser entrypoints.
It adds no product dependency or file-format capability. `replay` builds without
libFuzzer; the eleven target wrappers require the `fuzzing` feature and pinned
`libfuzzer-sys = 0.4.13`. `Cargo.lock` is checked separately from the product lock.

## Stock decoder extension

Three targets call actual stock parser/decoder APIs. The parser functions do
not access files, launch helpers/processes, contact a network, or use a GPU.
Both libFuzzer wrappers and stable replay leave product panics uncaught;
returned invariant errors remain findings through the existing classifier.

| Target | Actual entrypoints and admitted work |
| --- | --- |
| `raster` | Restricted PNG/JPEG/GIF/BMP/TIFF dispatch through `image::decode_with_pixel_limit`; PNG additionally exercises admitted original-profile/resolution retention, matching samples, source identity, notice inventory and lease release |
| `tiff_pages` | Full-source inventory, first and last eligible page (at most two), sealed assessment round trip and exact source/page/descriptor/output/loss binding |
| `icons` | Full directory preflight and first/last explicit entry (at most two), including indexed XOR samples hidden by AND masks |

All targets retain the harness 256 KiB source and 16,384-pixel limits. The selected
TIFF API uses the existing 4,096-checkpoint allowance. The stock raster and icon
public APIs do not expose an interruptible decoder callback; these bounds are
not hard time, allocation/RSS, cancellation-latency, or sandbox guarantees.
The harness introduces no new product format limits or decoder dependencies.

The extra PNG path has a separate 16 KiB semantic-metadata and charged-storage
allowance. Valid sources with larger profiles can therefore be rejected by this
target even when ordinary decoding succeeds. Its callback checks do not make the
selected opaque PNG pixel call interruptible. Both accepted captures and refused
captures must release their metadata charge. The reviewed 29-source manifest and
historical streams remain unchanged; new runs bind the updated harness source.

The retention follow-up passed 15 Rust controls, strict checks with all optional
wrappers enabled, and linked all eleven wrappers plus replay. The separate
6,168 / 7,453 / 3,598 raw-PCND / 3,598 checksum-aware streams retain their previous
counts. A stable libFuzzer smoke retained all 44 original seed copies (17 stock
raster sources plus 27 retained-PNG fixtures) through 2,000 runs. This build has
no sanitizer or coverage instrumentation; the smoke is not coverage-guided or
ASan qualification. Source identities and outputs are retained locally under
`docs/validation/png-retention-fuzz/`.

`stock-seeds.json` separately binds 29 exact source lengths/hashes. The parent
fixture directories retain independent PNG/JPEG/GIF/BMP/TIFF oracle provenance.
Four derived sources have explicit parent SHA256 values and reproducible
edits in `fixtures/provenance.json` and `fixtures/generate_stock.py`: malformed
EXIF IFD offset, missing GIF LZW EOI after complete samples, valid TIFF Orientation
6 and invalid Orientation 9. Orientation 6 is a normalized positive control for
ordinary and selected-page import. Its exact 2×3 straight RGBA output is checked
against the independent 3×2 libtiff fixture oracle, including hidden RGB. Invalid
9, previous assessment versions and previous browser request framing still refuse.

The initial pre-orientation-support run added these independent streams. Its
negative orientation expectation and exact source snapshot remain historical
evidence; they do not constrain the subsequently qualified valid feature.

| Target | Seeds | Cases | Accepted | Returned errors |
| --- | ---: | ---: | ---: | ---: |
| Raster | 16 | 4,112 | 1,060 | 3,052 |
| Selected TIFF pages | 5 | 1,285 | 334 | 951 |
| Icons | 6 | 1,542 | 130 | 1,412 |
| Additional total | 27 | 6,939 | 1,524 | 5,415 |

After TIFF orientation normalization and its V2 report/protocol landed, the
ordinary and selected Orientation 6 predicates became positive, and invalid
Orientation 9 was added to both streams. The integration replay measured:

| Target | Seeds | Cases | Accepted | Returned errors |
| --- | ---: | ---: | ---: | ---: |
| Raster | 17 | 4,369 | 1,244 | 3,125 |
| Selected TIFF pages | 6 | 1,542 | 512 | 1,030 |
| Icons | 6 | 1,542 | 130 | 1,412 |
| Current additional total | 29 | 7,453 | 1,886 | 5,567 |

`replay_smoke.py` first executes and validates the historical seven-target
6,168-case stream unchanged, then calls the separate stock replay. The original
manifest, sorted seed order, mutation policy and receipt stay intact. This
initial extension passed 8 Rust controls, 7 Python controls, locked stable
tests/Clippy, all ten optional wrapper checks, and linked the three new wrappers.
The orientation follow-up passed all four stock Rust controls, seven Python
controls, strict Clippy, the updated stock replay and rebuilt all three new
wrappers with `CARGO_INCREMENTAL=0` in the canonical workspace target directory.
The historical seven-target stream was not rerun during this scoped follow-up.

Each new linked libFuzzer driver also completed 2,000 bounded runs from its exact
staged seeds. These stable binaries reported missing sanitizer symbols and no
coverage instrumentation. This is driver/mutation smoke, not coverage-guided or
sanitizer qualification. The previously installed private nightly/cargo-fuzz
tools are no longer present; no installation or product toolchain change was
made for this slice. Preserve the historical guided evidence below separately.

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

## PCND native-project extension

`pcnd` is a separate pure target for the public
`pcnd::read_with_metadata_admission_checked` reader. It accepts at most 256 KiB
input, 16,384 canvas pixels, eight layers, 131,072 staged pixels (two nominal
256 × 256 resident tiles), and a separately charged 16 KiB source attachment.
Huge default-only sparse layer dimensions are not confused with materialized
pixel cost. Checks share the existing 4,096-checkpoint allowance.

Accepted documents undergo bounded structural checks and canonical raw
write/read semantic comparison through a 1 MiB private sink,
including exact source metadata and hidden pixels. Original and restored
attachments use separate 16 KiB budget realms; this does not charge two live
snapshots against one attachment quota. The two admitted tiles consume at most 512 KiB raw; strings, headers and the
attachment are bounded by the 256 KiB input. The separate 1 MiB output allowance
covers both plus canonical framing; exceeding it is an invariant finding, never
a silently skipped roundtrip.
The pure entrypoint has no filesystem, helper, editor or GPU work. Test and replay
runners read fixtures externally. These are logical bounds, not hard RSS or time
limits, sanitizer instrumentation or a hostile-input sandbox.

`pcnd-seeds.json` and `pcnd-fixtures/provenance.json` independently bind fourteen
seeds, ten positive and four negative. They include four independently serialized
PCND1/2 sources, recomputed-outer-SHA unknown-version/ICC-extent/inner-digest
negatives, RGBA-runs/Zstd/LZ4/runs-LZ4 tile bodies, huge default-only geometry and
two/three-resident-tile admission boundaries. See the generator's exact parent
hashes/recipes. Zstd seed generation uses installed Zstandard 1.5.7; product
codec versions and validators are unchanged.

The additional deterministic stream has exactly **3,598 cases**. The original
seven-target 6,168-case and three-stock-target 7,453-case receipts and inventories
remain separate and unchanged. The main replay helper runs the three raw streams and the separately reported
checksum-aware stream described below;
the new stream can also run alone:

```sh
CARGO_INCREMENTAL=0 cargo test --locked --manifest-path fuzz/Cargo.toml --target-dir target
CARGO_INCREMENTAL=0 cargo build --locked --manifest-path fuzz/Cargo.toml --target-dir target --bin replay
python3 fuzz/pcnd_replay_smoke.py --replay target/debug/replay
```

Random mutations usually fail the outer SHA before deeper parsing. Valid
compressed seeds and integrity-recomputed negative controls deliberately reach
those deeper paths; a short stable libFuzzer driver run does not establish
coverage or replace a future instrumented campaign. No nightly/tool installation
or alternate Cargo target directory is required for this extension.

A **separate checksum-aware PCND stream** mutates each seed's body and recomputes
only its outer SHA-256 before invoking the same parser. It does not repair inner
attachment digests, encoded tile data or semantic fields. Its 3,598 cases are
reported separately from the raw PCND stream and the historical/stock streams.
This reaches container boundaries behind the integrity prefilter; it is still
stable mutation smoke, not coverage-guided qualification. Generated input batches
are bounded, live under ignored validation storage and are removed on exit.

```sh
python3 fuzz/pcnd_semantic_smoke.py --replay target/debug/replay
```

The actual `pcnd` libFuzzer wrapper also executes **both** the raw parser and the
explicit checksum-aware route on each input. The latter makes one fallible,
input-bounded copy and hashes in checked 64 KiB chunks under the same checkpoint
allowance. Only the outer 32-byte checksum is replaced. The extra harness-only
SHA-256 dependency is pinned to the product's already selected `sha2` 0.11.1;
no product parser API or checksum validation is widened.

The initial local PCND qualification used Darwin/arm64 and the repository's
selected stable toolchain. Fourteen Rust controls (five PCND-specific) and nine
Python inventory/replay controls passed. The unchanged historical and stock
streams passed separately, followed by raw PCND 3,598 cases (28 accepted) and
checksum-aware PCND 3,598 cases (601 accepted). All eleven wrappers compiled.
The actual stable PCND driver completed 2,000 inputs with both routes enabled,
reporting 31 MiB RSS and warnings for absent sanitizer/coverage instrumentation;
it reduced its working corpus to one byte. This is mutation smoke, **not** an
instrumented coverage result or exhaustive hostile-parser qualification. Original
fourteen seed files and their hashes remain preserved separately from that
working corpus. The ignored `docs/validation/pcnd-fuzz/` receipt binds exact
sources, commands, logs, lock edge and executed driver/replay digests.
