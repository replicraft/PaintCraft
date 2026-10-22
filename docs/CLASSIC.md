# PaintCraft Classic

Classic is the frontend fidelity target; shared backend work is the current
implementation priority. The additional Windows 11 Paint photos
in `docs/temp/paint-references/photos/` guide this work. Studio remains available.

## New reference observations

| Image | Observed Classic UI |
|---|---|
| `11111imagekr.png` | Edit colors dialog: hue/saturation square, luminance slider, RGB/hex, basic/custom palettes, OK/Cancel |
| `i11mage.png` | Brush gallery with nine named presets and stroke samples |
| `im111age.png` | Vertical/horizontal flip menu |
| `im1a1111ge.png` | Editable percentage zoom, high zoom presets, fit, minus/slider/plus |
| `imag111111e.png` | Fractional zoom presets including 75%, 50%, 25% and 12.5% |
| `ima1111ge.png` | Rectangle/free-form selection, select all, invert, transparent selection and delete |
| `imag111e.png` | Clockwise/counterclockwise/180-degree/custom rotation |

These seven photos were inspected on 2026-10-19. They supplement the original
15-image inventory in [reference coverage](STUDIO_REFERENCE_COVERAGE.md).
They establish visible presentation and available operations; they do not
establish independently measured brush pixel semantics.

## Canvas size

New Classic documents start at **1000 × 486 pixels**. **View → Default canvas size…**
sets the configured width/height and chooses between that default and remembering
the last created/resized Classic canvas. The choice survives restart. Changing
preferences leaves the current document and history untouched.

**View → Canvas size…** stages a top-left resize with Apply/Cancel, one undo step
and preserved off-canvas content. See [the canvas-size policy](CANVAS_SIZES.md).

The desktop starts at **1200 × 850 logical pixels**, providing more room for the
ribbon and the default canvas. Smaller windows and larger canvases still scroll.
**Clear Canvas** is available beside Undo/Redo and in Edit. It replaces every
layer (including hidden and off-canvas content) with a single sparse Background
layer in the canvas color, defaulting to white. Selection does not limit clearing.
One Undo restores the prior layers, pixels and selection; canvas dimensions, title
and file association are retained. Repeating it on an already blank background
does not dirty the image. Studio exposes the Edit action with a transparent
background. Clearing is an explicit PaintCraft command, not a historical parity
claim.

## Current behavior

Classic has separate v7/v10/v11 skins, SVG tool icons, centered
small canvases, native file dialogs, layer editing, typed edits and undo/redo.
The status bar provides percentage entry, 12.5–1000% presets, a zoom slider,
fit-to-window and zoom buttons. Invalid percentage drafts are discarded on
commit. View menu actions and keyboard shortcuts use the same dispatch path.
Zoom and the optional pixel grid do not change document pixels or history.
Keyboard zoom controls the canvas, preserving the size of the application UI.

The round brush preserves pointer positions in 1/256-pixel units and stores
antialiased coverage in the document. Preview and publication use the same
rasterizer. Brush opacity is adjustable from 0–100% and is captured for the whole
gesture. Zero opacity leaves pixels and history unchanged. Eraser restores the
opaque canvas background color, defaulting to white for imported images without
a solid canvas ground. Pencil defaults to one pixel and has a size slider and
precise pixel entry, retaining hard edges and exact pixel placement. See
[brush rendering](BRUSH_RENDERING.md).

The Colors ribbon now opens a staged **Edit colors** dialog for the primary or
secondary swatch. It provides a hue/saturation square, brightness strip, RGB/HSL
fields, six-digit RGB hex entry, 48 basic colors and 28 custom slots. OK adopts
the selected RGB; Cancel/Escape discard all staged changes. Incomplete/invalid
hex disables OK. Existing alpha is preserved, including hidden RGB under alpha
zero. Hue/saturation intent survives black while adjusting the picker.

Custom colors are saved in the settings file on OK and remain available after
restart. Palette changes are tool preferences and add no document history or
unsaved marker. The optional settings-v3 `custom_colors` field defaults to empty
for existing preference files. The palette layout follows the supplied photo;
native palette values and color-processing parity remain unqualified.

