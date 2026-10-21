# Direct single-layer native display

`GpuRenderer::with_single_layer_display(true)` opts into exact source-to-display
conversion for native-resolution requests. The default remains the existing ordered
compositor after mixed native comparative evidence. The desktop opt-in is
`PAINTCRAFT_GPU_SINGLE_LAYER=1`. No dependency or document/wire policy changes.

## Eligibility and numerical contract

The current document type supports only the flat default-v1 composition policy.
Exactly one potentially contributing root must fully cover the requested viewport.
Invisible, zero-opacity, fully clipped and provably empty transparent layers do not
contribute. A second potential contributor or a partially covering layer selects
the original ordered compositor. Resident transparent tiles may conservatively
prevent eligibility; proving them empty is not required for correctness. No new
mask, group, blend-mode or color/profile semantics are invented by this path.

The source view uses the existing bounded device cache, with layer-local samples
clipped and packed to the requested document region. A single compute pass applies
`floor((source_alpha * layer_opacity + 127) / 255)`, then ecolor's exact encoded-sRGB
premultiplication into a fresh Rgba8Unorm display texture. One source over transparent
black has unchanged straight RGB whenever its rounded alpha is nonzero; when zero,
premultiplication emits transparent black. Hidden stored RGB is never rewritten.

This eliminates the private intermediate compositor buffer and its submission/wait
for eligible displays. Source upload/patch work and display conversion share one
encoder and observed completion boundary. `single_layer_displays` reports the
bypass; `composition_submissions` excludes this fused display pass, just as it
excludes the existing separate display conversion. It is not a total queue counter.
Straight export/readback composition, multiresolution filtering and ineligible
native displays retain their existing paths.

## Ownership and admission

No authoritative pixel or CPU tile lease is borrowed into mutable GPU storage.
Prepared source views reference the cache's derived buffers only within serialized
renderer access. Their keys publish after successful completion and fresh
cancellation/epoch checks. Public frames own independent display textures, not the
mutable cached source. Patching a later source therefore cannot change an older
frame. Failed/cancelled requests follow the existing cache/pool invalidation path;
submitted work uses the same admission and completion-observation guards.

The pipeline is lazy and renderer-local. Existing conservative native source/output
allowances remain; bypassing an allocation does not lower admission or establish a
physical GPU/RSS bound. Scratch retained from earlier general renders is still
subject to its ordinary pool policy. Native device recreation remains separate.

## Executed correctness and measurement scope

Real Metal tests exhaust all 65,536 source-alpha/layer-opacity pairs against direct
integer expectations. Additional tests cover odd viewport extents, negative layer
offsets, sparse hidden colors, unchanged source reuse, row-band patches, old-frame
ownership, straight export, conservative eligibility, zero-retention streaming,
every cancellation checkpoint and epoch retirement. Desktop tests compare actual
egui framebuffer pixels and exercise admitted worker preparation/publication.
These are default-v1 checks, not Windows Paint or Paint.NET reference qualification.

The `single-layer` suite in `benchmarks/tools/run_native_viewport.py` compares the
existing ordered display with the contender through real desktop preview staging
and App texture registration. It retains cold device/pipeline/fixture setup and
teardown, eight completed displays, raw Criterion/process samples, calibration,
untimed process RSS and independent command/scalar output checks. Tiny, dense,
large viewport and opacity-only cases have one source; multiple-layer and partial
coverage cases are fallback controls. It does not measure OS presentation, input
latency, physical allocation or general CPU/GPU crossover.

[The completed native comparison](../benchmarks/results/gpu-single-layer-2026-10-21/DECISION.md)
retains all 240 statistical samples and 780 process observations. Intermediate
work falls as intended, but fallback variation, a large-view regression and RSS
results reject global enablement. The implementation remains explicitly selectable.
