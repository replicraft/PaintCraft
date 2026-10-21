# Device-local GPU source reuse

The ordered integer compositor now retains bounded, clipped, dense layer-source
buffers on its own device. Authoritative sparse host tiles, recorded history,
Session publication and saves remain independent. This is a first layer-view
residency implementation, not a GPU tile atlas or GPU-authoritative editing store.
Output composition still covers the complete requested region. Explicit readback
covers that region; native windows use [completed display textures](GPU_DISPLAY.md)
without normal host readback/re-upload.

## Identity and updates

Keys include document/layer identity, origin, raster extent/default RGBA, clipped
world/local bounds and derived 64-pixel source-region identities. They retain no
host pixels, leases or weak tile control blocks. Opacity/order/visibility belong
to ordered dispatch, not raw source bytes: changing them does not require upload
when sampled pixels stay identical. Another document cannot reuse the namespace.
Device ownership is implicit in the renderer; a replacement gets a fresh cache and
DeviceEpoch. Invalid/exhausted region identities or a key that cannot fit metadata
admission use ordinary streaming instead.

A matching unchanged view uploads nothing. Changed source cells identify vertical
bands; bands on the same row or adjacent rows coalesce into full-view-width strips.
Only those clipped rows are gathered/uploaded, keeping each GPU copy contiguous.
This deliberately uploads more than the minimal changed rectangle when a cell
covers few columns; source tile storage and fine CPU output patches remain separate.
An empty, fully transparent implicit layer skips source gathering and dispatch;
hidden RGB remains untouched in authoritative storage and never leaks into output.

Update staging buffers use COPY_SRC, resident sources use STORAGE|COPY_DST, and
copies and ordered composition share one command encoder. Nothing is scheduled
through pending Queue::write_buffer calls. Dropping an unsubmitted encoder discards
partial transfers. Submitted layers retain the existing completion wait; cache
identities update only after completion and cancellation/epoch checks. Any ordinary
render error or cancellation clears all source entries. A poisoned source mutex
retires derived state before reuse. No GPU cache event commits or saves an edit.

## Bounds and ownership

Default retained limits are 64 MiB of source buffer sizes, 4 MiB of explicit
metadata charge and 64 entries. These are provisional guardrails, not measured
optimal thresholds or physical GPU allocator/RSS bounds. Metadata includes entry
Vec capacity and actual region-token Vec capacities. An oversized view streams;
small/zero limits remain correct. Buffers used in the current frame are protected
from eviction, so a sequential frame larger than the cache streams its overflow
rather than evicting the entire just-used working set. Unused older views are LRU
victims. This favors earlier dispatched views under sustained overload, not fairness
or an optimal active-layer policy.

`GpuRenderer::with_region_source_cache(true)` can retain multiple independently
keyed clipped views of one layer under the same existing byte/metadata/entry caps.
Different view bounds never alias a buffer. A changed layer origin, extent or
default retires that layer's old scope; opacity/order changes still reuse raw bytes.
Matching views patch their own changed rows, so overlapping retained views acquire
their new identities independently when next requested. Enabling or disabling the
policy clears retained derived sources and changes no document pixels.

Frame protection begins at a public native/readback/scaled request boundary, rather
than at each internal native block. Scaled requests therefore preserve early sources
through the whole frame and stream overflow without repeatedly evicting that working
set. Completion checks, cancellation retirement and independently owned display
textures follow the same rules in either policy. This is bounded region-view
retention, not a sparse tile atlas, asynchronous transfer or physical GPU admission.
The original single-view policy remains the default after
[native comparisons](../benchmarks/results/gpu-region-sources-2026-10-21/DECISION.md)
confirmed allocation/upload reductions but mixed process timing and mostly tied
statistical samples. This is an explicit host option, not automatic native dispatch.

Calls sharing a renderer serialize source updates through a derived-cache mutex.
Cancellation callbacks must not reenter render/cache queries on the same renderer.
The native UI never takes this mutex: its admission query reads an immutable field.
The native worker adds retained buffer capacity plus three metadata allowances
(old keys, new keys and update-strip metadata), currently 76 MiB, to existing
transient output/source/preview declarations. This is declared worker admission,
not process-wide physical GPU memory enforcement. GPU buffers, driver staging,
readback/host output and opaque driver allocations remain distinct resource classes.
Device loss/timeout and OS memory-pressure behavior need broader qualification.

`GpuRenderStats` reports source allocations, uploaded bytes/strips, source hits,
patched sources and retained usage after a successful frame. They describe actual
application staging, not hardware bus traffic, allocator residency or presentation
latency. Failed attempts return no successful frame/statistics.

## Executed evidence and remaining work

Required real-Metal tests exercise exact scalar parity, unchanged/partial uploads,
clipped offsets, opacity/reorder changes, document separation, one-entry overflow,
streaming/metadata limits, cancellation at every observed patch boundary, poison
recovery, lock-independent admission and decoded-lease release. Device destruction
rejects old frames, clears retained sources on the next attempted render and
reconstructs host-backed committed/undo/redo output on a fresh device.

The [desktop measurement record](../benchmarks/results/gpu-sources-2026-10-21/DECISION.md)
compares actual preview staging, GPU composition, completion, full readback and
teardown with source retention enabled/disabled. It retains raw statistical/process
samples and an over-entry-limit workload. No zero-copy display or input/frame-tail
claim follows from those complete-work observations.

Persistent sparse GPU working tiles/atlas layout, pooled/region-only output/readback,
asynchronous transfer services, CPU/GPU cost dispatch, device-wide physical budgets,
GPU committed-edit chains, automatic native device recreation, other adapters and
native queue/input/display latency remain open. T12/T13/T21/T37 remain partial.
