# Browser MSP2 export boundary

Web Studio can prepare an explicit compressed MSP2 copy through the existing
isolated codec Worker. Choose Version 2, review the actual encoded preview and
losses, and approve before downloading. The exact opt-in export variant requires
preflight; default MSP1 retains its previous bytes, capacities and no-Worker route.
Classic, ordinary Open and preserving MSP writes are unchanged.

The source must be an immutable PCND snapshot of the captured document/state.
Before serialization the caller retains a 512 MiB logical service reservation.
The inventory admits positive u16 canvas sides, at most 4,194,304 canvas pixels,
eight layers, 8,388,608 aggregate logical layer pixels, 64 MiB of padded source
tiles and 64 KiB of title/layer/control metadata. A source attachment stays in
the real PCND2 snapshot under its existing 8 MiB wire allowance and separate
charged admission. The raw snapshot cap is 32 MiB + 128 KiB;
PCND writes clipped edge samples rather than charging every padded tile as raw
samples. Existing DDS limits remain separate (including its 65,536 side cap).

The existing PCND reader checks the eight-layer limit before allocating layer
metadata. The existing MSP facade admits before its single composite, then counts
and admits exact packed/output buffers before encoding. Only opaque exact black
and white are accepted: there is no threshold, dither or matte. Actual losses are
`flatten` and `hidden-rgb`, plus `source-metadata-omitted` only after a typed
explicit choice against the exact immutable copy. The actual encoded bitmap supplies the nearest-sample
thumbnail, bounded to 192 × 192; the freed composite slot is reused for decoding.
Cancellation checkpoints remain in the parser, renderer, counting/encoding and
preview loops. A completed, bound refusal has no output. Malformed framing or
binding is an error, not proof that a worker completed safely.

