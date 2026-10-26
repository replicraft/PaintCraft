# AVIF decoder qualification status

As of 2026-10-26, AVIF remains **export-only** in the product. An optional private
native decoder build and allocation boundary now exist for qualification; no
public AVIF manifest contract, registry read gate or frontend consumer is enabled.
The intended first public reader covers RGB8 still pictures with optional straight
auxiliary alpha. The existing lossy ravif writer is unchanged.

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

## Initial image/binding audit (historical)

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

## Initial binding packaging audit (historical)

The audited dav1d-sys 0.8.2 build probes system dav1d >=1.3.0. Its optional
internal build fetches tag 1.3.0 and runs Meson/Ninja for a static library. The
build script has no explicit PaintCraft six-target cross-compilation contract.
A host-installed library is not evidence that a release artifact contains the
right architecture or runtime dependencies. At that audit stage no native build had been attempted:

| Release target | Decoder build and clean-install evidence |
| --- | --- |
| Windows x86-64 and ARM64 | Not established |
| macOS x86-64 and Apple silicon | Not established |
| Linux x86-64 and ARM64 | Not established |

That initial audit downloaded sources only into `/private/tmp` and changed no
Cargo files. The later optional static build below adds a vendored archive and
worker build dependency. Historical audit archive SHA-256 values are:

| Source archive | SHA-256 |
| --- | --- |
| dav1d 0.11.1 | `3ee89cb860616069c67520dcd66cacdb900b57c799f634a0eb6d91f6e2a82b61` |
| dav1d-sys 0.8.2 | `6ecb1c5e8f4dc438eedc1b534a54672fb0e0a56035dae6b50162787bd2c50e95` |
| dav1d C tag 1.3.0, GitHub archive | `1b3e75433dd69eb88ff3190ed1b1707ca5b9f43260b6348c551455c885eaab3a` |

## Gate to an actual consumer

1. Complete bounded item/AV1 preflight for still RGB8 with optional straight
   auxiliary alpha and an explicit color policy. Keep HDR, grids, sequences and
   broader variants unsupported until qualified; never silently discard them.
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

### Static-build follow-up

The binding-default 1.3.0 audit above is historical evidence, not the selected
shipping version. The private worker qualification feature now pins upstream
**1.5.4**, immutable commit `54706fc6bc0cdecab7e9593974a4039cc038fca7`, with source
and license under `apps/paintcraft-codec-worker/vendor/dav1d`. Upstream
[release notes](https://github.com/videolan/dav1d/blob/1.5.4/NEWS) include fixes
newer than 1.3.0; the build neither probes a system library nor downloads sources.
The small allocation/tagging diff is `native/dav1d-allocation.patch`. Its selected
C-only build routes decoder-owned allocations through the existing worker heap,
including headers/padding. This is requested-allocation accounting, excluding
libc internals, thread stacks, allocator overhead and physical RSS. Public AVIF
import and six-target packaging remain unqualified; the native decoder notice
must be bundled before enabling any release build.

On the local Apple-silicon host, four focused native boundary tests pass; their
four private child fixtures verify decoder-allocation retirement and actual
context, tile-scratch and picture quota exits through the same worker heap.
Touched-build and explicit compiler-environment rebuilds pass with pristine
source hashes checked again. These are private backend checks, not an enabled
AVIF import or cross-target release qualification.
