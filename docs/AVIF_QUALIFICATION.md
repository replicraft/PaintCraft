# AVIF decoder qualification status

As of 2026-10-26, native Studio **Import image…**, CLI and native MCP support
`native-avif-srgb8-worker-v1` through the verified AVIF-enabled V5 helper.
The first reader covers explicit identity sRGB8 still pictures with optional
same-size straight auxiliary alpha. This is a qualified subset, not full AVIF
parity. The existing lossy ravif export path is unchanged.

## Admitted source and conversion policy

The bounded `avif::admit_srgb8` parent parser reuses `BoxTree` and the shared
`item_graph` for primary identity, finite `iloc` extents, `ipma` associations and
essential-property ownership. It verifies actual AV1 sequence/frame declarations
against associated `av1C`, `ispe`, `pixi` and explicit NCLX metadata. The parent
never invokes C. The helper repeats admission and crosschecks actual decoded
geometry, precision, layout, color, level and still-picture flags before emitting
its typed response.

The primary must be reduced-still-picture AV1 profile 1, 8-bit full-range 4:4:4,
with primaries 1, sRGB transfer 13 and identity matrix 0. Native planes are G/B/R;
RGBA assembly reorders them without a matrix conversion. Optional alpha must be
same-size profile-0 8-bit monochrome, full range with CICP 2/2/2 and the exact
straight-alpha auxiliary link. Coded hidden RGB is retained, including at alpha
zero. Encoder-input RGB and decoded RGB are separate evidence: ravif output can
already be lossy even at quality 100.

Limits are 64 MiB input, 4 MiB metadata, 4096 boxes, depth 8, 1024 properties,
65,536 per side and 4,194,304 pixels, or a caller's smaller allowance. There is
one admitted sequence and combined-frame OBU per coded item. Missing or unused
properties, metadata disagreement, sequences, film grain, super-resolution,
render-size changes, derived images/grids, transforms, premultiplication, ICC,
HDR/deep precision and SDR YCbCr profiles are refused. The physical-container
inventory API alone does not qualify a source for decoding.

Nonempty primary or alpha names require `avif-item-names-not-retained` consent:
they remain in the source assessment receipt, not PCND metadata. An unnamed
identity source has no conversion loss and imports without an empty approval
prompt. Studio's named-source approval requires an actually visible prior-frame
preview and current document/draft ownership. Cancel, source changes, missing or
unknown consent and stale results leave the previous document intact.

Imports create unsaved PCND copies. The shared Job/Launch/executor, retained
preparation and resident tile leases, same-open source identity and final adoption
checks are reused. Source protection follows the document across tabs/workspaces
and remains effective after PCND Save, including renamed/hardlinked source
aliases. These checks do not eliminate arbitrary filesystem races.

## Helper contract and delivery

Code 5 requires the exact `BUILD_V5_AVIF` record, `AVIF_NATIVE_PROFILE`, schema 2
manifest and four-entry qualified variant set. A V4 helper cannot decode this
profile; non-AVIF jobs accept either exact known contract. Verification does not
retry another contract after integrity failure. Standalone binaries or incomplete
installations fail closed. Ordinary Open, Classic, browser AVIF reads, foreign
preserving Save and broader source profiles remain unavailable.

Normal helper builds enable `avif-codec` by default; only explicit
`--no-default-features` builds produce V4 for compatibility checks. Native
release and source-installer builds also request `paintcraft-codec-worker/avif-codec` and
bundle the helper, manifest, JPEG XR notice and exact dav1d COPYING notice. The
complete CLI installer verifies and installs that payload; source mode builds
both CLI and helper before using the same installation transaction. Actual local
Apple-silicon source and archive installations pass native AVIF CLI/MCP imports
and PCND saving. The six native release runners are configured to build and test
the helper and installed consumers, but that changed workflow has not been
executed on all six targets. No Windows, Intel Mac or Linux clean-install claim
is made from local or synthetic archive checks.

## Independent evidence

Owned opaque and straight-alpha fixtures are checked against pinned **libaom
3.15.2**, commit `af3dc9aadc793c1b00edc003e3fc19a44fdeb574`, a different decoder
engine from selected dav1d 1.5.4. All native planes for the two primary streams
and alpha stream match exactly; assembled RGBA, including hidden RGB, is exact.
The checked-in [oracle record](../crates/services/paintcraft-formats/tests/avif-fixtures/libaom-oracle.json)
retains archive hashes, build options, harness and output hashes. FFmpeg's
libdav1d readback independently checks container/plane assembly, not a second
codec engine. Raw sequence level 31 is retained; the FFprobe level-23 diagnostic
is recorded separately rather than changing source declarations to match it.

Helper test suites pass with both feature configurations: 29 tests
with AVIF enabled (8 private fixtures ignored by default), and 24 without (3
ignored). Successful qualification requires normal process exit as well as a
complete response. Actual quota child fixtures exercise context, tile-scratch
and picture allocation failures; malformed, mismatched, cancelled and stale
requests do not publish pixels.

Actual copied V5 Studio tests pass 12 English/Japanese cases: named approval,
cancel, changed source and draft staleness, plus unnamed opaque/alpha adoption.
They preserve a nonempty previous document on refusal, reopen saved PCND with
exact independent RGBA, release transient/resident charges and refuse renamed
source aliases after Save. Installed CLI/MCP tests pass 12 cases across V5,
including existing JXL/JXR/HEIF regressions; the V4 regression passes 10 with the
two V5-only AVIF cases skipped. Missing/unknown consent, wrong V4 manifest,
missing helper, cancellation and source guards are exercised through real routes.

Next is common SDR YCbCr qualification with explicit range/matrix/chroma policy
and independent RGB/alpha references. Broader metadata, HDR, grids, browser
execution, hostile corpora, latency/whole-operation benchmarks and six-target
installation evidence remain separate work. The
[AVIF specification](https://aomediacodec.github.io/av1-avif/v1.2.0.html) covers
substantially more than this initial profile.

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
containment on each target. At this historical audit stage neither was established.
The selected native-allocation shim below addresses requested decoder allocations;
it does not claim an OS sandbox or physical RSS bound.

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

### Static-build follow-up

The binding-default 1.3.0 audit above is historical evidence, not the selected
shipping version. The selected worker now pins upstream
**1.5.4**, immutable commit `54706fc6bc0cdecab7e9593974a4039cc038fca7`, with source
and license under `apps/paintcraft-codec-worker/vendor/dav1d`. Upstream
[release notes](https://github.com/videolan/dav1d/blob/1.5.4/NEWS) include fixes
newer than 1.3.0; the build neither probes a system library nor downloads sources.
The small allocation/tagging diff is `native/dav1d-allocation.patch`. Its selected
C-only build routes decoder-owned allocations through the existing worker heap,
including headers/padding. This is requested-allocation accounting, excluding
libc internals, thread stacks, allocator overhead and physical RSS. The native
decoder notice is now bundled by release packaging; six-target execution remains
unverified.

On the local Apple-silicon host, four focused native boundary tests pass; their
four private child fixtures verify decoder-allocation retirement and actual
context, tile-scratch and picture quota exits through the same worker heap.
Touched-build and explicit compiler-environment rebuilds pass with pristine
source hashes checked again. These boundary checks complement the actual
consumer evidence above; they are not cross-target release qualification.
