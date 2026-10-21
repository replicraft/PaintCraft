# Completed native GPU display textures

Native windows now consume a completed GPU texture directly. The renderer first
computes the existing exact straight RGBA8 composite, then a compute pass produces
an Rgba8Unorm texture using the selected egui/ecolor 0.37 encoded-sRGB integer
premultiplication rule. Every output channel is an exact eight-bit result. This
avoids normal native-frame host readback, CPU ColorImage conversion and re-upload.
It does not make the GPU authoritative for document edits, history or saves.

`render`/`render_cancellable` still return straight RGBA8 for readback consumers.
Their mapped-copy allocation is now fallible, and a scope guard unmaps on success,
cancellation, allocation/mapping error or unwind. `GpuTextureFrame::readback` is an
explicit verification adapter returning premultiplied display bytes, not straight
export pixels. Odd-width texture rows use checked padded GPU readback and remove
padding before returning. CPU/export numerical policy is unchanged.

## Publication and registration

A texture frame owns its texture/view and originating device/queue/DeviceEpoch.
Conversion submission completes and cancellation/epoch checks pass before the
frame returns. Native worker results carry the existing document/preview key and
attempted epoch. The UI rejects stale handles/keys/devices before registration and
rechecks target epoch and requested region/dimensions. Only the UI registers an accepted view
with egui's renderer using nearest sampling. Workers never lock that renderer.

The accepted registration owns the frame. Replacement, CPU fallback, document/tab
reset, backend replacement and App drop unregister the old texture. A rejected
completion never registers an ID. Committed/preview progress guards remain shared
with the CPU path; a GPU texture does not acknowledge an edit or save. A missing or
unsupported display target keeps/falls back to CPU-backed presentation. Headless
harnesses retain their CPU-backed path by default.

`App::rendered()` now explicitly reports the last CPU-backed display and is empty
for native GPU surfaces. Exports and clipboard/document operations use authoritative
host documents, not this optional harness/display buffer. Native consumers requiring
host bytes must request an explicit readback; it is not silently performed per frame.

## Memory and failure scope

Source reuse retains its separate bounded [layer-view cache](GPU_SOURCES.md).
Each direct frame allocates a requested-region composite buffer and display texture;
[viewport presentation](VIEWPORT_RENDERING.md) clips native requests to visible bounds.
There is no output pool, mipmap or tiled display output yet.
The accepted UI surface and unread worker result have independent lifetimes.
Declared worker accounting does not establish process-wide physical GPU/driver
memory bounds. GPU submission waits remain synchronous on the admitted worker;
queued work and pending display age still need native latency measurement.

Device loss invalidates the epoch and source views. Late textures cannot replace
a new backend or a cancelled gesture. CPU composition remains available, while
automatic recreation of a lost eframe presentation device/window is still pending.
Callbacks must not reenter a renderer while its source-cache mutex is held.

## Executed verification

Real-Metal tests compare all 65536 color/alpha pairs to an independent integer
reference and selected egui ColorImage conversion, cover clipped odd extents and
padded strides, cancel at observed composition/presentation boundaries, retry explicit
readback and reject retired device frames. Native registration tests verify stale
keys/epochs, CPU fallback cleanup and unchanged document tickets; installation also
validates dimensions before registration. An actual
egui renderer draws native and managed images side by side over a background;
nontrivial output pixels match exactly with nearest/predictable filtering and no
dithering. This validates the display path, not Windows historical reference fidelity.

[Complete texture-readiness measurements](../benchmarks/results/direct-display-2026-10-21/DECISION.md)
compare actual desktop staging, GPU work and UI registration against completed
straight readback/ColorImage conversion/backend upload. Both include cold setup and
teardown. OS input/display traces, compositor swap/presentation, GPU timestamps,
window/device recreation and broad adapter qualification remain separate gates.
