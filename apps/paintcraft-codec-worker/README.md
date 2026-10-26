# Native decoder isolation helper

Complete native installations use this executable for explicitly qualified Studio
JPEG XL, JPEG XR, HEIF and AVIF conversions. Desktop, CLI and MCP consumers share the
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
| 5 | Reduced-still identity sRGB8 AVIF 4:4:4 with optional straight monochrome alpha | `native-avif-srgb8-worker-v1`, V5 or V6 required |
| 6 | Explicit BT.709/sRGB8 AVIF 4:4:4 or sited 4:2:0 with optional straight alpha | `native-avif-sdr-ycbcr8-worker-v1`, V6 required |

All profiles are bounded to 4,194,304 pixels. A selected library compiling does
not admit its other profiles or grant an encoder capability.

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
alpha/deep precision and other unqualified JXR forms remain refused. No JPEG XR
or HEIF writer is provided.

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

Three finite native contracts are recognized. `BUILD_V4` qualifies JXL/JXR/HEIF;
`BUILD_V5_AVIF` additionally requires the pinned static decoder token
`AVIF_DAV1D154_ALLOC1_ASM0_8BIT1` and exact four-variant manifest. The private
`BUILD_V6_AVIF_SDR` adds the reconstruction token and fifth variant described
below. Identity AVIF requires V5 or V6; SDR requires V6; other codecs accept
all three verified contracts, with no retry after integrity failure. All include `STDIN_PARENT_LEASE1` and `JXR_PRIVATE_SOURCE1`; binary
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
explicitly enable `paintcraft-codec-worker/avif-sdr-codec`. Meson 1.12.1 and Ninja
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
or four for V5, or five for the opt-in V6 build.
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

Normal builds include the V6 identity and qualified SDR AVIF decoder and require the native C compiler,
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
both decoder notices. Source installation requires the complete V6 output.
V4 and V5 return a canonical unsupported response for code6. Browser and Classic
reading remain unavailable; archive checks do not qualify six-target execution.
