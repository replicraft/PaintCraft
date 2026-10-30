# Bounded advanced browser DDS export

These source-bound preparation helpers serve the exact
`web-dds-texture-worker-v3` Studio flat-export variant. Hosts must admit that
route before preparation. Native `PCDEC002` and browser JXL decoder2 remain
unchanged; legacy/default browser DDS keeps its old route and capacity.

The browser profile admits at most 4,194,304 canvas pixels, each dimension at
most 65,536, eight layers, 8,388,608 logical layer pixels, 24 MiB of resident
padded tiles and 64 KiB of title/layer/control metadata. An existing source
attachment retains at most the document model’s 8 MiB wire allowance under a
separate charged realm; it is never stripped from the PCND snapshot. Data is kept or refused; no source
layer or encoded mip is truncated. This is not the largest possible browser
profile. Large thin populated rasters can exceed the padded-tile bound even
when their canvas area fits.

`SourcePlan::capture` defaults to refusing metadata omission and filter baking.
`capture_with_policies` receives independent typed choices against the exact frozen
document and DDS flat-export target. The old metadata-only wrapper grants no Bake
authority. Any nonempty filter stack, including disabled or hidden intentions,
requires explicit Bake and records `filters-baked`. A loss-name list cannot grant
that authority. The immutable snapshot retains the editable PCND3 stacks. The
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

## Historical version-one qualification

The following receipts qualify the previous protocol and resource profile. They
remain historical. Version-two metadata-bound checks are recorded separately below.

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

## Historical version-two metadata qualification

The version-two production build passed a real Studio flow from an independent
9×7 PNG with original ICC/pHYs retained in PCND2. Chrome 156 on Apple silicon
exercised RGBA8 sRGB output with a full nearest mip chain, explicit omission,
visible preview, revocation/recheck with a fresh nonce, Cancel and download.
Pillow 12 independently decoded exact top-level source pixels. Project bytes,
title and dirty state stayed unchanged.

Six fresh browser realms additionally altered the real outgoing request's
digest, extent, intent, target, version or retained-memory charge. Each refused
without encoded/preview output and preserved the project. Understated admission
returned the exact typed resource-limit result. Actual Worker termination is
recorded without treating it as proof of physical memory reclamation.

The local receipt is
`docs/validation/metadata-omission/advanced-browser-final-freeze.json`, SHA256
`1072302286a9edb882b22ea60059071059f379c2ec0a920652c89c33faaf2e7f`.
It preserves executed drivers, input/oracle provenance and all 25 production
asset identities, shared with the independent generic PNG privacy proof. This
qualifies the stated case, not every DDS model, browser or physical device.

## Version-three byte layout

