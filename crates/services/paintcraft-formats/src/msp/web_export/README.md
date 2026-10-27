# Browser MSP2 boundary (not yet admitted)

This Rust boundary prepares explicit row-RLE MSP2 copies for a future Web Studio
route. It does not change the legacy MSP1 exporter, browser registry, controller
or UI. No production browser or original Windows Paint qualification is claimed.

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
staging; result slots cover worker/frame/JS/bridge/download staging. Later JS/UI
adoption must prove these lifetimes and preserve this bound. The worker separately
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
