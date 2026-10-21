# Completed GPU compositor buffer reuse

GPU composition can retain one completed private straight-RGBA8 storage buffer for
a later request. Retention is bounded independently of source-cache residency and
configurable through `GpuRenderer::with_working_cache_limits`; zero disables it.
The measured 64 MiB candidate cap holds at most one buffer, not a preallocated arena.
Fresh allocation remains the default (`DEFAULT_OUTPUT_POOL_BYTES == 0`): the native
comparison has small-view regressions and mixed 4K evidence. A host must explicitly
configure a nonzero cap to use retention. See [the decision](../benchmarks/results/output-pool-2026-10-21/DECISION.md).
Requests larger than the cap stream without retaining their output. A retained
buffer may serve a smaller request; the larger capacity remains charged/reported.
Changing a region's origin does not invalidate scratch because no content is reused.

## Ownership and completion

The pool contains no document/history references or display textures. Private
compositor buffer leases can be returned only after observed completion of the
final GPU read: completed straight readback, or completed display conversion.
Failure, cancellation or unwind drops the private buffer without recycling it.
Ordinary errors also clear idle output/source caches. Retired device epochs reject
new results; replacement renderers have independent pools. Idle eviction does not
cancel an active render, which may warm the pool after its completed result returns.

Fresh buffers rely on wgpu's guaranteed zero initialization. Reused buffers explicitly
clear the entire requested logical output before any source-over reads. The first
contributing layer's encoder carries the clear; an empty/clipped/transparent-only
composition submits and waits for its clear separately. Readback and presentation
use the requested dimensions/stride even when retained capacity is larger. Buffer
usage adds COPY_DST only for recyclable buffers; disabled/oversized paths retain
the previous fresh-buffer usages and implicit initialization behavior.

Completed display textures keep independent ownership. A texture view can be cloned
and retained by an external caller or egui, so dropping a frame alone does not prove
that its texture can be safely overwritten. This pool never recycles those textures.
An old registered display must remain pixel-identical while later composites reuse
their private scratch. Texture pooling needs a separate ownership/completion protocol.

## Admission and observability

Native render admission includes the immutable output retention allowance in addition
to source-cache and requested-region transient declarations. Reading this allowance
takes no GPU/cache mutex. Buffer allocation/reuse counts, explicit cleared bytes and
idle retained capacity are reported with GPU frame statistics; callers can inspect
or evict idle output separately. Checkout holds the pool mutex only for bookkeeping,
not for GPU allocation, rendering or completion waits. Existing source serialization
prevents overlapping compositor ownership on one renderer.

Retention is a service-local capacity bound. Declared worker bytes, live allocation
sizes, process RSS and physical GPU/driver residency are different quantities.
Accounting every service/metadata allocation, pressure-driven GPU eviction and native
latency/interference qualification remain open. No physical-memory or throughput
claim follows from enabling reuse.

## Checks and qualification scope

Metal regressions compare exact pixels across changing colors, source identities,
transparent/empty output, smaller/larger regions and warm reuse. They retain multiple
live display textures across later composites and idle eviction, verify disabled and
over-cap behavior, cancel at observed warm checkpoints, retry exact straight output
and retire a device epoch. The actual egui renderer draws an old registered texture
after its compositor buffer has been reused and compares every pixel with a managed
CPU image. Authoritative source pixels/tickets remain unchanged.

The native measurement driver compares fresh versus pooled completion/acceptance for
a small viewport, a 4096×2160 output and alternating large/small requests. Construction,
clears, source processing, operation staging, GPU waits, display conversion, accepted
registrations and teardown are included. The alternating case exposes retained large
capacity rather than excluding it. Required visible pixels, independent CPU oracles,
raw process/statistical samples and cold startup controls govern the default policy.