Integers are big-endian. Reserved bytes and inactive settings must be canonical.
The request is a 224-byte header followed by the declared PCND snapshot:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSQ03` |
| 8..10 | protocol 3 |
| 10..12 | zero |
| 12..16 | snapshot byte length |
| 16..24 | width, height |
| 24..65 | owner: session UUID, state UUID, u64 epoch, Studio=3 |
| 65..81 | nonce UUID |
| 81..113 | source SHA-256 |
| 113..121 | canonical eight-byte settings |
| 121 | source flags: mask1 flatten, mask2 hidden RGB, mask4 any filter intentions |
| 122 | independent bake intent: 0 Refuse, 1 Bake |
| 123..128 | zero |
| 128..160 | canonical filter inventory SHA-256 |
| 160..224 | unchanged source metadata context below |

Settings contain encoding (RGBA8/BC1/BC2/BC3), transfer (sRGB/Linear8), alpha
(straight except opaque BC1), actual level count, filter (Nearest/BoxStored),
effort (Normal/High), and two zero bytes. RGBA effort canonicalizes to Normal;
one-level filtering to Nearest. Decoding refuses noncanonical wire values.

The response is a 384-byte header followed by DDS bytes, thumbnail RGBA and
optional refusal text, in that order:

| Bytes | Field |
|---|---|
| 0..8 | `PCDDSR03` |
| 8..10 | protocol 3 |
| 10..12 | status, zero |
| 12..24 | captured width, height, snapshot length |
| 24..28 | encoded length |
| 28..32 | thumbnail width, height (u16 each) |
| 32..73 | exact captured owner |
| 73..89 | nonce |
| 89..121 | source SHA-256 |
| 121..129 | settings |
| 129..193 | encoded SHA-256, thumbnail SHA-256 |
| 193 | seven loss bits: flatten1, hidden RGB2, block4, mip8, linear16, metadata32, filters64 |
| 194..198 | quantized RGB sample count |
| 198..263 | version byte length, 64-byte zero-padded ASCII version |
| 263..265 | bounded UTF-8 refusal length |
| 265..266 | captured source flags |
| 266 | exact captured bake intent |
| 267..288 | zero |
| 288..320 | exact captured filter inventory digest |
| 320..384 | exact request metadata-context echo |

Successful replies must match source/owner/nonce/options/version, both result
digests, exact thumbnail geometry, actual DDS header/complete-chain inventory
and canonical loss/statistics constraints. The host does not independently
re-render to prove the worker's quantization count. Canonical completed refusals
carry no DDS, preview, loss or quantization data. Malformed/unknown completion
is not a release acknowledgement: JS integration retains the existing
fail-closed quarantine rules and never treats `terminate()` as a join.

## Source metadata context and copy authorization

Offsets are relative to request byte160 or response byte320:

| Bytes | Field |
|---|---|
| 0..16 | actual frozen document UUID |
| 16..48 | attachment digest, or all zero when absent |
| 48 | bit0 attachment present; bit1 explicit omission intent; no other bits |
| 49 | actual target: 1 DDS, 2 MSP; DDS decoder requires 1 |
| 50 | operation 1 = flat export only |
| 51 | zero |
| 52..56 | exact attachment wire bytes including its 192-byte header |
| 56..64 | bounded conservative retained-storage allowance |

Absent metadata requires zero digest/length/charge. Present metadata requires
explicit intent, at most 8 MiB wire bytes, and charge between wire extent and
8 MiB + 64 KiB. SourcePlan derives these values from the actual attachment; a
loss string or an “allow all” list cannot mint the private Policy witness.
The existing 640 MiB reservation also covers this additional retained charge:
the previous inventory plus its maximum 8.0625 MiB remains below that cap.
The Worker quotes the charge before PCND capture, denies understated storage
before copying, and verifies document UUID, actual digest/extent and charged
admission against the decoded snapshot. It keeps the real PCND attachment.

Only after validating the actual DDS target does the Worker prepare a copy with
omission. Its new bit32 acknowledges `source-metadata-omitted`; no-metadata plus
explicit intent echoes that intent but sets no omission loss. The response binds
all 64 context bytes as well as existing source/owner/options/digest fields. Old
version-one and version-two requests and replies are rejected without fallback.

The host retains the original SourcePlan Policy and Frozen copy. Wire owner state
names that actual copy (including text-draft pixels); the live session owner and
Frozen.current independently guard later publication. Unchecking the metadata
choice revokes old preparation/preview, rather than reusing an earlier approval.
The omission is separately explicit, so unrelated format losses still require
the existing consent while this one reason needs no duplicate checkbox.


## Filter inventory and allocation admission

The versioned `filter_bake::Inventory` digest binds document/state UUIDs, canvas,
ordered root/layer IDs, each cached ordered stack digest, visibility, opacity,
placement, finite raster dimensions and implicit default pixels. The host captures
it once with the original sealed Bake policy. Changing a stack with unchanged
state UUID still invalidates snapshot preparation. Counts and actual retained Vec
capacity are separately checked resource facts, not semantic digest bytes; PCND
reading may reduce spare capacity without changing the inventory digest.

Before reading PCND, the Worker quotes source bytes, the existing padded-tile cap,
the existing 8 MiB PCND metadata ceiling, bounded inline layer/stack storage,
8 layers × 256 `FilterInstance` values and the existing 1 MiB scratch allowance.
The independent, dynamically sized source-profile charge is then added using the
unchanged context. After reading, actual filter count/storage are reconciled and
retained in subsequent preparation quotes. The old 64 KiB name/control cap stays
separate from filter-owned storage. No 16 KiB profile limit applies here.

The Worker checks the immutable source SHA, state/geometry, metadata context and
filter inventory before capturing a target-bound policy from byte122. Preview
loss masks cannot select Bake. Source flag4 maps to actual loss64, independently
of metadata loss32. Both success and completed refusal echo intent and digest.
The host verifies those fields in JS and Rust, keeps its original policy and
rechecks owner/intent before publication. Revoking consent retires preparation.
These are logical admission/binding contracts, not measured RSS or forced Worker
reclamation guarantees.

Version-three ordinary controls passed locally: Formats 757 passing tests (28 existing ignores), Worker 7,
Web 344 (3 existing ignores), and Node 102. The actual production Worker passed
24 cases in Chrome 156.0.8078.79 on Apple silicon: eight positive enabled/disabled
Invert cases with/without original source metadata, eight completed refusals and
eight framing failures. Independent Python-authored PCND3/digest inputs and literal
black/white pixels check actual DDS RGBA8 samples and MSP2 packed rows, not a
PaintCraft rerender oracle. Source files stayed byte-identical. This tiny corpus
does not requalify maximum geometry or every codec setting; historical capacity
receipts retain their original scope. Actual PNG, DDS and MSP editor workflows also passed on these same 25 asset
hashes: unchecked/default blocking, explicit Bake, revocation/recheck, no stale
artifact download, exact independent pixels, and unchanged original project,
title and dirty state. Their proof directories are listed below.

The exact source/assets/response inventory is retained in
`docs/validation/filter-worker-v3/production/attempt-v8fa4g/production.json`, SHA256
`b4511a163bf7a5aba194ea21069e5afa9117b58242dfeae2ef8ea7a65d051ba4`.
No memory high-water or runtime-performance claim follows from these cases.


The matching visible-consent proofs are retained separately:

- PNG: `docs/validation/filter-bake-ui/browser/attempt-0vAVeM/production.json`
- DDS: `docs/validation/filter-bake-ui/browser/attempt-dcMLpq/production.json`
- MSP: `docs/validation/filter-bake-ui/browser/attempt-15KhIo/production.json`

`docs/validation/filter-worker-v3/ui-crossbinding.json` records their complete
receipt hashes and independently verifies all 25 production asset hashes match
the raw Worker proof. The actual UI specimen uses enabled Invert; disabled-only
intentions are separately covered by the real Worker corpus and host controls.
