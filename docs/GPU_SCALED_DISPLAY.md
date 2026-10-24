# Exact bounded GPU scaled display

`GpuRenderer::render_display_texture` accepts the same validated `DisplayRequest`
as the [CPU oracle](MULTIRES_DISPLAY.md). Level zero delegates to native-resolution
GPU presentation. Positive levels compose ordered native RGBA8 pixels before box
filtering and final egui premultiplication. This is the provisional encoded-sRGB
filter, not qualified historical Paint sampling.

Output is one independently owned coarse RGBA8 texture. Output tiles have a
configurable edge of 32 or 64 samples; changing that edge cannot change sampling
coordinates or numerical results. Metadata proofs fill complete uniform composite
tiles without native pixel expansion. Unknown source regions recursively split
into at most 256-square blocks, use the existing ordered GPU compositor and feed
one bounded accumulator. Uniform subregions contribute exact weighted counts.

Wide sums use pairs of unsigned 32-bit shader words, with explicit carry,
multiplication and exact rounded integer division. No optional shader-int64 feature,
floating-point quotient, per-layer downsampling or production pixel readback is
required. Canvas-edge boxes count only actual canvas pixels. Source scratch cannot
be recycled before its dependent accumulation completes. Previously returned frame
textures remain independently readable after later renders or idle-cache eviction.

The current implementation waits for each dependent submission. Optional
[bounded layer batches](GPU_BATCHING.md) reduce compositor submissions while
preserving native ordering and completion before reduction. They add an immutable
pending-resource allowance to native worker admission. This does not claim
asynchronous scheduling, tiled texture presentation, output-cache reuse or physical
GPU memory admission. The immutable 2 MiB scratch allowance describes bounded
working declarations; callers must separately charge coarse output plans and source
cache allowances. Device texture/storage/dispatch limits return ordinary resource
errors. Cancellation and retired epochs prevent partial frames from returning and
retire potentially incomplete source-cache work.

Desktop integration validates sampling level as well as source bounds, output size
and document/preview/device identity before registering a texture. A scaled-path
resource failure can use the CPU oracle without disabling a still-current native
GPU. Headless straight-pixel consumers continue using CPU scaled pixels rather than
unpremultiplying display bytes. Device retirement still rejects old completions.

Tests compare both tile edges with CPU boxes across offsets, alpha, clipping, partial
edges and sampling levels; retain old live frames; exercise dense multi-block carry;
and compare analytic trillion-pixel composites against independent u128 sums.
Real egui framebuffer comparisons cover native and scaled textures. Hardware tests
require the renderer's `gpu` feature and `PAINTCRAFT_REQUIRE_GPU=1`; a zero-test run
or unavailable adapter is not evidence of GPU correctness.

[The native comparison](../docs/benchmarks/gpu-scaled-display-2026-10-21/DECISION.md)
retains raw statistical/process results. The native benchmark driver compares the
actual cached CPU scaled path with completed managed upload against both GPU tile
edges with completed native registration. Tiny, dense multilayer, sparse 4K brush
and uniform 4K opacity-edit controls are included. This implementation alone does
not establish a general CPU/GPU crossover. CPU scaling remains automatic; explicit
`PAINTCRAFT_GPU_SCALED=1` native experimentation uses 64-sample output tiles, with
CPU fallback on ordinary scaled-resource failures. Native-resolution GPU display
remains enabled regardless of the option.
