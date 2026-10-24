# Classic follow-up, October 24, 2026

This is the work record for the latest Classic feedback. It does not replace the
feature roadmap or claim native Paint parity.

## Implemented

- Arial is the default text family. Native font discovery retains Arial when
  installed and resolves an unavailable Arial default to bundled Selawik. Explicit
  other family preferences survive discovery. Interface text still uses Selawik;
  the browser retains its bundled font default.
- Stroke size paints its glyph at the center of its actual button rectangle.
- Classic initially fits the canvas using the Zoom to Window calculation. A
  restored explicit view scale takes precedence; new/opened documents fit again.
- Classic images up to 1 megapixel and 2048 pixels on either side reuse a complete
  level-zero texture across idle zoom changes. Magnified live painting/text and
  queued edits retain regional rendering to avoid widening preview work. Larger
  images retain bounded regional
  display requests. This reduces render work; native end-to-end latency is not
  benchmark-qualified by this change.
- The checkerboard is submitted as one mesh. Scrollbar geometry changes schedule
  the next frame instead of repeatedly discarding the current layout. A regression
  toggles Layers six times at both display scales and checks settled frames for
  discard requests and the egui performance banner.
- Delete in the Selection menu and the Delete shortcut erase the active layer's
  selected pixels to Classic's canvas ground. Other frontends may capture a
  transparent clearing color. No selection is a no-op. Desktop/browser use the
  same replacement-fill command; Undo restores document and selection.
- Historical ribbons now expose Outline, Fill and Size alongside their shapes.
  They retain their own image controls and do not advertise newer AI controls.
- Classic tooltips, selection rows, dialog buttons, chooser subtitle, shape
  choices, units and shared confirmation buttons use Fluent translations.
- The picker has a narrower glass tube and shorter blue bulb, following the
  supplied Paint reference. Its shared and theme-specific SVGs were updated.
- The README contains feature bullets, actual application screenshots and run
  commands. Images are in `assets/repo`; renderer/source notes stay in docs.

## Localization and historical skin follow-up

- Added Fluent arguments for tool sizes, font counts, opacity, canvas dimensions,
  history rows and recovery messages. Values such as font names and filenames are
  preserved. Classic, Studio and browser controls now translate the reported
  menu labels, text preference counts, tooltips and accessible names.
- Translation functions and language types are imported at file scope. The CI
  translation check catches missing literal entries and mismatched Fluent
  arguments in English/Japanese. It does not prove that arbitrary English prose
  outside translation calls is covered. Low-level diagnostic payloads retain
  their original details inside translated status envelopes.
- Windows 7/10 now use dedicated compact Home/View ribbons instead of the v11
  menu arrangement. Clipboard, Image, Tools, Shapes and Colors follow the supplied
  reference grouping; brushes have separate primary and dropdown interactions.
  Size lives in Shapes, with line-width samples and a custom numeric entry.
- Historical colors use side-by-side square Color 1/Color 2 wells, exactly ten
  custom palette slots and a rainbow rectangle for Edit Colors. Their seven-column
  shape gallery uses small blue outlines. Windows 7 has its blue application menu,
  pale ribbon gradient and gold selection treatment; Windows 10 has a flat gray
  ribbon and blue selections.
- Historical status strips are 22 points tall, with a compact percentage entry,
  circular minus/plus controls and a blue slider thumb. Their zoom actions and
  slider/touch handling retain shared editor behavior. Windows 11 keeps its own
  status presentation.
- Updated the actual offscreen captures in `assets/repo/classic-windows-7.png`
  and `assets/repo/classic-windows-10.png`. These include real edited document
  pixels; OS window borders and the macOS menu bar are outside the captures.

This improves the reference layout; it does not establish full skin or feature
parity. Historical File backstage panels, some original artwork, native Windows
window decorations and version-specific details remain. Clipboard operations and
Magnifier remain explicitly disabled. Existing canvas centering is retained.

## Brush comparison

