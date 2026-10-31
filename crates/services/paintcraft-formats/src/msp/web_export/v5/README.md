# Spatial export protocol 5

This private same-ABI protocol is selected only when the frozen document stores
at least one Box descriptor, including disabled filters and radius zero.
Protocol 4 remains a separate 256/416-byte contract and explicitly refuses Box
and PCND5 even when the caller already installed a spatial workspace.

The request is 320 bytes followed by the exact compact PCND5 snapshot. Its magic
is `PCMSPQ05`, and its big-endian version is 5. The response is 480 bytes plus
encoded bytes, preview pixels, and bounded refusal detail; magic `PCMSPR05`.
All protocol-4 field offsets and reserved bytes retain their meanings. In
particular, source metadata stays at request160/response320 and A8 facts at
request224/response384. No reserved protocol-4 field is repurposed.

Both headers append this exact 64-byte context at request256/response416:

| Relative offset | Width | Big-endian meaning |
| --- | --- | --- |
| 0 | 2 | source container 5 |
| 2 | 2 | region profile 1: Tile64, ordered prefixes, finite-source CLAMP |
| 4 | 4 | count of all stored Box instances |
| 8 | 4 | enabled Box instances in visible, nonzero-opacity canvas intersections |
| 12 | 4 | reserved zero |
| 16 | 8 | relevant layer-region tile count |
| 24 | 8 | peak workspace for active spatial layer plans only |
| 32 | 8 | conservative aggregate sample/prefix/mask/composite work |
| 40 | 8 | operation payload peak |
| 48 | 16 | reserved zero |

Work and tile counts include all relevant visible layers, including point-only
and unfiltered composition. Thus zero spatial workspace does not imply zero
work. These units are deterministic accounting, not milliseconds. Work is at
most 268,435,456 and operation payload remains at most 160 MiB. Source, pixel,
layer, ICC, A8, encoded-output and service reservations keep their prior caps.

SourcePlan first captures geometry and accounting without serializing. Only
after the admitted frozen PCND snapshot exists does `spatial_context(len)` add
its actual byte length and finalize the context. A maximum-source-size stand-in
must not replace that length. The Worker validates bounded declarations before
PCND decode, reserves the maximum of decode and evaluation phases, and recomputes
all facts from the actual decoded document before any filter evaluation. The
response echoes all 64 bytes exactly, including typed refusals.

Accounting carries no bake/privacy authority. Original captured policies,
filter/mask inventory digest, source hash, owner, nonce and settings remain
independent bindings. The parent must revalidate live intent before publication.

The standalone Wasm operation supplies one reserved profile scope to all layer
regions. An installed Native/CLI factory is reused with the same byte realm and
shared cumulative work counter, with only tighter limits; denial never selects
a fresh fallback realm. Scoped declarations are not RSS or total process memory.
The wire contains Rust-owned storage facts and is private to matching ABI
producers/consumers; it does not promise cross-ABI byte-accounting compatibility.

Synchronous Wasm export cannot observe a JS Cancel while it is executing. The
existing service must terminate/quarantine the Worker and retain its lease until
joined teardown; a cancellation flag alone is not proof that storage is freed.