The full ribbon, brush media gallery, clipboard,
selection variants, transforms and resize handles remain active development
work. This frontend is not yet a finished reproduction of Windows Paint.

## UI and theme pass (2026-10-19)

UI fidelity and themes take priority over additional editing operations. The latest
user direction permits explicit UI prototypes during this pass. The v11 ribbon
now has compact Image/Tools groups, a scrollable shapes gallery, circular color wells,
two preset-color rows and ten custom-color slots, Settings gear and a collapsible Layers panel. Crop, color picker and unsupported shapes are disabled previews; transform
popups expose their reference entries with execution disabled. Resize handles are
visual markers; View → Canvas size remains the working resize path.

Settings provides Light, Dark and System appearance, with live theme preview,
Apply/Cancel, interface/skin selection and canvas defaults. Both frontends use
the selected theme and bundled Selawik. Cancel restores the previous theme;
preferences do not alter document history. Existing settings files default to
System when the optional appearance field is absent.

Classic and Studio own separate canvas layout modules, scroll IDs and document
workspaces. Switching requires saving every changed document; the transition
offers Save, Discard and Cancel. Discard closes changed documents and retires their
recovery checkpoints; they do not reappear with discarded pixels when returning.
Save cancellation/failure keeps
the current interface. The other frontend has its own documents, histories and
view states, restored when switching back during the same application session.
The shared host still owns low-level pixel rendering and input translation into
the common editor engine. Saving and recovery cleanup finish before parking a
workspace; switching never converts or flattens its documents.

On macOS, `muda` supplies the application, File, Edit, View and Window menus.
Native semantic actions route through the same validated outbox as the ribbon.
Quit uses the existing save/discard/cancel close guard. Command-comma opens
Settings; Studio layer visibility uses Shift-Command-comma on macOS to avoid the
collision. Native file dialogs continue to use `rfd`.

### Text settings reference

`mspaint-text-settings.png` was inspected on 2026-10-19. It shows family/size,
bold/italic/underline/strikeout, left/center/right alignment and background fill.
The Classic Text toolbar now presents these controls. `fontdb` discovers family
names from installed system/user font directories on a background thread; the
searchable picker also includes bundled Selawik. Family names are deduplicated
without loading every font into egui. Discovery is native-only; headless tests
use the deterministic bundled family.

Font family/size, bold/italic, underline/strikeout, alignment and background fill
now affect real document text. Cosmic Text shapes the selected font and Swash
produces hinted grayscale coverage; preview and commit use the same resolved
mask transaction. See [the text rendering policy and library comparison](TEXT_RENDERING.md).

**Keeping text editable until save remains a prototype preference.** It does not
yet create persistent editable text objects or rasterize them on save. Text is
currently committed to pixels when placed, with one undo step.

## Interaction refinements

The Interface button beside Settings switches between independent frontends and
exposes Classic's skin picker. Ctrl/Cmd + wheel zooms over the canvas without
scaling application chrome. Pencil, brush, eraser and outlined shapes have a
size slider plus exact pixel entry. Tool size is captured at gesture start.

Fixed palette swatches now show a descriptive color name and hex in their hover
text. The first twenty use the familiar Paint-style Black/Gray/Dark red/Red/
Orange/Yellow/Green/Turquoise/Indigo/Purple and lighter-color naming; the eight
additional project swatches have descriptive names. These extra slots are not a
claim of an exact native Windows 11 palette. Studio floating panels are constrained
below the document tabs so added options cannot cover tab controls.

Shift constrains rectangles to squares and ellipses to circles while drawing.
Lines snap to horizontal, vertical or 45-degree directions. Releasing Shift
restores the unconstrained pointer geometry before placement. The eraser shows
its exact square pixel footprint, scaled with canvas zoom, with contrasting edges
that remain visible over light and dark pixels. Its overlay does not edit pixels.

