# Native codec isolation helper

Complete native installations use this executable for explicitly qualified Studio
JPEG XL, JPEG XR, HEIF, AVIF and HDR/EXR conversions. Desktop, CLI and MCP consumers share the
same verified native dispatch, captured-source guards and resident document
adoption. This does not enable generic image Open, Classic readers, HEIF writing
or browser use of this executable. The separately qualified Studio browser JPEG
XL Import uses `web-jxl-srgb8-worker-quarantine-v1` and a WebWorker, with its own
transport and qualification; it does not use a native helper manifest.

## Qualified native sample profiles

| Worker code | Meaning | Document route |
| --- | --- | --- |
| 1 | Native gray8 Y and optional opacity, original CICP retained | Internal HEIF diagnostics only; no guessed sRGB interpretation |
| 2 | Metadata-free straight sRGB RGBA8 JPEG XL | `native-jxl-srgb8-worker-v1` |
| 3 | Strict unsigned RGB8 JPEG XR, T.832 default-sRGB policy after refusing embedded color contexts | `native-jxr-srgb8-worker-v1` |
| 4 | Explicitly declared sRGB8 HEIF color and optional straight alpha | `native-heif-srgb8-worker-v1` |
| 5 | Reduced-still identity sRGB8 AVIF 4:4:4 with optional straight monochrome alpha | `native-avif-srgb8-worker-v1`, V5, V6, V7 or qualified V8 required |
| 6 | Explicit BT.709/sRGB8 AVIF 4:4:4 or sited 4:2:0 with optional straight alpha | `native-avif-sdr-ycbcr8-worker-v1`, V6, V7 or qualified V8 required |
| 7 | Explicit Radiance RGBE-to-SDR conversion with bound settings/report | `native-hdr-sdr8-worker-v1`, final V7 or qualified V8 required |
| 8 | Explicit OpenEXR-to-SDR conversion with bound settings/report | `native-exr-sdr8-worker-v1`, final V7 or qualified V8 required |
| 9 | Fixed fresh-writer unsigned RGB8 JPEG XR long-header subset | `native-jxr-writer-rgb8-worker-v1`, optional qualified V8 only |

The general decoder ceiling is 4,194,304 pixels; AVIF, HDR/EXR and writer-compatible
JPEG XR have tighter profile limits. A selected library compiling does not admit
its other profiles or grant an encoder capability.

HEIF color requires matching explicit NCLX and coded VUI declarations: primaries
1, transfer 13, full-range identity GBR 4:4:4, or matrix 1 YCbCr 4:4:4/4:2:0 at
full or limited range. The qualified 4:2:0 siting is type 0, with bilinear
reconstruction, outermost-sample extension and round/clamp to RGB8. Odd 4:4:4
edges are qualified; odd 4:2:0, shifted conformance crops and derived color images
are refused. ICC, unlabelled color, HDR/deep samples, premultiplication, display
transforms, extra image/metadata items and unknown import properties are refused.
Every admitted coded image has one qualified base-layer still IDR. The selected
`heic-decoder` 0.1.1 SPS parser and frame decoder execute only inside the helper.

The helper crosschecks actual decoded dimensions, precision, plane extents,
chroma sampling, color metadata and alpha against admission before converting.
The parent separately validates the response model/extent and binds its bounded
container assessment to the exact captured source SHA before allocating document
tiles. The parent does not invoke the selected SPS parser; that stays isolated.

YCbCr conversion requires `heif-color-reconstruction` consent, describing range
expansion, rounding/clipping and, for 4:2:0, interpolation. Nonempty primary or
alpha item names additionally require `heif-item-names-not-retained`: receipt
retention does not mean those labels are stored in PCND. An unnamed full-range
identity container has neither loss. Source NCLX/range/chroma/alpha declarations,
item names and SHA remain explicit in conversion receipts. Alpha is exact and
RGB checks include alpha-zero pixels. Converted documents are unsaved PCND copies,
not source-preserving HEIF saves.

JXR requires `jxr-resolution-not-retained` consent, including the normative
96-DPI default when resolution is absent or zero. Receipts distinguish raw source
resolution bits from effective density and inferred defaults. ICC, metadata,
alpha/deep precision and other unqualified JXR forms remain refused. HEIF writing
remains unavailable. Optional qualified V8 adds the fixed JPEG XR writer below.

