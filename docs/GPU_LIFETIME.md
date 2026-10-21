# GPU lifetime and render completion

The GPU compositor remains derived working data. Committed pixels, undo/redo
records and saves use host-backed document/history storage. A device event never
publishes an edit, changes a session ticket or acknowledges a save.

## Epoch ownership

`paintcraft_render::gpu::DeviceEpoch` is a process-local lifetime identity shared
through Arc, with an irreversible atomic retirement flag. Clones identify the same
lifetime; a fresh epoch cannot alias an older retained one. It is not a document
ID, wire identifier or reusable numeric GPU slot.

`GpuRenderer::new` installs a loss callback that retires its epoch. A host that owns
an existing callback uses `with_epoch` instead: create one epoch for the device,
retire it on loss/reset, and give a fresh device a fresh epoch. `with_epoch` does
not replace the host callback. The desktop owns this callback explicitly; it
retires the epoch and requests repaint. It does not issue GUI commands, navigate
history or mutate documents from the callback.

Rendering checks cancellation and the epoch at entry, between stages, while gathering
source rows, before submissions, after layer completion, before readback access
and after copying the final host result. Calls already executing and submitted GPU
work cannot be preempted. Existing worker reservations remain charged until actual
completion is consumed; cancellation does not imply immediate resource release.

## Desktop completion boundary

Every native worker frame carries its attempted device epoch, including a CPU
fallback computed after a GPU error. The UI accepts a frame only if its job handle,
document/preview ticket and current device identity match. GPU success additionally
requires a current epoch. A fallback from an old device cannot disable its
replacement or change the current canvas/status.

A backend replacement cancels the pending frame, clears its accepted handle and
invalidates the texture key. It preserves host pixels, editing state and history.
When the active compositor epoch is retired, the UI removes that compositor and
schedules CPU composition from the current host document. Previously displayed
host pixels may remain while the replacement result is prepared. This does not
promise instantaneous failure detection or input/display latency.

## Executed checks

The original epoch-only validation recorded 618 passing workspace tests with 21
explicit ignores. The later [source-residency contract](GPU_SOURCES.md) records
the expanded GPU path and current validation evidence. Strict workspace/all-target
Clippy, formatting, file limits, release desktop/CLI builds and an isolated
30-frame native-window smoke pass. The opt-in required GPU run is:

```sh
PAINTCRAFT_REQUIRE_GPU=1 CARGO_INCREMENTAL=0 cargo test -p paintcraft-render --features gpu --test gpu -- --nocapture
```

Three tests pass on Apple M1 Pro / Metal:

- Exact scalar parity for layer order, opacity, offsets, sparse/default pixels,
  hidden RGB, tile edges and smooth-brush edge coverage.
- Explicit destruction of a real wgpu device retires its epoch. A newly requested
  device exactly renders the retained committed document and its undo/redo states;
  independent expected RGBA arrays and unchanged ticket/snapshot identity are checked.
- Host-owned loss callbacks survive renderer construction; destruction during
  source staging rejects output. Ordinary cancellation leaves the epoch usable.

CPU-only desktop regressions inject late success/fallback frames from an unrelated
epoch and verify they cannot replace current pixels/status. Another holds admitted
work across replacement and verifies cancellation retains its reservation until
completion and leaves the published document unchanged. Epoch sharing/retirement
also has a device-independent unit test.

## Unqualified scope

Explicit `Device::destroy` on Metal is not an unexpected driver reset, physical
adapter removal, suspended device, hung queue or cross-platform qualification.
Fresh-device compositor reconstruction is tested headlessly; automatic eframe
window/surface/device recreation is not implemented. CPU composition still needs
a working presentation device to upload/display pixels in the native window.

Bounded device-local clipped layer buffers now reuse unchanged sources and update
changed row strips; errors/cancellation retire cached sources and fresh devices
start empty. Persistent sparse GPU working tiles/atlas layouts, asynchronous
transfer/readback services, physical GPU memory admission, device-aware capability
generations and GPU committed-edit chains remain incomplete. GPU work still composes a complete frame. Native presentation now consumes a
[completed display texture](GPU_DISPLAY.md); explicit readback remains available. No throughput, memory reclamation or native tail-latency improvement is
claimed by this correctness change. T13 and GPU-01..06 release qualification remain
partial; see [the roadmap ledger](ROADMAP_STATUS.md).