The Classic menu bar supports hover switching and a nested Zoom submenu. Selection
has a rectangular-selection popup with Select all/Deselect; free-form selection
remains disabled. The Brushes popup selects the working round/square tips and
shows their current selection. Shape Outline and Fill popups select the supported
solid/no-paint combinations. Textured brushes, extra shape implementations and
other reference menu commands remain incomplete; this is not full menu parity.

## Brush and Image popup reference pass

The nine-entry brush list follows `i11mage.png`: Brush, Calligraphy brush,
Calligraphy pen, Airbrush, Oil brush, Crayon, Marker, Natural pencil and Watercolor
brush. Original, deterministic sample illustrations distinguish the entries.
Only Brush currently selects a supported native-style tip; the other eight media
are disabled and explain their availability on hover. Square brush remains a
working PaintCraft extension below a separator. The gallery fits available screen
height and scrolls on smaller windows. These decorations are not previews of
implemented document stroke algorithms.

Rotate exposes right/left 90°, 180° and custom entries from `imag111e.png`. Flip
exposes vertical then horizontal from `im111age.png`. These are presentation
prototypes: every transform entry remains disabled and cannot create history or
alter pixels. Selection also exposes disabled invert, transparency and delete
entries from `ima1111ge.png`, alongside the working rectangular/select-all/deselect
operations. Clicking an unavailable choice leaves the popup open; Escape or an
outside click dismisses it. This follows the user's UI-first priority and does
not establish native operation parity.

## Workspace View controls

View → Status bar now changes the actual bottom panel and gives the canvas the
released space. Its optional `classic_status_bar` settings-v3 field defaults to
true for older files and survives restart. This Classic preference does not hide
Studio's independent status presentation or change document pixels/history.

View → Full screen and F11 issue the same native viewport command; Escape exits
when no pending text or drawing gesture owns cancellation. The window's reported
fullscreen state updates the menu, and fullscreen is not persisted as a document
or user preference. macOS muda exposes both commands through the same action
outbox, with checked/enabled state revalidated on each pass. Tests verify viewport
commands and cancellation priority; native startup validates menu accelerator
construction. OS fullscreen transitions across supported targets still require
platform qualification. Automatic toolbar hiding and Thumbnail remain
unimplemented.

Checkable menu rows now reserve a left indicator column and draw a checkmark,
with normal hover styling instead of a permanently filled selection background.
Accessible labels, shortcuts and toggled state remain attached to the same button.
Classic v11 File/Edit/View menus use a 240-point minimum width and 30-point row
height; v7/v10 retain their existing density. This is presentation work guided by
the supplied View menu photo, not a claim of exact cross-DPI native geometry.

Classic's shared pixel-grid action is presented as **Gridlines** with Ctrl/Cmd+G.
Studio retains **Pixel grid** and does not inherit that accelerator. Action labels
and shortcuts resolve against the active frontend; macOS menu text and accelerator
registrations are reconciled on frontend changes, including removing Classic-only
bindings when entering Studio. Gridlines remain a per-document display option and
never alter saved pixels or history.

## Pixel rulers (T15)

View → Rulers and Ctrl/Cmd+R toggle fixed horizontal and vertical strips around
Classic's scrolling canvas. Their zero follows the canvas origin, including on
centered documents; labels and ticks use document pixels. Tick spacing adapts to
zoom/display density, uses whole-pixel minor divisions, and visits only visible
values. Cursor markers track the same canvas position. Inch/centimeter rulers and
reference-build parity are not claimed by this pixel-only implementation.

The optional settings-v3 `classic_rulers` boolean defaults to false for old files.
Visibility survives restart and remains separate from Studio. Plain and ruled
Classic views share a stable document scope so scroll position survives toggling.
A pending drawing gesture is cancelled before changing viewport geometry; ruler
interaction never publishes document pixels, history or save state.

Tests in `apps/paintcraft/tests/gui/rulers.rs` cover menu/shortcut routing,
1×/2× drawing coordinates, persistence, scroll preservation, ruler hit isolation
and cancellation. `crates/presentation/paintcraft-classic/src/ruler_axis.rs` checks fractional
zoom and 1×/1.5×/2× mapping, visible intervals and integer deep-zoom ticks. GPU
captures review both themes at 1× and 2×. These checks support this application
behavior, not historical Paint parity or platform input qualification.