Code5 AVIF requires explicit matching AV1/container declarations: profile-1 8-bit
full-range identity GBR 4:4:4, primaries 1 and transfer 13. Its optional alpha is
same-size profile-0 full-range 8-bit monochrome with CICP 2/2/2 and a straight
auxiliary link. GBR planes are reordered exactly; coded RGB at alpha zero is
retained. Nonempty names require `avif-item-names-not-retained` consent; unnamed
identity imports have no invented color-conversion loss. Names, exact source
SHA, geometry and coded/container declarations remain in receipts. Code6
separately qualifies the explicit SDR policy below and always requires
`avif-color-reconstruction` consent. ICC, HDR/deep precision, grids, transforms,
premultiplication and browser reads remain refused. See [AVIF qualification](../../docs/AVIF_QUALIFICATION.md).

## Dispatch, ownership and supervision

The host locates only a same-build helper relative to its own installation, never
through PATH or a format-selected executable. Platform verification checks bounded
schema 2 JSON, declared OS/architecture, the referenced build record, streaming
SHA and retained executable/manifest identities before and after dispatch. These
are observed-byte consistency checks, not publisher authentication, an atomic
filesystem snapshot or elimination of the path-based spawn race. Installation
files require the same trust as the application.

Protocol 2 captures immutable source bytes, SHA-256, a random request nonce and
Session/state/epoch/frontend ownership. Every response must match those values,
application version, codec, sample model and exact bounded payload length. Partial
or failed results do not publish. Prepared results retain their transient charge
until adoption/discard; adopted document tiles carry their existing resident
leases through history and snapshots. GUI review is tied to a captured result,
an actually visible prior-frame preview, explicit loss consent and current
editing state. Source changes, cancellation or stale ownership refuse adoption.
Saving converted output cannot replace the captured source, including renamed
files and hardlink aliases.

The injected runtime supervisor uses an explicit absolute executable path,
bounded I/O and a deadline. It kills/reaps the child and joins both pipe threads
before returning, including cancellation and callback unwind. The helper starts
the existing single-thread Rayon pool; no new renderer or thread pool is added.
The trusted child must not delegate inherited pipes or spawn descendants: the
supervisor controls its exact child, not an arbitrary process tree.

Finite native contracts are recognized explicitly. `BUILD_V4` qualifies JXL/JXR/HEIF;
`BUILD_V5_AVIF` adds the pinned static decoder and exact four-import manifest;
`BUILD_V6_AVIF_SDR` adds SDR reconstruction and the fifth import; final V7 adds
the two explicit float conversions. Optional qualified V8 adds fixed JPEG XR
encoding and its writer-compatible reader through a distinct schema 3 contract.
Identity AVIF accepts V5/V6/V7/V8, SDR accepts V6/V7/V8, and float conversion
accepts final V7/V8; writer-compatible JPEG XR requires qualified V8. These are
finite verified records, with no version-prefix acceptance or integrity-error retry.
All include `STDIN_PARENT_LEASE1` and `JXR_PRIVATE_SOURCE1`; binary
protocol 2 is unchanged. The supervisor retains its
sole stdin writer after the exact request. EOF, I/O failure or an extra byte makes
the helper watchdog exit without allocation (status 76). This is parent-liveness
evidence, not a synchronous parent-death join or a job-object sandbox. Forwarded
writer handles would invalidate its sole-writer premise.

JXR alone uses a parent-owned random private backing file, byte-checked against
the captured request before and after selected decoder use. The shared Launch
service owns it until supervised reap, then explicitly closes it. No host has a
duplicate scratch adapter or selected shared-temp convenience decoder. Parent
death can orphan private temporary files; no hard disk quota or Windows ACL
parity is claimed.

## Memory boundary and packages

The default qualified paths use a fixed 256-MiB live requested Rust heap ceiling
from process startup. Checked accounting covers allocation, zeroing and realloc;
refusal exits directly with status 75, mapped to `ResourceLimit`. This excludes
physical RSS, allocator overhead, stacks, mappings and unaccounted native allocations.
The AVIF feature statically builds pinned dav1d 1.5.4 with a small audited allocation
shim: decoder context, scratch and picture requests, including headers/padding,
use that same heap once. C ABI failures cannot unwind. Libc internals and thread
stacks remain outside this requested-allocation boundary. Threads and frame delay
are fixed to one; the pixel limit is set before decoding. The parent
reserves this fixed child ceiling plus captured source, bounded output and its
existing verification/document preparation declarations; reported peak never
justifies charging a smaller ceiling. A response peak is a preparation snapshot,
not process-exit RSS. There is no OS filesystem or network sandbox claim.

