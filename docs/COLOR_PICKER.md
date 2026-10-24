# Document color picker

Classic's former disabled picker is now a real tool in desktop and web. Primary
click installs the sampled color into the primary slot; secondary click installs
it into the secondary slot. Sampling occurs at the final pointer position on the
matching release. Dragging retains at most the first/last positions and produces
no paint preview. Escape/focus loss before release cancels the gesture.

Classic samples one pixel from the ordered visible image, then restores the
previous tool after success. Studio defaults to raw active-layer sampling and
keeps the picker active. Its toolbar exposes Image/Layer, 1 × 1, 3 × 3, 5 × 5,
11 × 11, 31 × 31 and 51 × 51 sampling, plus keep picker / previous tool / switch to
Pencil. Ctrl at press temporarily captures Image mode without changing the stored
Layer choice. The gesture captures the target layer, button/slot, source, size,
original slot color and post-click tool choice.

The Studio choices follow the feature categories in the official
[Paint.NET picker documentation](https://paint.net/doc/latest/ColorPicker.html).
This does not establish native-equivalent averaging, defaults or complete input
behavior. Single-key/temporary-tool shortcuts and a live sampling loupe remain
work. Classic's automatic tool return remains a defined profile choice pending
versioned Windows reference qualification.

## Sampling rules

The shared renderer samples document coordinates from immutable document pixels,
independently of selection masks, viewport zoom, grid/checker decorations or
presentation caches. The center must be inside the canvas. A neighborhood clips
to the canvas; its denominator is the actual clipped pixel count.

Image mode uses the normal ordered RGBA8 compositor, retaining root order, finite
layer bounds, visibility, opacity and intermediate rounding. Invisible/zero-alpha
source RGB cannot enter a flattened sample. Layer mode samples exact straight
RGBA, including hidden RGB, without applying layer visibility or opacity. Pixels
outside that finite layer contribute transparent black, irrespective of its
implicit default. A hidden active layer can therefore be inspected in Layer mode.

Neighborhoods average the four encoded straight channels independently, with
integer half-up rounding. No linear-light conversion, alpha weighting or screen
color readback occurs. This is a specified current policy; native Paint.NET's
precise neighborhood arithmetic remains unqualified. Image-mode output allocation
is bounded to 51 × 51 × 4 bytes (10,404); Layer mode accumulates directly. Huge
uniform canvases require no full-frame allocation. Cancellation returns no sample.

## Read-only ownership

Picking updates session color/tool state. It creates no batch, ChangeSet, history
entry, publication epoch, dirty document or native file change. An explicit sample
also leaves an unfinished text draft unpublished. The existing explicit tool-change
flow may place text before selecting a different tool; that remains a separate edit.

Desktop uses the existing ordered native editing worker. Sampling captures the
immutable document after earlier accepted edits publish, and shares that service's
current provisional preparation charge/admission (not a physical allocation claim).
Later document commands wait behind the sample. Before updating a slot, completion
checks document owner/history ticket, picker revision and the original slot color.
New gestures, explicit tool activation/re-selection, another slot value, stale
owner/selection state or cancellation prevent a late update and automatic return.
Layout synchronization of an unchanged tool preserves an outstanding request.
Read-only color/tool completion requests a UI repaint without publishing pixels.

Web runs the same bounded kernel synchronously and copies successful results back
into its shared controls. The slots remain session state, like other primary/
secondary color choices; project downloads and browser preferences do not acquire
new pixel/palette persistence from this feature.

## Qualification (2026-10-24)

- [Sampling tests](../crates/services/paintcraft-render/tests/sampling.rs) compare
  every neighborhood size and canvas edge against full ordered rendering plus
  an independent channel sum; raw hidden RGB, invisible layers, opacity, finite
  bounds, transparent sources, huge sparse canvases, invalid centers and cancellation
  are covered.
- [Tool tests](../crates/editing/paintcraft-tools/tests/color_picker.rs) cover
  button ownership, fixed-point positions, no ink preview, captured settings,
  cancellation, slot/tool changes, explicit re-selection and after-click behavior.
- [Native controller tests](../apps/paintcraft/src/controller/picker/tests.rs)
  exercise the actual worker and gates: preceding ink, no extra publication,
  source-slot/tool/document staleness, cancellation, hidden layers, selections and
  an unpublished text draft. The native library suite passes 263 tests (8 ignored).
- [GUI tests](../apps/paintcraft/tests/gui/color_picker.rs) cover Classic at 1×/2×,
  both buttons, exact source pixels, unchanged snapshot/ticket/selection and tool
  restoration. Studio tests use a real layered PCND, hidden active layer, all
  settings categories, Ctrl-image override and a 51 × 51 sample. The full GUI suite
  passes 219 tests (45 ignored); the final ownership changes also have targeted
  guard/GUI checks. The [web test](../apps/paintcraft-web/src/picker_tests.rs)
  verifies clean-document sampling and control synchronization without changing
  the selection/history; all 49 web tests pass. Shared render/tool/UI suites,
  strict workspace/wasm Clippy, formatting and source-length checks pass.
- An isolated Brave/WebGPU scene picked primary/secondary colors and downloaded
  the project before/after each pick: both pairs are byte-identical. Drawing after
  a successful pick uses the sampled color with the automatically restored tool.
  Final PNG/swatch checks are recorded in the ignored validation artifacts. This
  is browser-path evidence, not Windows/OS input, tablet/IME, cross-browser or T22
  conformance qualification. No performance gain or physical memory claim is made.

Generated projects, screenshots, scripts, hashes and logs stay in ignored
`docs/validation/color-picker-2026-10-24/`. T14/T15 remain partial.