## Palette and readability pass (2026-10-22)

Classic v11 now shows the reference arrangement of 20 named preset colors and ten
custom slots. Saved custom colors appear in the ribbon, with primary/secondary
selection on left/right click; an empty slot opens the working color editor.
The dialog retains the full saved custom palette. Color choices leave document
pixels/history unchanged. Earlier historical skins retain their fixed palette.

Classic v11 chrome uses larger Selawik body/button text, readable group captions,
larger color wells and an accent outline around the primary color. Main toolbar
interaction targets grow without expanding the compact shape cells. Ribbon
captions align and the shape gallery exposes its three rows. These metrics are
scoped to Classic; Studio and historical skins retain their presentation settings.

The visual-review helper exports light/dark workspace, View-menu and Settings
frames at 1440×900 with real egui GPU rendering. These are generated application
previews, not original-app reference captures or OS window/pen/IME qualification.
That pass checked the then-default 1200×850 desktop size; the current size is
recorded in the floating-controls pass below.

UI fidelity remains in progress: remaining dropdowns, native-style tool controls,
more exact spacing and unsupported editing operations retain their separate scope.

[UI validation and review frames](validation/classic-ui-2026-10-22/validation.json)
record 108 passing GUI tests (16 explicit ignores), strict all-target Clippy,
and six real-GPU light/dark scenes. Current [dark](validation/classic-ui-2026-10-22/classic-v11-dark-workspace.png)
and [light](validation/classic-ui-2026-10-22/classic-v11-light-workspace.png)
workspace previews are included for visual review.


## Floating tool controls (2026-10-22)

Classic v11 now places size in a slim vertical panel beside the canvas. Brush
adds a separate opacity panel. Circular handles, accent-colored rails, rounded
cards and direct numeric entry follow the supplied modern Paint direction. The
quick size slider covers 1–64 px; numeric entry retains the full 1–1024 px range.
Brush media choices remain in the existing gallery. Painting tools no longer
reserve the horizontal options row. Fill/selection and text retain their working
options; Studio and historical skins keep their existing presentations.

Control edits change tool preferences without modifying document pixels or
history. Each stroke still captures its options when it starts. Pointer presses
inside the panels are excluded from canvas painting, including the first layout
frame, and normal egui layer ownership remains checked. Disabled/modal contexts
keep the controls disabled.

A stable minimum inset across tools prevents brush/eraser switches from moving
the image. Small canvases still center in the full viewport when there is room.
Fit leaves enough space for the rail while retaining a document-centered view.
The requested default desktop size is now 1360×900, leaving room for the default
1000×486 document and controls. Panel height adapts to shorter viewports; GUI
checks cover 1100×500 and maximum numeric size without moving the canvas.

[Validation](validation/classic-tool-panels-2026-10-22/validation.json) records
110 passing GUI tests (16 explicit ignores), strict all-target Clippy, a desktop
build, eight real-GPU light/dark review scenes, and isolated 30-frame native macOS
light/dark startup checks. These do not establish pen/IME behavior, native latency
or complete Windows Paint reference parity.

Current brush views: [dark](validation/classic-tool-panels-2026-10-22/classic-v11-dark-brush.png)
and [light](validation/classic-tool-panels-2026-10-22/classic-v11-light-brush.png).

Classic ribbon follow-up (2026-10-22): new documents use `Untitled - PaintCraft`
in the native title, while bound documents use their filename. Layers now has
one bottom caption and a centered icon button. The Image group includes the
missing Remove background preview. The scrollable shape gallery adds polygons,
directional arrows, four/six-point stars and callouts as individual SVGs.
Unimplemented operations remain disabled; this does not complete shape semantics
or background removal. Compact grid columns keep Layers reachable at narrower
window sizes. 111 GUI tests, the title regression, vector rendering checks and
strict workspace Clippy pass. Light/dark review images and logs are in
[the ribbon validation record](validation/classic-ribbon-2026-10-22/validation.json).