Release builds compile desktop, CLI and helper together under the lockfile and
explicitly enable `paintcraft-codec-worker/float-codecs`. Meson 1.12.1 and Ninja
1.13.2 build the pinned source without a system-library fallback. On Windows,
Meson uses `b_vscrt=mt` exactly when Rust target features include `crt-static`;
otherwise it uses `md`. Release flags select the static mode, while normal and
source builds follow their actual Rust target configuration. Six-target
execution remains unverified until CI runs.
Portable archives place helper and manifest beside the application. macOS nests
them in `Contents/MacOS`; Linux packages use `usr/bin` and
`usr/share/paintcraft`. MSI installs helper and manifest beside the desktop
executable. Packagers reject missing, stale, wrong-target or mismatched embedded
records without executing foreign binaries. macOS ad-hoc signing is reflected in
the final manifest SHA; it is not Developer-ID signing or notarization.

Schema 2 `qualified_import_variants` lists exactly three native variants for V4
or four for V5, five for V6, or seven for final qualified V7. Optional qualified V8
uses schema 3 with exactly eight imports and one qualified export.
Standalone copied executables and helper-free installs fail closed. Packages
containing the helper include `JPEGXR_LICENSE.txt`, reproducing the selected
JPEG XR dependency's actual required Microsoft notice rather than relying on its
different Cargo license label. `DAV1D_LICENSE.txt` reproduces the pinned decoder's
COPYING notice. The CLI archive and source installers keep helper, manifest and
notices together and verify them before publication.

## Qualification

Owned HEIF fixtures use independent FFmpeg/libx265 encoding, exact FFmpeg and
libheif native-plane readback, libheif container/alpha decoding, and an independent
exact-rational RGB reference. The declared PaintCraft bilinear policy matches
FFmpeg within 1 RGB code value; identity RGB and alpha are exact. libheif's 4:2:0
RGB reconstruction differs substantially on these patterns, even with its forced
bilinear option. Those original outputs remain negative evidence, not a widened
rounding tolerance or universal application-parity claim. See the
[HEIF color fixtures](../../crates/services/paintcraft-formats/tests/heif-color-fixtures/README.md).
JPEG XL uses owned independent `cjxl` specimens; JPEG XR uses owned original C
JXRLib encode/decode specimens. AVIF native planes and assembled opaque/alpha
RGBA match pinned independent libaom 3.15.2 exactly, with separate writer-loss
evidence. Actual V5 Studio tests cover 12 English/Japanese cases; installed
CLI/MCP tests cover consent, PCND reopen, cancellation and renamed source aliases. Actual copied-installation CLI/MCP and headless
native UI tests exercise PCND reopening, loss consent, source aliases,
cancellation, stale owners and helper failures. They do not replace real
Windows/Linux installer/device checks, broad adversarial corpora or additional
original-application/color/depth qualification.

The hidden developer diagnostics remain separate from document admission:

```sh
paintcraft-cli --json codec-inspect source.jxl --codec jxl
paintcraft-cli --json codec-inspect source.heic --codec heif
paintcraft-cli --json import source.heic --frontend studio --output converted.pcnd \
    --allow-loss heif-color-reconstruction,heif-item-names-not-retained
```

The `--codec heif` diagnostic deliberately remains native gray code 1. Color
conversion is selected through the explicit import route, never by relabeling
those gray diagnostic samples sRGB.

## Build and compatibility checks

Normal builds include the final V7 HDR/EXR conversion helper and its legacy decoders and require the native C compiler,
Meson 1.12.1, Ninja 1.13.2 and Python 3.12 or newer:

```sh
cargo build --locked -p paintcraft-cli -p paintcraft-codec-worker
cargo test --locked -p paintcraft-codec-worker --tests
cargo test --locked -p paintcraft-cli --test installed_codec_worker -- --include-ignored
```

The explicitly feature-free build retains V4 compatibility. Its installed tests
skip the four AVIF cases. V5 identity-only compatibility additionally enables
`avif-codec` with defaults disabled. Restore the normal helper when finished:

```sh
cargo test --locked -p paintcraft-codec-worker --no-default-features --features avif-codec --tests
cargo test --locked -p paintcraft-cli --test installed_codec_worker -- --include-ignored --skip avif_sdr::
cargo test --locked -p paintcraft-codec-worker --no-default-features --tests
cargo test --locked -p paintcraft-cli --test installed_codec_worker
cargo build --locked -p paintcraft-codec-worker
```

