# Integer JPEG XL conversion

As of 2026-10-29, Studio has a new explicit JPEG XL import path for admitted
8-bit and 16-bit integer sources. The editor and PCND copy remain straight
sRGB RGBA8. This is precision conversion, not 16-bit editing or preserving
JPEG XL Save. Ordinary Open and Classic JPEG XL reads remain denied.

Current source and next shipping builds select V10 on all six native targets:
Windows x64/ARM64, macOS Intel/Apple Silicon and Linux x64/ARM64. Normal Cargo
helper builds enable both `jxr-codecs` and `jxl-integer-codec` by default. Native
adoption still verifies the exact helper manifest and requires explicit Studio
import. The [V10 qualification run](https://github.com/replicraft/PaintCraft/actions/runs/40889561591)
at `5eb0831f` passed all six targets, including mandatory installed App writer
controls with 30 scenarios and float conversion-identity checks.

Published 0.1.1 artifacts and the tag are unchanged: those downloads retain V7
on five targets and V8 on Apple Silicon, with their earlier strict 8-bit reader.
Browser integer import has local production-build evidence. This source-policy
promotion does not claim a new deployed release, native C execution on every
runner, original-application compatibility or signing/notarization.

## Pixel and source contract

The shared isolated import profile admits only metadata-free, static, identity-oriented,
original-profile sRGB with 8-bit or 16-bit integer color. Optional alpha must be
unnamed, straight, full-resolution integer alpha with the same depth as color.
Mixed color/alpha depths, floating alpha, associated/downsampled alpha, named or
additional extra channels, ICC/custom profiles, linear/HDR color, animation,
previews, discarded container metadata and other unqualified forms are refused.
The limits remain 64 MiB encoded input and 4,194,304 pixels.

The new protocol codec tag is **9**. It chooses one integer-family decode;
unsupported sources do not trigger a second decoder attempt or a fallback to a
different profile. Existing codec tag 2 remains strict 8-bit and refuses 16-bit
sources. The selected decoder remains the `jxl` crate from jxl-rs; the separate
lossless RGBA8 export continues to use `zune-jpegxl`.

Eight-bit output retains exact straight RGBA, including hidden RGB beneath zero
alpha. Each 16-bit component independently becomes:

```text
output_u8 = (u32(source_u16) + 128) / 257
```

Integer division gives nearest rounding without half-value ties. The same rule
applies to R, G, B and alpha, including hidden RGB; there is no premultiplication,
matte, exposure or color transform. Counts report RGB components and alpha values
that differ from the reconstructed `output_u8 * 257` samples. The
`depth-16-reduced-to-8` loss is mandatory for every 16-bit source, even when both
counts are zero, because the editable copy no longer retains 16-bit precision.
Eight-bit sources have no depth loss and do not receive an empty consent modal.

The fixed **96-byte Report** binds source depth, geometry, source SHA-256, exact
output RGBA SHA-256 and both change counts to its versioned conversion policy.
Parents validate framing, reserved fields, payload length, geometry and both
digests. Consent is bound to the SHA-256 of the canonical report bytes, rather
than to an unverified label or a mutable preview.

## Frontends and isolation

| Route | Exact contract | Publication |
| --- | --- | --- |
| Native V9 | `native-v9-jxl-integer8-16-srgb8-parent-lease1`, schema 2 | Adds `native-jxl-integer-srgb8-worker-v1` to the V7 import inventory; declares no JXR writer |
| Native V10 | `native-v10-jxl-integer8-16-srgb8-jxr-rgb8-lossless-parent-lease1`, schema 3 | Retains the V8 JXR writer/own-reader inventory and adds the same integer JPEG XL import |
| Browser Studio | `web-jxl-integer-srgb8-worker-quarantine-v1` | Uses the built isolated Worker/Wasm and the same codec 9 report; native helper manifests are not browser capabilities |

Studio retains the previous document while preparing a 16-bit conversion. The
actual converted preview must have been visible in an earlier frame before the
translated loss checkbox can be enabled. Open remains disabled until explicit
consent. Adoption checks the current document/session/frontend/draft owner and
report digest again, then creates an unbound dirty copy. Cancellation, source
changes, stale owners or a changed assessment leave the current document,
binding and source guards intact. Native renamed/hardlinked input aliases remain
protected from overwrite; PCND Save/reopen retains the converted raw samples.

Native work uses the existing bounded, supervised helper and parent lease, with
a 256 MiB requested heap ceiling and the unchanged worker deadline. That does
not establish a hard process RSS cap or a complete hostile-code sandbox. Browser
Wasm has a 256 MiB linear-memory maximum. A validated completed refusal can release
its logical reservation; uncertain cancellation, stale completion, traps or
malformed responses retain quarantine until reload. PCND download remains
available. Worker termination does not prove synchronous execution stop or
physical memory reclamation.

## Decoder provenance

The selected crates.io `jxl` 0.8.0 snapshot has a three-file metadata exposure
patch: `src/api/data_types.rs`, `src/api/codestream_parser/image_info.rs` and
`src/headers/extra_channels.rs`. It exposes actual source extra-channel depth,
dimension shift and name presence so admission does not infer alpha properties
from main-channel depth or output pixels. Decoder arithmetic, parsing, color
transforms, dependencies and feature defaults are unchanged.

All 179 upstream archive files and the original BSD-3-Clause license are retained.
The exact archive checksum, patch, inventory, license hashes and narrow exceptions
for unchanged upstream long files are enforced by the
[vendor verifier](../scripts/verify_jxl_decoder_vendor.py). The
[snapshot notes](../vendor/jxl-0.8.0-paintcraft1.README.md) describe the complete
delta. This source verification does not complete the broader dependency/security
and release-license reviews.

## Evidence and reproduction

These results qualify the stated fixtures and paths, not all JPEG XL files or
all operating systems and browsers.

| Evidence | Result and scope |
| --- | --- |
| [Shared independent fixtures](../crates/services/paintcraft-formats/tests/jxl_worker16.rs) and [worker tests](../apps/paintcraft-codec-worker/tests/native_jxl_integer.rs) | Independent libjxl source samples, exhaustive integer rounding, exact report/payload binding, strict unsupported profiles and old-tag refusal |
| [Installed native App controls](../apps/paintcraft/src/app/importing/native_codec/jxl_integer_installed_tests.rs) | Actual immutable V9 and V10 helpers on macOS ARM, headless App rendering, English/Japanese first-visible-frame consent, exact hidden RGB/alpha, PCND round trips, aliases and stale/cancel/refusal preservation; final runs passed in 125.44 s and 128.38 s |
| [Installed CLI/MCP controls](../apps/paintcraft-cli/tests/installed_codec_worker/jxl_integer.rs) | Actual producer-installed CLI/MCP payloads on macOS ARM: V9 passed 12 ordinary tests plus seven exact selectors; V10 passed 12 ordinary tests plus nine exact selectors, retaining AVIF/float baselines and V10 JXR goldens/guards. CLI/test bytes, source hashes and installed files remained unchanged. This was scoped development-checkout evidence, not the clean six-target candidate CI |
| [Production Worker proof](../apps/paintcraft-web/scripts/codec-production-browser.mjs) and [Studio UI proof](../apps/paintcraft-web/scripts/jxl-production-browser.mjs) | Built Astro assets in headless Chrome 156.0.8078.49 on macOS ARM: 14 Worker checks and seven UI groups passed. All nine downloaded PCND artifacts passed an independent Rust raw-pixel oracle, including hidden RGB/alpha and failure-preserved documents |
| [Candidate orchestration](../scripts/packaging/qualify_native_codecs.py) | Finite V8/V9/V10 selection, real package/install checks, exact nonzero worker/CLI/App fixture gates and clean-source checks. The [six-target V10 run](https://github.com/replicraft/PaintCraft/actions/runs/40889561591) at `5eb0831f` passed all six targets with mandatory installed App writer and float-identity fixtures; the earlier `025099c8` V9/V10 matrix remains historical evidence |

The browser results include 8-bit automatic adoption, 16-bit preview/cancel/retry
and explicit adoption, PCND download/reopen, strict unsupported profiles and
quarantine after uncertain failure. Host Rust and JavaScript controls, production
Wasm/Astro asset checks and final all-target Clippy for the frontend and isolated Worker also passed. These checks
remain separate from actual browser execution evidence. Native test source
snapshots and run logs retain the tested App executable's historical digest;
the Cargo output binary was subsequently replaced, so its exact bytes are not
claimed as retained. Immutable helper binaries remain retained.

Run shared conversion and provenance controls from the repository root:

```sh
python3 scripts/verify_jxl_decoder_vendor.py
cargo test --locked -p paintcraft-formats --test jxl_worker16
cargo test --locked -p paintcraft-formats --test native_jxl_gate
```

For full native candidate qualification, use a clean committed checkout on the
matching native host. This Apple Silicon example builds, packages, installs and
executes the required actual fixtures without changing shipping policy. V9 is an
explicit compatibility qualification; V10 is the current selected profile:

```sh
python3 scripts/packaging/qualify_native_codecs.py --candidate v9 --target aarch64-apple-darwin --output /absolute/new-v9-evidence
python3 scripts/packaging/qualify_native_codecs.py --candidate v10 --target aarch64-apple-darwin --output /absolute/new-v10-evidence
```

To rerun the focused App control against an explicit same-version helper:

```sh
PAINTCRAFT_JXL_INTEGER_HELPER=/absolute/path/to/v9-or-v10-worker cargo test --locked -p paintcraft --lib importing::native_codec::jxl_integer_installed_tests::copied_jxl_integer_helper_runs_actual_app_review_and_failure_guards -- --exact --ignored --nocapture --test-threads=1
```

For the complete installed CLI/MCP baseline and exact integer/writer selectors,
use a verified installation whose CLI matches the same-source qualification
build. V9 excludes writer selectors; V10 retains them:

```sh
python3 scripts/packaging/qualify_cli_helper.py --profile v10 --target aarch64-apple-darwin --installation /absolute/path/to/verified-installation
```

For production browser proofs, install the declared pnpm dependencies and build
from `apps/paintcraft-web`. `PLAYWRIGHT_MODULE` points to an installed Playwright
ES module with Chromium available; each proof writes a fresh attempt directory:

```sh
pnpm install --frozen-lockfile
pnpm build
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs CODEC_EVIDENCE_DIR=/absolute/worker-evidence node scripts/codec-production-browser.mjs
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs CODEC_EVIDENCE_DIR=/absolute/ui-evidence node scripts/jxl-production-browser.mjs
```

The first source-policy promotion historically exercised normal debug builds on macOS ARM: a
plain Cargo default produced the exact V9 record, and the target-selected recipe
produced V10. Fresh real CLI archives and installations passed 12 ordinary tests
plus seven exact selectors for V9, and 12 plus nine for V10. All 40 test functions
passed through 20 bounded listing/test processes with unchanged source, test and
installed payload hashes. These warm canonical-target debug checks are distinct
from the optimized clean six-target candidate CI; they do not qualify a new
complete desktop release or signing/notarization. Raw receipts are retained in
local ignored `docs/validation/jxl-integer-promotion` evidence.

That first promotion selected JPEG XR writing only on Apple Silicon. The later
six-target V10 run supplied the mandatory actual fresh-writer App and float-identity
gates, enabling the current all-target V10 selection. Legacy-import App checks
alone do not substitute for those gates. Portable expected writer files were
independently C-qualified; C did not run on every runner.

Remaining gates include broader original files and reference applications,
cross-browser/device evidence, security/license review and hostile-resource
qualification. HDR/ICC/metadata/animation, additional channel/depth profiles,
preserving JPEG XL Save and high-depth editing remain outside this conversion.
No new 0.1.1 release or whole-format/roadmap completion is implied.
