# Catalog MSP2 export protocol6

Source integration under qualification; no production browser delivery claim.
Protocol6 is independently negotiated. Protocol4 and protocol5 remain closed.
Native executable codec helpers receive already rendered pixels and do not adopt
this private browser snapshot packet.

Request `PCMSPQ06` is 384 bytes; response `PCMSPR06` is 544 bytes, with u16BE
version 6 at offset 8. Existing fields through mask context remain at their prior
offsets. The region context is 64 bytes at request 256/response 416 and names source
container 6/profile 1. It permits zero stored Box while still pricing all relevant
visible point/composite layers. Point-only layer work is doubled for uniform proof
plus dense fallback; spatial plans already evaluate each stage once.

New catalog context is 64 bytes at request 320/response 480:

| Offset | BE field |
|---|---|
|0|u16 source container6|
|2|u16 prepared profile 1|
|4|u32 all stored Levels|
|8|u32 enabled Levels, including hidden/off-canvas|
|12|u32 unique complete enabled policies, max 256|
|16|u64 compile work|
|24|u64 retained program quote|
|32|u64 preparation peak including inventory|
|40|u64 evaluation peak: retained programs plus region scratch|
|48|u64 compile plus region/composite work|
|56|8 zero reserved bytes|

Accepted numerical profiles are exactly the initial eight listed by the PCND
catalog contract, each with scoped codec `params-v1`; a future general reader
cannot widen this Worker capability. The source must be PCND6 with stored Levels,
including disabled/default intent. Request SHA, actual manifest, decoded inventory,
owner, dimensions, immutable copy policies and both context blocks are validated.
All response bindings echo the captured request; current-owner validation remains
independent from source decoding and descriptive accounting.

SourcePlan captures metadata-only facts, then finalizes using actual PCND byte
length. No 32-MiB substitution is used for payload peak. The unchanged MSP 160-MiB
payload, source/pixel/layer/encoded bounds, 640-MiB browser reservation and 256-MiB
Worker linear-memory declaration still apply. Workspace is at most 64 MiB and work
at most 268435456 abstract units, not elapsed time. Existing parser/control copies
are retained and the additional two 64-byte packet context copies are priced.

After predecode admission, actual source facts are recomputed before execution.
One outer caller-reusing workspace and frozen prepared-document job enclose
encoding and thumbnail creation. Tables compile once per complete key and survive
all tiles/chunks; no per-pixel or per 65536-sample preparation occurs. A current
native caller's byte realm and cumulative work counter are reused with minimum
limits; only a standalone operation uses the already reserved fallback scope.
The program remains retained through encoded-output/thumbnail phases and is
priced there. Metadata omission and filter baking remain independent explicit
copy authorities. This private quote is same producer and consumer ABI descriptive
accounting, not a cross-ABI or aggregate browser/GPU RSS guarantee.