The version-two request is 192 bytes plus raw PCND, with magic `PCMSPQ02`, protocol 2,
source length/geometry, Owner (session/state/epoch/frontend), nonce and raw SHA.
Byte 113 is explicit version 2; byte 114 is the common source loss flags; bytes
115–127 are reserved zero. The response is 352 bytes plus MSP, preview and optional
refusal detail, with magic `PCMSPR02`. It binds all request identity fields and
application version. Byte 121 is version 2, bytes 122–128 are zero, byte 193 is
the actual loss mask, bytes 194–197 are zero, and byte 265 repeats source flags.
Bytes128–191 carry the same 64-byte source context specified in the
[DDS wire layout](../../dds/web_export/README.md#source-metadata-context-and-copy-authorization):
actual document UUID, optional attachment digest, presence/explicit omission,
actual MSP target2/flat-operation1, zero reserved byte, wire extent and retained
charge. Response bytes288–351 echo it exactly. Metadata-none requires zero
digest/extent/charge; explicit intent may still be echoed without claiming loss.
The Worker quotes and admits attachment storage before reading PCND2, validates
the actual decoded document, and never strips the snapshot to bypass policy.
Bit4 of the actual loss mask denotes real metadata omission; the existing two
source flags keep their meanings. Old version-one frames are refused.

Both payload digests, complete MSP row table/packet inventory, exact fresh header,
geometry and opaque black/white thumbnail are validated before host acceptance.
The thumbnail digest binds worker output; it is not a host re-render proof.

The 1 MiB encoded cap is independently conservative over every allowed shape.
For a packed row of n bytes, fresh packets emit at most n + ceil(n/4): repeats
consume at least three bytes, and adjacent literals need either a full 255-byte
literal or an intervening repeat. Enumerating widths 1…65,535 and maximum legal
heights yields a whole-file upper bound of 903,410 bytes at 65 × 64,527, including
the 32-byte header and two-byte-per-row table. Skinny shapes are included.

The full logical host quote is:

    256 MiB worker ceiling + 64 MiB retained source
    + 5 × (32 MiB + 128 KiB) source slots + 5 × 1 MiB result slots
    + 8 × (192 × 192 × 4) preview bytes + 1 MiB metadata
    = 511,442,944 bytes, plus at most 8 MiB + 64 KiB retained source attachment
    = 519,897,088 bytes < 512 MiB

Source slots cover serialization, retained request, bridge/JS copies and transfer
staging; result slots cover worker/frame/JS/bridge/download staging. Admission
precedes serialization, and immutable owner/options/source/nonce binding survives
Rust frame validation, preview, consent and tracked Blob URL download. The lease
remains held until URL revocation acknowledgment; download initiation is not a
filesystem-persistence acknowledgment. The worker separately
checks a 160 MiB payload quote containing padded source, raw source, composite,
exact packed/output, parser allowance, response and thumbnails before allocations.
The existing 256 MiB Wasm ceiling remains separate. Neither quote measures RSS,
GPU textures, browser implementation overhead, stacks or allocator capacity.

Local Rust controls cover the actual 4 MP canvas, both positive-u16 skinny axes,
non-monochrome refusal, source/layer/metadata/cancellation limits, independently
specified odd-width packet bytes, output digests/framing/header inventory, owner
and version binding, and the complete shape-bound enumeration. Shared snapshot
controls exercise different DDS/MSP side/tile/raw limits and unchanged raw PCND
bytes. Existing DDS golden/linear/cap/refusal tests remain green after extraction.

## Historical version-one local production qualification

These receipts remain valid for the prior protocol/resource profile. The new
`web-msp2-row-rle-worker-v2` metadata-bound route has a separate representative
production-browser qualification described below; local Worker/context and Node
controls are reported separately.

A frozen production app and bundled lean Worker passed eight independent
positive frames and three canonical policy refusals in headless Chrome 156 on
Apple silicon macOS. Cases include the 4 MP canvas, both u16 skinny axes, the
row-table bound, and two-layer raw PCND above 32 MiB. A fresh Chrome process with
no prior codec Worker also completed the 4 MP/two-layer first-Worker case under
the unchanged deadline. This does not establish cold OS-cache performance.

Actual desktop and 390 × 740 mobile-viewport Studio controls exercised the
version chooser, complete preview, loss consent and exact golden download.
Project bytes, dirty state and title were unchanged. Real active-Worker cancel
and protocol-owner invalidation produced no MSP download, retained quarantine
and left project download usable. Rust host tests separately cover editor-owner,
modal and options staleness; protocol-owner mutation is not an editor UI test.

The result held 512 MiB through download initiation and returned to zero after
URL retirement. Separate same-Wasm instrumentation observed at most 102,760,448
bytes of linear memory; the 256 MiB hard-growth probe was refused. These are not
RSS/GPU/browser-implementation measurements. Shared-bridge regressions also
passed a small actual DDS golden/download lifetime case and JPEG XL import with
all independent samples retained in a downloaded PCND.

Evidence: `/private/tmp/paintcraft-msp2-web-proof-2026-10-27/` records the exact
source composition, bundle hashes, native/Pillow reference, raw failures and
production results. The native specimen executable's content hash is unavailable;
its source, logs and every generated artifact remain identified. No physical
mobile/tablet, original Windows Paint, maximal-capacity or broader codec claim
is established by these local browser cases.


## Version-two metadata qualification

The current production Studio build passed a real opaque 4×4 PCND2 row-RLE flow
with retained original source metadata. Chrome 156 on Apple silicon exercised
explicit omission, visible encoded preview, revocation/recheck with a new nonce,
Cancel, reopen and download. Pillow 12 independently decoded every expected
pixel. Original attachment/project bytes, title and dirty state stayed unchanged.

Six fresh realms altered actual request digest, extent, intent, target, version
or retained-memory admission. All refused without image/preview output; the
understated charge returned the typed resource-limit result. Exact executed
drivers, original inputs, oracle wheel and the shared 25-asset production build
are bound by `docs/validation/metadata-omission/advanced-browser-final-freeze.json`,
SHA256 `1072302286a9edb882b22ea60059071059f379c2ec0a920652c89c33faaf2e7f`.
This does not qualify every historical MSP variant, browser or physical device.

## Explicit metadata choice

The old `SourcePlan::capture` remains refusal-safe for attachment-bearing inputs.
`capture_with_metadata_policy` validates the actual MSP flat-export target before
serialization. Default or “allow all” loss names grant no metadata omission.
The host keeps the original Policy with the actual Frozen copy through await,
visible preview and download; live owner/Frozen checks guard later editor changes.
Unchecking the separate metadata choice revokes earlier preparation/approval.
Only unrelated losses need the existing additional export-changes checkbox.
PCND and preserving-save paths do not accept this omission policy.
