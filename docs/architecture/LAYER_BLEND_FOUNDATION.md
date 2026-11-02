# Layer blend foundation

Multiply and Screen are being integrated as stored, editable layer properties.
The current slice covers numerical behavior, typed editing, persistence, history
and scoped serial CPU composition. It does not expose frontend blend controls or
enable GPU/browser blend execution.

## Numerical contract

`paintcraft_core::types::LayerBlendMode` defines Normal, Multiply and Screen.
Normal calls the existing source-over implementation. New modes use checked-range
integer arithmetic over straight encoded RGBA8, preserving the exact backdrop
when source alpha is zero. Independent rational fixtures cover translucent
backdrops, hidden RGB, extrema and rounding. This establishes the numerical
foundation, not application or proprietary-renderer parity.

The eventual compositor applies filters, the own-layer mask and layer opacity
before ordered layer blending. Native composition precedes composite LOD
reduction; display premultiplication follows it. GPU and browser execution need
separate qualification of this sequence.

## Stored intent

Layer metadata, compact staging and hot/cold history retain the mode independently
of visibility and opacity. Typed `layer.set_blend_mode` belongs to the closed
`paintcraft.batch/layer-appearance-v2` profile. Clear Canvas restores Normal.

Native PCND8 records store the mode and declare `layer-blend-modes.v1`.
All-Normal documents retain their previous minimal PCND encoding; removing the
last non-Normal mode returns to that branch. Unknown tags and nonminimal
all-Normal PCND8 streams refuse. Layers, filters, masks and source metadata remain
part of the same persistent document.

Actual engine receipts disclose stored modes through result/v6 and CLI
file-command/v6. These schemas do not expand older frozen discovery bundles.
Older inspection, MCP and foreign-copy routes remain closed to unsupported modes.

## Local evidence and limits

The metadata integration records are under
`docs/validation/layer-blend-metadata-integration/`. Independent PCND fixtures,
real Session receipts, malformed-schema controls and original failed attempts are
retained separately. Receipt qualification covers 72 real results and 72 actual
CLI envelopes across 36 feature combinations; schema constraints and the stricter
producer sorting contract are checked separately.

The same-target before/after probes observe no size change in the measured native
Layer/Document/SavedLayer/Packed or CPU/GPU cache controls. A real Wasm probe also
observes unchanged public Layer/Document sizes and the fresh-decoder retained quote
for its finite two-pixel specimen. These are actual layout observations, not
universal memory, RSS, latency or private Wasm-layout claims.

Rendering and frontend adoption remain separate gates. Unsupported consumers
must reject stored modes before installing a replacement document or producing a
visual copy. No provisional blend execution price or higher resource cap is
activated by this metadata slice. Full Palette blend/style parity remains open.

## Scoped CPU composition

Serial native rendering now evaluates Multiply and Screen with the existing
caller workspace. The fixed opaque-backdrop reduction is exact; translucent
backdrops retain the general integer implementation. Filters precede the own-layer
mask, then opacity and ordered blending; complete native composition precedes LOD.
Normal numerical kernels remain unchanged.

New modeled work uses 32 extra source-action units per proved-opaque sample and
256 for the general call, plus bounded context setup. These are policy units,
not instruction counts or measured timings. No new counter, pool, cap or mask-query
revision is introduced. Captured base/Noise ownership remains paired. Public nested
requests recheck their actual ROI and remaining budget; cache misses and dirty
patches stay inside the requested footprint.

Local gates pass 262 Render tests across 17 targets, including 54 required-adapter
GPU tests on Apple M1 Pro Metal. The new GPU test proves refusal, not blend arithmetic
parity. Independent literal checks, true partial dirty patches, exact-budget calls,
cancellation, nested reentry and native/Wasm strict checks pass. Original compile
and test-fixture failures remain under
`docs/validation/layer-blend-cpu-integration/`. Active control sizes are observed on
the native target; private Wasm layouts remain unclaimed. The separate
[CPU measurement record](../performance/CPU_LAYER_BLENDS.md) reports the finite
host workloads, raw evidence and unresolved comparisons.

GPU composition, frontend adoption, foreign-copy consent and new browser execution
authority remain open. Existing frontend guards and closed consumers stay in place.