The complete CLI source installer builds and installs both native binaries,
manifest and notices with `sh scripts/packaging/install_cli.sh --source` (or
`./scripts/packaging/install_cli.ps1 --source` on Windows). These local commands
do not replace actual release execution on all six targets.

## SDR decoder qualification

Normal V6/code6 builds preserve code5 identity behavior and use the distinct
`native-avif-sdr-ycbcr8-worker-v1` profile. They declare source/native planes,
i32 staging, RGBA and converter work before C decoding. This accepts only
explicit 8-bit matrix-1/sRGB 4:4:4 or 4:2:0 CSP_VERTICAL/COLOCATED, full or limited
range, with optional same-size straight alpha. The
[owned SDR fixtures](../../crates/services/paintcraft-formats/tests/avif-sdr-fixtures/README.md)
retain independent native-plane and rational RGB evidence. Unknown chroma
position, other matrices, HDR and derived images remain refused.

The exact `BUILD_V6_AVIF_SDR` record adds the pinned allocation and
`AVIF_SDR709_YUV444_420_CSP12_WORK1` tokens and a five-variant set. Installed
native Job/adoption verifies that contract, source-bound assessment and actual
reconstruction/name consent. Source-family selection validates captured bytes
before admission and helper discovery; it never retries a decoder after failure.
Packaging and private-prefix validators retain exact V4/V5/V6 compatibility and
both decoder notices. Source installation requires the complete final V7 output.
V4 and V5 return a canonical unsupported response for code6. Browser and Classic
reading remain unavailable; archive checks do not qualify six-target execution.


## Private float conversion qualification

The initial opt-in `float-codecs` qualification built a private V7 helper for HDR and EXR conversion while normal builds remained V6. That historical snapshot declared five variants and did not promote installers. Its original logs/hash remain historical evidence; final production manifests use the separate exact seven-variant descriptor below.

Code 7 (Radiance) and code 8 (OpenEXR) accept exactly 32 canonical settings bytes
followed by at most 16 MiB of original source, with a 1,048,576-pixel allowance.
The unchanged PCDEC002 header binds the entire envelope. The source assessment
separately binds the original image bytes, source metadata, settings, numeric
statistics and output digest. Success carries the application version, a fixed
96-byte report and straight RGBA8. Old codec responses retain their original
layout. Unknown options, reserved bytes, bad lengths, quota declarations and
response owner/version/dimension mismatches fail closed.

The worker reuses the existing precision conversion, optional bounded HDR/EXR
decoders, 256 MiB requested-allocation ceiling, parent stdin lease and kill/reap
supervisor. This ceiling does not measure physical RSS or stack usage. Parent
report reconstruction checks metadata, bounded statistics and canonical digests;
it does not independently decode original floating-point samples. Filesystem
freshness, editor ownership, resident adoption and user consent remain caller
responsibilities, not properties of the binary envelope.

Qualification commands (serial execution avoids conflating concurrent latency
with codec correctness):

```sh
cargo test -p paintcraft-codec-worker --features float-codecs --offline --locked -- --test-threads=1
cargo test -p paintcraft-codec-worker --no-default-features --offline --locked --test native_float --test native_avif --test native_avif_sdr --test native_heif -- --test-threads=1
cargo test -p paintcraft-codec-worker --no-default-features --features avif-codec --offline --locked --test native_float --test native_avif --test native_avif_sdr --test native_heif -- --test-threads=1
```

The V7 suite passed 40 tests with eight explicitly ignored external/qualification
tests. Its float cases compare all 17 independent reference scenarios (12
successful conversions and five refusals), reconstruct the parent assessment,
and check malformed envelopes, response binding, quotas, cancellation and
deadlines. Lease-EOF termination is exercised after a complete request without
claiming a precisely observed mid-decode phase. The shared parent-death test
holds its sole stdin writer outside `Child::wait`, proves no premature EOF, then
observes child termination after killing the parent. Earlier evidence that kept
stdin inside `Child` was insufficient because `wait` closes that handle; the
corrected fixture is separate from the production watchdog implementation.


## Final V7 qualification descriptor and distribution

Normal builds now enable `float-codecs`, retaining the immutable V7 build record, PCDEC002 protocol 2 and manifest schema 2. The exact profile is `native-v7-hdr-exr-sdr8-settings32-report96-16mib-1mp-parent-lease1`. Its ordered seven import variants are the V6 five followed by `native-hdr-sdr8-worker-v1` and `native-exr-sdr8-worker-v1`. Native gray remains an internal operation, not an eighth document import variant. Historical private V7 manifests are refused; no profile ordering admits unknown versions or retries integrity failures.

