# Native decoder isolation helper

Complete native installations use this executable for explicitly qualified Studio
JPEG XL, JPEG XR and HEIF conversions. Desktop, CLI and MCP consumers share the
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

Native build record `BUILD_V4` includes `STDIN_PARENT_LEASE1` and
`JXR_PRIVATE_SOURCE1`; binary protocol 2 is unchanged. The supervisor retains its
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
physical RSS, allocator overhead, stacks, mappings and direct native allocations.
Optional codec experiments require their own allocation qualification. The parent
reserves this fixed child ceiling plus captured source, bounded output and its
existing verification/document preparation declarations; reported peak never
justifies charging a smaller ceiling. A response peak is a preparation snapshot,
not process-exit RSS. There is no OS filesystem or network sandbox claim.

Release builds compile desktop, CLI and helper together under the lockfile.
Portable archives place helper and manifest beside the application. macOS nests
them in `Contents/MacOS`; Linux packages use `usr/bin` and
`usr/share/paintcraft`. MSI installs helper and manifest beside the desktop
executable. Packagers reject missing, stale, wrong-target or mismatched embedded
records without executing foreign binaries. macOS ad-hoc signing is reflected in
the final manifest SHA; it is not Developer-ID signing or notarization.

Schema 2 `qualified_import_variants` lists the three exact native variants above.
Standalone copied executables and helper-free installs fail closed. Packages
containing the helper include `JPEGXR_LICENSE.txt`, reproducing the selected
JPEG XR dependency's actual required Microsoft notice rather than relying on its
different Cargo license label.

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
JXRLib encode/decode specimens. Actual copied-installation CLI/MCP and headless
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
