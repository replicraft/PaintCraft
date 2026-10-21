# Bounded ordered GPU submission batches

`GpuRenderer::with_submission_batching(true)` groups consecutive layer dispatches
in one encoder. Separate compute passes preserve exact integer layer-opacity and
source-over rounding in original document order. Layers are not mathematically
regrouped or downsampled independently. Native readback, native display and scaled
display use the same compositor. The original one-layer submission policy remains
default after the [native comparison](../benchmarks/results/gpu-submission-batches-2026-10-21-rerun/DECISION.md)
confirmed multilayer gains, mixed five-layer controls and higher process RSS.

Native experiments can set `PAINTCRAFT_GPU_BATCHING=1` for batching with the
original source policy, or `PAINTCRAFT_GPU_BATCHING=regions` for batching plus
bounded regional sources. Unset/unrecognized values leave serial single-view
rendering enabled. Positive sampling levels still require the independent
`PAINTCRAFT_GPU_SCALED=1` option; otherwise the automatic CPU scaled path remains.
These explicit options do not promise a speedup for every image or qualify a
general CPU/GPU dispatch threshold.

## Bounds and ownership

A batch contains at most 16 contributing layers and normally at most 1 MiB of
source-view sizes, including cache hits. Before a source would exceed that window,
the previous batch completes. An individual larger source executes alone under
the caller's existing regional input/output admission. Hidden, zero-opacity,
fully clipped and empty implicit transparent layers contribute no dispatch.

Pending source-key capacities have a 1 MiB flush threshold. One final key can cross
it; that key is still bounded by the existing source-cache metadata allowance.
Thus pending keys are bounded by the threshold plus one admitted key, not exactly
1 MiB. Cached views are protected as soon as preparation uses them. Their new
pixel identities are published only after successful batch completion and a fresh
cancellation/device check. Over-cap source caches still stream rather than pinning
unbounded retained state. Source limits and authority do not change.

Dropping an unsubmitted encoder discards its queued copies and dispatches.
Cancellation checks occur between layers, before submission and after completion;
already submitted work is not cancelled. The worker waits for its submission before
the next successful batch or publication. Errors retire derived cache state and
never install partial output. Driver timeouts/device loss retain their existing
error path; native recovery and physical outstanding-work accounting are not
qualified by this feature.

Private output clearing stays ordered before the first contributing dispatch;
empty reused outputs still clear. A completed private output can be recycled only
after the dependent readback or display reduction/conversion. Public display
textures retain independent ownership across later batches and scratch reuse.

An immutable extra 4 MiB worker declaration covers the bounded additional pending
source/staging window, key threshold and bookkeeping. One working source/key keeps
its existing source/output allowance; opaque driver allocations and global physical
memory are outside this declaration. The query does not take GPU/cache locks.
Serial mode declares zero extra bytes. The desktop adds the declaration when
admitting either native-resolution or scaled jobs.

## Evidence and remaining scope

`GpuRenderStats` reports actual compositor submission count and maximum layer,
source-byte and key-capacity windows. These counters exclude reduction, presentation
and explicit readback submissions; they are not GPU timing or bus measurements.
Tests compare independent scalar pixels across 1/5/20/100 layers, reverse ordering,
exact byte-window boundaries, oversized solitary sources, zero/small/full cache
limits, cancellation checkpoints, epoch retirement and old-frame ownership.
Actual egui framebuffer tests cover both submission policies, including native and
scaled partial edges. A real admitted desktop worker verifies 20 dense layers.

The native comparison includes cold setup/teardown, actual preview staging, completed
GPU output and native texture acceptance under both source policies. It retains
single-layer and uniform controls. This is bounded synchronous batching, not an
asynchronous transfer ring, priority scheduler, OS latency qualification or a
general CPU/GPU crossover policy. No new dependency is introduced.
