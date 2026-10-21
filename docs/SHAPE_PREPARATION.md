# Streamed shape preparation and exact large ellipses

Rectangles and ellipses now expose `visit_filled` and `visit_outline`: clipped spans
flow into a fallible sink without an intermediate geometry vector. The editor uses
these visitors to populate its existing bounded PaintPlan. Fill and outline still
share one operation plan, retaining fill-before-outline order and exact per-pixel
blending. ShapeKind::Line and unsmoothed brush span unions retain their other path.

## Cancellation and ownership

Visitors check cancellation on entry, before each visited row and after traversal.
Rows with no emitted span still check, including thin ellipses with long empty
caps. Sink errors stop traversal immediately and preserve their original error
context. The editor forwards its worker token and lets descriptor limits reject
additional spans as they are produced. Pixels are prepared/replayed only after
both geometry phases succeed. Failed/cancelled private plans and earlier private
batch prefixes are discarded by the existing Session publication boundary.

Legacy `filled`/`outline` collection functions remain available for callers that
own their output allocation policy, with valid rectangle bounds/nonnegative outline
thickness. They collect through the same visitor. They retain their infallible Vec
interface; callers requiring recoverable allocation use a fallible sink instead.
No claim is made that every legacy raster API now has bounded geometry allocation.

## Numeric behavior

The existing default-v1 ellipse pixel-centre predicate remains exact:
`u²*h² + v²*w² <= w²*h²`, with `u=2*x+1-w` and `v=2*y+1-h` in box-local coordinates.
Large legal shape endpoints previously overflowed intermediate signed products,
even when the requested canvas was tiny. Row preparation now computes squared
terms in u128 and uses checked multiplication for the ordinary case.

When the product needs more bits, the row bound is the largest integer `m` such that
`m²*h² <= w²*(h²-v²)`. Four u64 partial products produce an exact `(high, low)` u256
comparison; bisection takes at most 63 steps for a positive i64 extent. There is no
floating quotient or approximate fallback. The ordinary integer-square-root path
uses a floating seed only, with exact integer corrections. Pixel results do not
depend on that seed's rounding. This is a private numeric helper, not a new public
big-integer type or dependency.

Editor endpoint differences and inclusive sizes are checked before constructing
the box. Visitors reject unrepresentable extents with a resource error. Outline
insets use wide arithmetic, and empty local pieces are rejected before adding the
world origin, avoiding `x0-1` overflow at signed endpoints. Zero thickness produces
no outline; oversized thickness makes a solid figure; negative thickness is invalid.

## Executed scope and remaining work

Small shapes match an independent pixel-centre/inset oracle over 2,880 figure,
size, clip and thickness combinations. Wide products match independent schoolbook
u32 limbs, including boundaries and 2,048 deterministic random pairs. Large clipped
ellipses are checked at centres and independently located edges, through i64-sized
extents. Tests cover signed endpoints, per-row cancellation and fallible sink exit.
Editor tests commit a clipped ellipse whose endpoints reach the admitted coordinate
limit, then undo/redo its exact pixels, and cancel rectangle/ellipse preparation
after an earlier private edit without changing owner/history state.
[The validation record](validation/shape-preparation-2026-10-21/validation.json)
pins source, commands and logs.

This is correctness and cooperative preparation, not historical Paint/Paint.NET
parity, an input/display latency result, a throughput improvement or an RSS ceiling.
Wide arithmetic and callback overhead remain unqualified by timing. Earlier
benchmark decisions retain their frozen source scope. Stroke union construction,
built-in font geometry and other kernels still need appropriate checked preparation;
whole-batch planning, aggregate geometry admission, physical budgets and native
cancellation/display tails remain open.
