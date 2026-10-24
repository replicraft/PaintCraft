# Free-form selection and Invert

Classic 7/10/11 and browser Classic expose Rectangular and Free-form selection in
the Select dropdown. Its icon and checked mode follow the chosen selection shape.
Studio exposes the same modes in its selection tool options. Invert selection is
available from the Edit menu, Classic selection dropdown and Ctrl/Cmd+I, including
the native macOS menu. Existing text, IME, modal and exact-modifier guards apply.
With no active selection, Invert selects the whole canvas.

A primary-button lasso retains the entire integer document path and closes on
release. Fewer than three distinct vertices leave the previous selection alone.
A completed polygon uses the engine's even-odd, pixel-center binary rasterization;
its sampled mask is authoritative. Selection-only publication and Undo/Redo retain
content StateId, document pixels and native save cleanliness. Painting, flood
connectivity and Delete consume exact coverage through the shared engine.

Mode is captured at pointer-down. Lasso growth has fallible point/capacity checks,
including the release position: native allows at most 65,536 points and the browser
retains its 8,192-point allowance. Exceeding a limit cancels the complete gesture;
a truncated prefix is never submitted. Lasso is separate from queued painting
ownership. Escape, cancellation and foreign-button release retain existing guards.

Draft feedback shows the closing path as a view overlay. It does not create a
raster preview document, mutate the session or add history during movement.
Browser texture keys omit selection-gesture revisions so motion does not repeatedly
recompose the image. Committed feedback comes from actual mask boundaries, including
holes and the finite outer edges of an inverted selection. It is not a rectangle
around the polygon. Scrolling clips true edges without adding viewport borders.

The raster outline constructor accepts explicit work, segment and shared byte
bounds; uniform full/empty fields need no pixel-area scan. Scratch and retained
geometry are capacity-priced. Failure or cancellation exposes no partial outline.
The host cache retains one immutable mask/viewport result, releases old geometry
before replacement, and clears when selection or the active document is absent.
Successes reuse geometry; cached resource failures can retry after budget activity.
The display policy allows 2,097,152 work units and 16,384 segments per outline,
sharing a 16 MiB process geometry realm. If extraction exceeds its bounds, the UI
shows a translated notice to zoom in. Selection coverage and editing remain intact.
These are declared payload/work bounds, not application RSS or latency guarantees.

[Tight sampled dimensions, Crop, Fit Selection and exact rotate/flip](MASKED_SELECTION_TRANSFORMS.md)
now work for proven nonempty mask selections. Delete retains its independent
capability and Classic canvas-ground/Studio transparent clearing policies.
Active empty masks display 0 × 0. Bounded bounds-extraction failure omits unproven
dimensions and disables dependent actions. Floating movement, transparent color-keyed
selection, fractional masks and matched native behavior remain work.

## Evidence

Tool tests cover captured modes, all path points, button ownership, invalid/short
paths, exact schema cap, final release admission and cancellation without a partial
selection. Raster tests compare 12,288 field/view combinations with an independent
pixel-edge oracle, including inversion, translation, clipping, holes and viewport
contacts. They inject every observed cancellation and resource/work/segment denial.
Cache tests check reuse, final-reader ownership and retry after capacity release.

Real pointer/menu/keyboard GUI tests cover all seven languages in native Classic
7/10/11 plus Studio (28 cases) and browser Classic (21 cases), with 1x/2x display
scales and forced repeated layout passes. They check every selected and painted
pixel for a concave polygon and its complement, selection-only cleanliness, Delete,
Undo/Redo, Deselect and returning to rectangle mode. Shape-output assertions require
the actual concave outline and reject a fabricated bounding rectangle. Browser
checks also confirm independent erase/crop/transform capabilities.

The workspace regression run passed 1,394 tests with 75 explicit ignores across
102 result summaries. The two new frontend matrices passed separately after their
final test registration and outline assertions. Final strict workspace Clippy,
Wasm checking and the portable browser release build passed, together with format,
source-length, whitespace and the 919-message, seven-language translation audit.
Raw logs and source recording are local under the ignored
`docs/validation/lasso-selection-2026-10-24/` directory. These are headless host
checks; physical pen/IME devices, real OS-menu interaction, native reference builds,
interactive latency and cross-browser qualification remain separate evidence gaps.
T14/T15/T19 remain partial and the T16/T22 conformance gates are not promoted.


## Floating selection follow-up, October 24, 2026

[The floating selection tranche](FLOATING_SELECTIONS.md) now implements exact
integer movement and transparent RGB-keyed placement with frozen source pixels,
private previews, bounded admission and one recorded change. Native Classic/Studio,
browser Classic and CLI have independent pixel and lifecycle evidence. Remaining
fractional, custom-angle, clipboard and native qualification work stays open.
