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
tiles and 64 KiB of source metadata. The raw snapshot cap is 32 MiB + 128 KiB;
PCND writes clipped edge samples rather than charging every padded tile as raw
samples. Existing DDS limits remain separate (including its 65,536 side cap).

The existing PCND reader checks the eight-layer limit before allocating layer
metadata. The existing MSP facade admits before its single composite, then counts
and admits exact packed/output buffers before encoding. Only opaque exact black
and white are accepted: there is no threshold, dither or matte. Actual losses are
`flatten` and `hidden-rgb`. The actual encoded bitmap supplies the nearest-sample
thumbnail, bounded to 192 × 192; the freed composite slot is reused for decoding.
Cancellation checkpoints remain in the parser, renderer, counting/encoding and
preview loops. A completed, bound refusal has no output. Malformed framing or
binding is an error, not proof that a worker completed safely.

The request is 128 bytes plus raw PCND, with magic `PCMSPQ01`, protocol 1,
source length/geometry, Owner (session/state/epoch/frontend), nonce and raw SHA.
Byte 113 is explicit version 2; byte 114 is the common source loss flags; bytes
115–127 are reserved zero. The response is 288 bytes plus MSP, preview and optional
refusal detail, with magic `PCMSPR01`. It binds all request identity fields and
application version. Byte 121 is version 2, bytes 122–128 are zero, byte 193 is
the actual loss mask, bytes 194–197 are zero, and byte 265 repeats source flags.
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
    = 511,442,944 bytes < 512 MiB

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

## Actual local production qualification

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
