# Bounded advanced browser DDS export

These source-bound preparation helpers serve the exact
`web-dds-texture-worker-v1` Studio flat-export variant. Hosts must admit that
route before preparation. Native `PCDEC002` and browser JXL decoder2 remain
unchanged; legacy/default browser DDS keeps its old route and capacity.

The browser profile admits at most 4,194,304 canvas pixels, each dimension at
most 65,536, eight layers, 8,388,608 logical layer pixels, 24 MiB of resident
padded tiles and 64 KiB of source metadata. Data is kept or refused; no source
layer or encoded mip is truncated. This is not the largest possible browser
profile. Large thin populated rasters can exceed the padded-tile bound even
when their canvas area fits.

`SourcePlan::capture` inspects immutable document data before allocation. The
host obtains the JS service reservation before `snapshot`, whose existing raw
PCND writer uses a fixed 32 MiB sink. No rendering or encoding runs there. The
SHA binds every byte, including all layers and metadata. The Worker reuses the
PCND parser with caller layer/tile limits before pixel inflation. At most 96
256×256 tiles are admitted; titles allow 4,096 characters/16 KiB and layer names
1,024 characters each. Bounded records/strings fit within the conservative
8 MiB parser-metadata allowance. Existing PCND wrappers retain their own policy.

The Worker calls the document DDS facade once. Its composite drops before
actual encoded top-mip decoding reuses that RGBA allowance; no second render or
encode supplies the preview. The nearest-sampled thumbnail is at most 192×192
and retains encoded sample transfer. Main UI converts Linear8 thumbnail RGB to
sRGB for display, preserving alpha. All generated mip bytes remain in the
artifact. Preview omission is not an export loss; actual facade losses alone
require fresh explicit consent.

The declared reservation is **640 MiB** of conservative logical payload slots.
With S = 32 MiB raw input, E = 24 MiB + 4 KiB encoded output, and P = 192×192×4
thumbnail bytes, the inventory is:

`256 MiB + 5S + 5E + 24 MiB source tiles + 8P + 1 MiB metadata`.

It totals 562.14453125 MiB and leaves 77.85546875 MiB margin. This includes the
whole Worker linear ceiling, main Rust snapshot and JS/frozen/digest/request
slots, result/digest/main Rust/Blob/handoff slots, and preview/upload payloads.
Some slots deliberately do not coexist or already lie within the Worker cap;
they are not discounted. This is not aggregate browser RSS, GPU memory, or an
allocator/stack guarantee. The Worker separately refuses a conservative
pre-render payload quote above 192 MiB, including input, tiles, facade workspace,
parser allowance, response/preview and decode scratch. The module still declares
the existing 256 MiB hard linear-memory maximum.

The host retains complete request binding and response through one full Rust
validation, actual visible preview, consent and Blob URL retirement. Settings
edits retire preview/approval, drain a known old operation, discard its result
and prepare the replacement. Unknown completion, active cancellation and stale
service owner quarantine the declaration until realm teardown. A completed
refusal releases; `terminate()` is not a join. Successful URL revocation proves
URL retirement, not persistence or browser-memory reclamation.

Local qualification used actual headless Chrome 156 on Apple Silicon. Real
controller jobs exercised 2048×2048 BC1/BC2/BC3 High-effort Box filtering and
full-chain RGBA8, with complete response identity to native Rust validation.
An eight-layer source exercised 24 MiB padded tiles, 8,387,589 logical pixels,
and maximum accepted title/name character counts. Its highest observed Worker
linear-memory high-water was 117,833,728 bytes; the 256 MiB growth refusal was
checked separately. These are bounded conformance/deadline observations, not
throughput benchmarks or universal allocator peaks. The 1MP predecessor proof
is retained separately.

Production desktop and 390×740 mobile-viewport tests used real project pickers,
Worker previews, consent, downloads and trusted touch controls. Whole BC1 Box
bytes matched the independent frozen specimen and differed from Nearest.
Linear8 bytes matched the validated reference. Before/after project copies
were identical. Real 4MP Worker cancellation and public service-owner invalidation
kept the charge quarantined, published no DDS and left project recovery usable.
The public owner-protocol test is distinct from host tests replacing the editor
document/title. Physical phones/tablets, other browser engines and hardware
pressure input are separate qualification scopes.

The optional `dds_web_qualification_inputs.rs` test emits fresh source/request/
native reference specimens into an explicit new directory. It does not replace
the independent compressed golden or promise a general BC color-error bound.
Node `dds-profile.test.mjs` checks Rust/JS caps and the quote formula. Local
browser receipts retain source patches, asset hashes, attempts and screenshots.

## Version-one byte layout

Integers are big-endian. Reserved bytes and inactive settings must be canonical.
The request is a 128-byte header followed by the declared PCND snapshot:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSQ01` |
| 8..10 | protocol 1 |
| 10..12 | zero |
| 12..16 | snapshot byte length |
| 16..24 | width, height |
| 24..65 | owner: session UUID, state UUID, u64 epoch, Studio=3 |
| 65..81 | nonce UUID |
| 81..113 | source SHA-256 |
| 113..121 | canonical eight-byte settings |
| 121 | source flags: flatten, hidden RGB |
| 122..128 | zero |

Settings contain encoding (RGBA8/BC1/BC2/BC3), transfer (sRGB/Linear8), alpha
(straight except opaque BC1), actual level count, filter (Nearest/BoxStored),
effort (Normal/High), and two zero bytes. RGBA effort canonicalizes to Normal;
one-level filtering to Nearest. Decoding refuses noncanonical wire values.

The response is a 288-byte header followed by DDS bytes, thumbnail RGBA and
optional refusal text, in that order:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSR01` |
| 8..10 | protocol 1 |
| 10..12 | status, zero |
| 12..24 | captured width, height, snapshot length |
| 24..28 | encoded length |
| 28..32 | thumbnail width, height (u16 each) |
| 32..73 | exact captured owner |
| 73..89 | nonce |
| 89..121 | source SHA-256 |
| 121..129 | settings |
| 129..193 | encoded SHA-256, thumbnail SHA-256 |
| 193 | five known loss bits |
| 194..198 | quantized RGB sample count |
| 198..263 | version byte length, 64-byte zero-padded ASCII version |
| 263..265 | bounded UTF-8 refusal length |
| 265..266 | captured source flags |
| 266..288 | zero |

Successful replies must match source/owner/nonce/options/version, both result
digests, exact thumbnail geometry, actual DDS header/complete-chain inventory
and canonical loss/statistics constraints. The host does not independently
re-render to prove the worker's quantization count. Canonical completed refusals
carry no DDS, preview, loss or quantization data. Malformed/unknown completion
is not a release acknowledgement: JS integration retains the existing
fail-closed quarantine rules and never treats `terminate()` as a join.