Final V7 packages require JPEGXR, dav1d and `HDR_EXR_LICENSES_V1.txt` notices. Exact legacy V4/V5/V6 payloads retain their original two notices. CLI archive extraction bounds the manifest and metadata before validating the exact profile-dependent file set, then verifies executable records/hashes/notices before atomic installation. Source installation explicitly builds `float-codecs` and requires final V7. Compatibility-only V6 builds use `--no-default-features --features avif-sdr-codec`; V5 uses `--no-default-features --features avif-codec`, and V4 uses `--no-default-features`.

The final descriptor does not alter exposure, alpha, negative-value, tone-mapping, source-space or report semantics. Original private qualification logs are not final-distribution evidence. Final consumer/package proof and native OS release checks are recorded separately.

Local final-descriptor qualification on Apple silicon passed the default worker matrix (40 tests, eight ignored; test-build helper `80aecb8e9db50085716a7de69f7ff840b016f5fa0d58978155f647bff40e840a`). A separate actual optimized source build and canonical archive/private-prefix installation produced helper `24a014c300a3b8050b50afa4e71d1391c2949446b12bae7583a2ca48017ce29d` and CLI `33dccc52d90d34be83c5f8f18b05c73588107c0f7a9314503a4beb44d657e1a8`. Against those installed optimized binaries, CLI/MCP passed all seventeen independent scenarios plus source/cancellation/rollback guards; native-parent preparation passed twelve exact RGBA/PCND cases and five refusals; legacy discovery passed all six existing operations and unknown/private/forged-manifest refusals. The worker test-build hash and distributed optimized-helper hash are recorded separately.

Actual local ad-hoc signing of a separate optimized helper copy also passed canonical post-sign manifest verification. This does not establish a complete signed GUI bundle, DMG, MSI, Linux package integration or execution on the other five native targets. Those remain release-workflow/OS gates; no release or tag is implied by local qualification.


## Optional qualified V8 JPEG XR fresh copies

Normal builds and default packages still use V7. Explicit `jxr-codecs` builds add
profile `native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1`, an exact
nonprivate V8 build record, and schema 3 with eight imports and one export.
Private V8 records remain refused. `PCENC001` requests and `PCJXR001` replies
are distinct from decoded RGBA frames; the existing supervisor, allocator cap
and parent lease are reused.

Exact native Studio `native-jxr-rgb8-lossless-worker-v1` enables the desktop
**Export image** choice, CLI `export-jxr` and MCP `export_jxr`. The fixed lossless
profile accepts an opaque RGB8 composite, at most 1,048,576 pixels and 65,536
per side, with 64 MiB encoded output. It never supplies a matte or silently
removes alpha. Actual flatten/hidden-color losses require review; source document,
history, dirty state, title, source identities and native Save binding remain intact.
The real encoded result and admission stay retained through picker/review and the
final guarded staging publication. No portable/browser export choice is added.

`native-jxr-writer-rgb8-worker-v1` imports the distinct bounded fresh long-header
profile through the same sealed native coordinator. Classification happens before
launch; it never retries another reader after failure. Existing short-header
imports keep their established profile. Print-resolution loss still needs consent,
including the normative 96-DPI interpretation of positive-zero stored fields.

An immutable optimized Apple Silicon helper `529ed471936258cba3ce1e72444c98bc5f9f0c2848b808552ca213dc3f2c716c`
passed independent original-C fresh/retained writer checks and actual CLI/MCP
consumers. A debug headless App paired with that release helper passed twenty-four
English/Japanese cases: fourteen export/guard cases, two actual fresh 4096×1
export/import/PCND round trips and eight legacy JPEG XR review cases. Four UI
exports matched every RGB byte through the independently pinned C decoder; its
successful exit is required. Alias/stale owner/draft/tab/frontend/helper changes
preserve sources and existing destinations. Native file-picker completion is a
headless seam, not automated OS-dialog interaction. Earlier failed fixture runs
remain recorded separately in `docs/validation/jxr-native-ui-2026-10-26`.

This does not complete broader JPEG XR profiles, original Windows application
interoperability, default/release promotion or all six native target runtimes.
There is no generic Open, Classic/browser reader, preserving JPEG XR Save or
16-bit editor claim. The exact registry variants remain preflight operations.
