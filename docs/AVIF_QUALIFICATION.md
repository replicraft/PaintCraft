# AVIF decoder qualification status

As of 2026-10-26, AVIF remains **export-only**. No decoder dependency, helper
codec, manifest qualification, registry read gate or frontend consumer is enabled
by this work. The existing lossy ravif writer is unchanged.

## Container prerequisite

`paintcraft_formats::avif::inspect_container` uses the existing bounded
`paintcraft_paint::BoxTree` walker. It checks still-image brands, physical box
extents, metadata/property counts, spatial declarations and the fixed AV1
configuration prefix. It returns one-based property indices and borrowed config
OBUs; it does not guess which property belongs to the primary image.

Limits are 64 MiB input, 4 MiB metadata, 4096 boxes, depth 8, 1024 properties,
65,536 per declared side and 4,194,304 declared pixels (or the caller's smaller
allowance). These are inspection bounds, **not decoder admission**. Missing item
storage, sequence brands, duplicate required containers, malformed declarations
and cancellation fail closed. Opaque and alpha outputs from the existing writer
are both inspected; inspecting alpha does not qualify alpha decoding.

The next parser step must resolve primary-item identity, finite `iloc` extents,
`ipma` associations and essential properties through a shared item graph with
HEIF. AVIF-specific code must then validate AV1 sequence/frame/color declarations
against the actual primary item. No codec may trust unassociated `ispe` sizes.

## Decoder and allocation audit

The locally selected `image` 0.25.10 manifest separates `avif` (ravif encoding)
from `avif-native` (mp4parse plus dav1d). Its `AvifDecoder::new` parses and decodes
before normal `ImageDecoder` limit setting and hardcodes default dav1d settings.
Merely enabling that feature would not meet PaintCraft's admission contract.

The downloaded candidate [dav1d 0.11.1 source](https://crates.io/crates/dav1d/0.11.1)
exposes frame-size, thread and frame-delay settings and a custom picture
allocator. A direct adapter could request one thread, one frame delay and an
explicit pixel bound before decode. That is a possible adapter design, not a
measured guarantee. The picture callback requires explicit alignment/padding
and lifetime accounting; it is not a general allocator hook.

In the audited [dav1d 1.3.0 source](https://github.com/videolan/dav1d/tree/1.3.0),
`src/lib.c`, `decode.c`, `obu.c` and `ref.c` also allocate contexts, tile/coefficient
scratch, metadata and references. `src/mem.h` maps their ordinary allocations to
C malloc/realloc/aligned allocation, outside Rust's global allocator and the
picture callback. Thread and pixel settings therefore do not establish the
existing worker's total heap ceiling. A public decoder needs audited accounting
for every native allocation or separately proven process-level memory
containment on each target. Crash isolation alone is insufficient. No such
containment is claimed here.

## Packaging audit

The audited dav1d-sys 0.8.2 build probes system dav1d >=1.3.0. Its optional
internal build fetches tag 1.3.0 and runs Meson/Ninja for a static library. The
build script has no explicit PaintCraft six-target cross-compilation contract.
A host-installed library is not evidence that a release artifact contains the
right architecture or runtime dependencies. No native build was attempted:

| Release target | Decoder build and clean-install evidence |
| --- | --- |
| Windows x86-64 and ARM64 | Not established |
| macOS x86-64 and Apple silicon | Not established |
| Linux x86-64 and ARM64 | Not established |

Sources were downloaded only into `/private/tmp` for inspection; no Cargo
manifest or lockfile changed. Archive SHA-256 values are:

| Source archive | SHA-256 |
| --- | --- |
| dav1d 0.11.1 | `3ee89cb860616069c67520dcd66cacdb900b57c799f634a0eb6d91f6e2a82b61` |
| dav1d-sys 0.8.2 | `6ecb1c5e8f4dc438eedc1b534a54672fb0e0a56035dae6b50162787bd2c50e95` |
| dav1d C tag 1.3.0, GitHub archive | `1b3e75433dd69eb88ff3190ed1b1707ca5b9f43260b6348c551455c885eaab3a` |

## Gate to an actual consumer

1. Complete bounded item/AV1 preflight and an explicit initial still RGB8/color
   profile. Keep alpha, HDR, grids, sequences and broader color variants visible
   as unsupported until separately qualified; never silently discard them.
2. Prove native allocation containment and deterministic settings, then package
   and independently reopen real decoder outputs on the supported targets.
3. Reuse the existing isolated helper, typed private prepared import, retained
   leases and source/owner checks. Add actual native Studio, CLI and MCP tests
   before qualifying their exact variant. Browser adoption is a separate gate.
4. Compare against a pinned independent reference backend, including color,
   alpha/depth refusals, malformed inputs, budget denial and cancellation.
   The [AVIF specification](https://aomediacodec.github.io/av1-avif/v1.2.0.html)
   defines more profiles and derived images than this first planned subset.

Five focused inspection tests cover actual current-writer containers, all-prefix
truncation, non-primary property identity, configuration errors, resource limits
and cancellation. They qualify only this prerequisite, not AVIF image import.