Inspected the author's actual numbered Paint stroke image in
[Rating Microsoft Paint brushes](https://spicymeatballsandwich.wordpress.com/2025/06/12/rating-microsoft-paint-brushes/)
and the first-hand illustrated
[Paint brush tutorial](https://cynmackley.com/2014/02/09/ms-paint-101-brush-and-color-selection/).
The numbered image was downloaded for visual inspection, not redistributed.
The following is PaintCraft's actual editor output, with all media at 32 pixels;
its dimensions and pointer path do not match the published Paint sample.

![PaintCraft brush samples, numbered 1 through 9](../assets/repo/brush-samples.png)

From left to right: Brush, Calligraphy brush, Calligraphy pen, Airbrush, Oil,
Crayon, Marker, Natural pencil, Watercolor. This also makes remaining differences,
such as the regularity of the oil bristles, visible for follow-up.

The visible airbrush sample has discrete particles, whereas PaintCraft had a
uniform radial wash. Airbrush now uses deterministic spray particles with radial
density. Oil/watercolor samples visibly taper/fade; their PaintCraft strokes now
lose charge with traveled distance and reset on the next gesture. Travel includes
clipped segments so viewport/selection clipping does not reset the material.

Calligraphy retains opposing diagonal nibs; marker retains its rectangular nib;
crayon and natural pencil retain different grain densities. These were visual
comparisons at uncontrolled sizes, not matched-input golden tests. Native Paint
version, pointer velocity, time-based spray accumulation, pigment mixing and wet
paper behavior remain unqualified. Existing preview/commit identity, clipping,
retracing, cancellation and resource bounds still apply.

## Still required by this feedback

1. Binary selection masks, free-form creation, inversion, floating movement and
   secondary-color-keyed transparent selection. Merely checking a menu item would
   not implement these behaviors. The current engine projection is rectangular.
   The [binary R8 storage foundation](SELECTION_MASKS.md) is implemented; its
   session/history/tool integration remains work.
2. Arbitrary-angle rotation with agreed resampling, bounds, selection and alpha
   rules across native, CLI and browser. Orthogonal rotations already work.
3. Subject-aware background removal. A border flood fill would not match Paint's
   subject detection, so it must not be substituted under that name.
4. Windows 7/10 File backstage panels, clipboard workflows, remaining historical
   artwork and version-specific behavior. Home/View tabs and ribbon grouping are
   implemented; full presentation or feature parity remains unqualified.
5. Remaining low-level diagnostic payload localization and continued
   localization/accessibility audits. Literal catalog checks do not establish
   complete prose coverage.
6. Sustained-input native zoom latency and matched-input Windows brush captures.

Crop already works after a nonempty canvas selection, across native/browser/CLI,
and has an end-to-end pixel/Undo regression. Without a selection its control
explains that an area must first be selected.

## Validation

The complete workspace run passed 1,205 tests with 60 ignored tests across 107
suites. The final native preview-region guard then passed all 230 ordinary GUI
tests (48 opt-in captures ignored), including magnified preview/release pixel
identity. The browser suite passed all 57 tests, including fitted startup,
selection erasure, crop and curve-preview release coordinates.

Workspace Clippy, WebAssembly Clippy and the final native all-target Clippy run
passed with warnings denied. The browser release distribution built successfully.
Four real offscreen application captures and a real editor brush sheet were
exported and visually inspected. These captures do not include OS window borders
or the native menu bar. Source length and whitespace checks passed.

An existing thumbnail cancellation test exposed a queued-versus-running race:
cancelling before the closure starts correctly drops its receiver. Its intended
running-job scenario now waits for a worker-start handshake. The targeted
thumbnail tests and complete workspace run pass with that synchronization.

Local raw evidence is archived in the ignored directory
`docs/validation/classic-feedback-2026-10-24/`. Live Windows latency, real tablet
input, complete translation coverage and native Paint golden comparisons are not
established by these checks.

The subsequent localization/ribbon workspace run passed 1,220 tests, with 60
opt-in tests ignored, across 108 suites. Workspace and WebAssembly Clippy passed
with warnings denied. The browser release distribution rebuilt successfully.
Updated offscreen captures were rendered and visually compared with the supplied
Windows 7 and Windows 10 reference folders. The final source checks also enforce
the 800-line limit and matching translation arguments. Raw follow-up logs are in
`docs/validation/ui-localization-skins-2026-10-24/` (ignored).
