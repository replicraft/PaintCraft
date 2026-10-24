# Graphics library responsibilities

## wide: batches of independent pixel operations

`wide` is an explicit SIMD API. It does not make an arbitrary algorithm faster.
A scalar loop may already be vectorized by LLVM, and explicit channel extraction,
packing, branches and unsupported operations can cost more than the vector work
saves. Measure the complete operation with normal compiler optimization enabled.

The first compositor prototype split each packed RGBA pixel into three RGB
channel vectors. The second processes red and blue in independent 16-bit fields
within each u32 lane, plus a separate green channel. The third widens four pixels
to two u16x8 vectors. Compile-time fixed vectors and one opacity vector per span
avoid repeated vector construction inside its four-pixel loop. All retain exact
alpha/opacity rounding, transparent hidden RGB behavior, mixed-destination fallback
and scalar tails. The measured u16 path is selected by default on AArch64 NEON, x86/x86-64 SSE2 and Wasm SIMD128 builds.
Other targets use the scalar specialization. M1 Pro timing is the current
performance evidence; x86 correctness and both Wasm build modes are verified.
The selection and numerical results live in the
[BF02 decision](benchmarks/wide-compositor-2026-10-24/DECISION.md).

Other concrete candidates are:

- Display RGBA premultiplication. Desktop `app/rendering.rs` and browser
  `canvas.rs` currently call egui's `ColorImage::from_rgba_unmultiplied`.
  This has fixed-divisor arithmetic and contiguous output. Compare the complete
  ColorImage conversion, including its allocation, against egui's optimized path.
  Do not insert an additional temporary pixel allocation to accelerate one loop.
- Bulk brush/tile blending after admission is another possible consumer of the
  verified arithmetic. Preserve COW storage, selection clipping and cancellation
  rather than moving vector writes ahead of resource admission.
- Binary selection tile operations such as union/intersection when those editing
  operations are connected. Dense samples are currently 0/255 bytes, so byte-wide
  bit operations are suitable. Existing mask inversion only changes a view flag
  and is already constant-time; replacing it with a SIMD scan would be a regression.
- Display box-filter accumulation, where source pixels are contiguous. The existing
  `display::Sum` uses bounded u64 accumulators. A vector version needs exact sums,
  correct cell boundaries and overflow analysis; byte lanes alone cannot hold them.
- Future implemented adjustments and convolution kernels. They need defined color,
  edge and rounding policies before performance comparisons are meaningful.

These are candidates, not implemented improvements or claims about brush latency.

## glam: geometry and transformations

The small import surface is intentional. `paintcraft-core::types::coordinates`
wraps `DVec2` and `DAffine2` in space-typed Point, Displacement and Affine types.
`paintcraft-ui::View` consumes these for screen/document mapping, inverse hit
coordinates, subpixel brush positions and visible-region bounds on desktop and
web. Actual execution is broader than the number of direct `use glam` statements.

The present integration is nevertheless limited. Glam can also support:

- Arbitrary rotation, scale and skew using inverse affine pixel-center mapping
  for image resampling. Interpolation and pixel admission remain separate policies.
- Selection and text transform handles, rotated bounds and local/document/screen
  conversions. Reuse the typed spaces rather than mixing raw matrices in widgets.
- Continuous brush tangent/normal, projection and distance geometry. Preserve
  rebasing before float conversion; changes to operations or rounding need brush
  coverage comparisons near pixel boundaries.
- Batched preview/handle geometry and future transformed layers.

Integer pixel bounds, tile addressing, deterministic coverage and file layouts
remain PaintCraft-owned contracts. Do not replace them with float vectors or
serialize glam's in-memory layout. Do not convert egui layout vectors to glam just
to increase usage. The existing Point/Displacement/Affine wrapper is the shared
extension point; add operations when a real tool consumes them.

Glam provides vector, matrix and affine types in multiple precisions. SIMD use
depends on the type and target; a DVec2/DAffine2 import is not proof of a speedup.
See the [glam documentation](https://docs.rs/glam/latest/glam/).

## epaint: existing UI drawing and tessellation

Epaint is already part of egui and is re-exported as `egui::epaint`. Studio's
`colors.rs` builds an `epaint::Mesh` for its color wheel; ordinary egui Painter
calls also produce epaint shapes. A second direct dependency is unnecessary for
these consumers and could cause version drift if declared independently.

Use it for custom UI meshes, cursor/selection overlays and presentation geometry
that belongs in egui's logical-point coordinate system. Keep document pixel
composition and undoable brush/text rasterization in the engine. UI triangle
antialiasing is not automatically the document raster policy, and its output is
not the persistent editable document representation. A direct epaint dependency
would make sense only for an integration that deliberately operates without egui.
See [epaint's documentation](https://docs.rs/epaint/latest/epaint/).
