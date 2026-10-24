# PaintCraft Classic

Classic UI fidelity and themes are the current implementation priority. The additional Windows 11 Paint photos
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

The desktop starts at **1360 × 900 logical pixels**, providing more room for the
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

The text toolbar now follows `photos/mspaint-text-settings.png` with a 220-point
font selector, editable size field and scrollable size presets, larger style
buttons, outlined selected formatting/alignment controls and an explicit
Background fill checkbox. Tooltips describe the controls; renderer library names
are no longer displayed in the toolbar. Font search reports an empty result
without changing the chosen family. Preset/custom sizes and formatting persist
without creating document history. 113 GUI tests and strict workspace Clippy
pass; light/dark Selawik captures and source-pinned logs are in the
[text toolbar validation record](validation/classic-text-toolbar-2026-10-22/validation.json).
This presentation work does not qualify native text raster parity or complete
editable text objects and IME.

Classic v11 File/Edit now follow the supplied `im1age.png` and `imag1e.png`
structure: File exposes import/recent, Save as and Print submenus, Save as project,
sharing/background/property previews and Exit. Picture export uses the existing
PNG/BMP/JPEG path; project saving retains the native Save As action. Exit and the
macOS Quit event share the action outbox and existing unsaved-window close guard.
File scrolls in short windows; its height also overrides the popup's initial
estimate so lower rows remain clickable. Edit presents disabled Cut, Copy, Paste
and Copy visible layers previews above existing working PaintCraft commands.
Unavailable entries explain their state; this does not implement clipboard,
printing, acquisition, recent files, sharing, desktop background or image
properties. Historical Classic skins retain their existing menu layout.
116 GUI tests, six shared action tests and strict workspace Clippy pass;
[light/dark captures and source-pinned logs](validation/classic-file-edit-2026-10-22/validation.json)
record the scope and remaining reference differences.

Classic ribbon dropdowns now use compact icon triggers for shape outline/fill
and shorter, unframed Selection/Brush arrows. Selection and shape popup choices
use readable 15-point labels and 32-point rows; each trigger provides one tooltip.
117 window-free egui tests and strict workspace Clippy pass, including tooltip
and hit-target checks at display scales 1 and 2. The user has requested no testing
that opens application windows until explicitly permitted, so this pass has no
new rendered/native visual evidence. Source-pinned logs and limits are in the
[headless dropdown validation record](validation/classic-dropdowns-headless-2026-10-22/validation.json).

Settings now uses grouped theme/interface/skin, canvas and text sections, an icon
sidebar on wide windows and wrapped navigation tabs on narrow ones. Page content
scrolls independently from the Apply/Cancel footer. Theme preview, draft
cancellation and separate-workspace switching retain their existing semantics.
Window-free tests cover 1360x900, 700x500, 480x500 and 480x360 at display scales
1 and 2, including stable controls across repeated frames and page changes.
Canvas defaults apply to new documents without resizing the open image.
119 egui tests and strict workspace Clippy pass; the
[headless settings record](validation/responsive-settings-headless-2026-10-22/validation.json)
contains source-pinned logs. Native/rendered visual review remains intentionally
deferred under the user's no-window-testing instruction.

The View menu now implements mutually exclusive **Always show toolbar** and
**Automatically hide toolbar** choices. The default remains always shown; the
auto-hide preference persists and older settings default to false. Auto-hide
reclaims the ribbon/options layout space and reveals it on menu/toolbar hover.
Pointer down/release frames, open popups and focused text input hold the layout;
the visibility decision is fixed across all layout passes in a frame so release
cannot move a gesture's canvas mapping. Font/options rows share the hover region.
Studio retains its own toolbar presentation. View now scrolls on short displays,
while Thumbnail is an explicitly disabled preview. The supplied View screenshot
establishes the choices, but pinned Windows runtime/visual parity is unqualified.
124 window-free GUI tests, preference/action checks and strict Clippy pass; see
[the toolbar visibility record](validation/toolbar-visibility-headless-2026-10-22/validation.json).
No application windows or rendered/native previews were launched.

Edit colors now adapts its width, picker/brightness dimensions and palette columns
to the window. Wide layouts keep the picker and numeric fields side by side;
narrow layouts stack them inside a scrollable body. OK/Cancel remain outside that
body and stay fixed while editing or scrolling. Numeric values precede their
channel labels, with larger inputs, an accented OK button, a bordered color
preview, stronger selected-slot outlines and dashed empty custom slots. The dialog
presentation lives separately in `colors/dialog.rs`; color conversion policy is
unchanged. Tests cover 1360x900 through 360x500/480x360 and display scales 1 and 2.
126 window-free UI tests, two color-conversion tests and strict workspace Clippy
pass; [source-pinned headless results](validation/responsive-colors-headless-2026-10-22/validation.json)
record acceptance/cancellation and palette persistence without document mutations.
Rendered/native visual comparison remains deferred until the user permits it.

Classic now owns a scrollable Layers list with larger selectable rows, icon actions
for creation/reordering/deletion and a close button. Name/opacity fields remain
visible below the list. Stable per-layer widget identities survive reordering,
and selecting another layer commits a focused name edit to its original owner.
The desktop host still applies document mutations through existing engine actions;
selection and panel visibility do not add history. Host layer/property integration
was extracted from the general chrome module. Studio keeps its own presentation.
128 window-free GUI tests and strict workspace Clippy pass; the
[headless Layers record](validation/classic-layers-headless-2026-10-22/validation.json)
covers light/dark layouts, display scales 1 and 2, long-list scrolling and repeated
layout passes. Real layer thumbnails and complete reference/native visual parity
remain open. No application windows or screenshot previews were launched.

Confirmation prompts now share bounded, theme-aware presentation with larger
buttons and an accented primary choice. Narrow windows stack actions; long
explanations scroll without moving the action area. Save/discard workspace
switching, document closing and ordinary discard/replace/export/recovery prompts
use the same widget. The host retains all side effects and gates choices on the
first layout pass. Escape/backdrop consistently cancel ordinary confirmations
and document-close prompts as well as interface switches.
131 window-free GUI tests and strict workspace Clippy pass; the
[confirmation record](validation/confirmations-headless-2026-10-22/validation.json)
covers long messages/paths, two/three-choice layouts down to 360x360, light/dark
themes, scales 1 and 2, once-only choices and pixel/history-preserving cancellation.
Native visual fidelity remains unqualified; no application windows were opened.

Canvas-size dialogs now use grouped dimension fields, larger inputs and a fixed
accented Apply/Cancel footer. The body scrolls independently, including the
remembered/configured startup-size choices. Classic owns the presentation through
view data; the desktop host retains tab/revision validation, typed resize actions
and preference persistence. Existing top-left anchoring and preservation of
off-canvas content remain unchanged.
132 window-free GUI tests, the stale-revision unit check and strict workspace
Clippy pass; the [canvas-size record](validation/canvas-size-headless-2026-10-22/validation.json)
covers both dialogs down to 360x500/480x360 in light/dark themes and scales 1 and 2.
Staged edits/cancellation preserve exact preference bytes and document history;
Apply produces one undoable resize and updates the remembered size. Native visual
comparison remains deferred; no application windows or screenshot previews ran.

Rotate/Flip popups now use frameless ribbon triggers, 40-point rows, 22-point
images, 15-point text and screen-bounded scrolling. The half-turn and custom-angle
entries use distinct original SVGs rather than reusing quarter-turn arrows. Their
source-sheet coordinates are null; all 86 registered assets pass vector,
transparency, cache and density checks, and regeneration is byte-identical.
Transform execution remains disabled. A separate headless geometry check confirms
the existing Layers icon draws at 52 points in both open/closed states and display
scales 1 and 2; the suspected font-height cap did not affect that path.
134 window-free UI tests, three icon tests and strict workspace Clippy pass; see
[the image-menu record](validation/image-menus-headless-2026-10-22/validation.json).
The supplied Rotate/Flip photos guide layout; native/rendered parity remains
unqualified and no application windows or screenshot previews were launched.

Following the requested size adjustment, Layers now draws a 44-point icon in a
60-point button. It stays frameless while closed and idle, shows a hover surface,
and retains its selected surface while the panel is open. The single bottom
caption remains. Classic v11 adds 12 points of spacing between File/Edit/View.
Updated headless checks cover actual icon geometry and hover frames at scales 1
and 2, menu spacing, and default-window ribbon visibility. All 134 window-free UI
tests and strict workspace Clippy pass; no windows or rendered previews ran.

Classic v11 now follows the supplied main-window reference's Save, Share, Undo and
Redo quick-action order. The requested Clear Canvas button remains as a PaintCraft
extension. New/Open and picture export use their existing File-menu entries;
command identities, shortcuts and file outcomes are unchanged. Share is an
explicitly disabled preview with one explanatory tooltip, using its own original
SVG in both the bar and File menu. Classic v7/v10 and Studio retain their bars.
135 window-free UI tests, three icon checks and strict workspace Clippy pass; see
[the quick-access record](validation/quick-access-headless-2026-10-22/validation.json).
Import/export/New tests now exercise the File menu. All 87 icons pass vector and
transparency checks and generated bytes are reproducible. No application windows
or screenshot previews ran; native/rendered parity remains unqualified.

Classic's color controls now live together in the palette module. The reference's
larger primary/smaller secondary treatment uses centered 40/32-point wells; each
well's single tooltip includes its role, existing palette name and RGB hex value.
Arbitrary RGB values use a Custom color label. Edit colors uses a dedicated
original color-wheel-plus SVG, distinct from the color-balance adjustment icon.
Existing color values, alpha handling and staged acceptance remain unchanged.
136 window-free UI tests, three icon checks and strict workspace Clippy pass; see
[the color-well record](validation/color-wells-headless-2026-10-22/validation.json).
Primary/secondary choices and custom RGB tooltips are checked at scales 1 and 2
without document/history changes. All 88 assets pass vector/transparency checks
and regeneration is byte-identical. Native names, processing and rendered visual
parity remain unqualified; no application windows or screenshot previews ran.

Classic v11 now owns its status presentation: stable cursor-position, selection-size
and canvas-size fields with SVG indicators, a bounded status-message area and
right-aligned zoom controls. Narrow windows put zoom on a second row. The host
reserves the space before canvas layout and draws status contents afterward,
showing current-frame coordinates without an extra repaint request. Messages
and recovery-checkpoint details remain available. Real selection dimensions use
the editor's half-open bounds; view changes add no document history. Zoom input
and icons now share a vertically aligned control region. Studio and Classic
v7/v10 retain their existing status layouts.
138 window-free UI tests, three icon checks and strict workspace Clippy pass; see
[the Classic status record](validation/classic-status-headless-2026-10-22/validation.json).
Tests cover 1360x900 through 360x360, light/dark themes and scales 1 and 2, including
stationary zoom controls and current-frame pointer feedback. All 89 assets pass
vector/transparency checks. Native/rendered parity and latency remain unqualified;
no application windows or screenshot previews were launched.

The text family popup now keeps a focused search field above an independently
scrollable family list. Only visible rows are created; filtering resets list
position, long names stay within the selector width, and selecting a family
explicitly closes the popup. Search clicks keep it open. Font and point-size
presentation live in separate modules; size popups cap their height relative to
the viewport, and text-style buttons now provide inactive hover feedback.
Popup ownership also fixes a discovered input issue: plain text and Escape in
open popups no longer alter or discard the pending canvas text.
140 window-free UI tests and strict workspace Clippy pass; see
[the font-control record](validation/font-controls-headless-2026-10-22/validation.json).
A synthetic 1,000-family catalog checks scrolling/filtering/selection at 480x500
and 360x360, scales 1 and 2. Actual application tests cover draft preservation in
font, size and File popups. Font discovery, rasterization and publication policy
are unchanged. Search interaction is PaintCraft policy, not native-parity or
measured-latency evidence; no application windows or screenshot previews ran.

The Image resize icon no longer opens the canvas-bounds dialog: changing bounds
is not image resampling. Resize and skew is an explicitly disabled preview until
its own image operation exists; View → Canvas size retains the real undoable
bounds operation. Crop, Resize and skew, Remove background and Color picker now
each provide one disabled-hover explanation. The resize explanation points to the
separate View command for users who want to change canvas bounds.
All 140 window-free UI tests and strict workspace Clippy pass; see
[the image-availability record](validation/image-availability-headless-2026-10-22/validation.json).
Checks cover disabled clicks opening no alternate dialog, one tooltip at scales
1 and 2, ribbon visibility, and the existing canvas-size persistence/undo path.
No image transform is claimed and no native windows or screenshot previews ran.

The Tools group now presents an explicit Magnifier preview rather than using
fit-to-window as a substitute for the missing tool. It stays disabled and points
to working status-bar/View zoom controls. Existing fit workflows now exercise
the status-bar action; disabled previews preserve the canvas view as well as
document pixels. Brush-gallery rows use inactive frameless styling, hover
feedback and a selected-tip outline. Round/Square behavior and unavailable media
remain unchanged; the samples remain original UI illustrations.
All 140 window-free UI tests and strict workspace Clippy pass; see
[the Tools/gallery record](validation/tools-gallery-headless-2026-10-22/validation.json).
The checks include single availability tooltips at scales 1 and 2, selected-tip
metadata, real fit/pixel mapping and document/view preservation. No magnifier or
additional media implementation is claimed. No native windows or rendered
previews ran; reference/native parity remains unqualified.

The Shapes gallery now follows the supplied 23-item reference ordering: the
fourth row ends with rectangular and oval callouts, followed by Cloud callout,
Heart and Lightning. Internal asset names are replaced by readable labels.
Working shapes have consistent hover feedback and a selected outline; disabled
shapes explain availability in one tooltip. Gallery presentation now lives in
its own module. Shared extra shape assets remain available to other frontends.
All 141 window-free UI tests and strict workspace Clippy pass; see
[the shape-gallery record](validation/shapes-headless-2026-10-22/validation.json).
Checks cover final-row geometry and tool selection at scales 1 and 2, with no
document/history changes. Only Line, Ellipse and Rectangle are functional; the
other twenty remain previews. Native/rendered parity remains unqualified and no
application windows or screenshot previews ran.

Selection, Brushes, shape outline/fill and Rotate/Flip triggers now show hover
feedback while staying frameless at rest. An accent outline remains while a popup
is open, with matching accessible state. Arrow hit targets keep their compact
height; shape paint choices now have 220x40-point rows and selected outlines.
Existing selection, fill/outline and brush behavior is unchanged.
All 143 window-free UI tests, the document-title unit test and strict workspace
Clippy pass; see [the dropdown-state record](validation/dropdown-states-headless-2026-10-22/validation.json).
The new tests inspect painted frames and Escape dismissal in light/dark at scales
1/2, with repeated layout passes and document/history preservation.

Desktop startup now leaves the initial viewport title at eframe's application-name
default, PaintCraft. eframe 0.37.0 captures that launch title for its macOS app-menu
name. The existing first UI pass separately publishes Untitled - PaintCraft or
the bound filename as the window title. This avoids capturing a document title
as the app-menu name and requires an app restart. Native appearance is unverified
under the no-window testing restriction; no application windows were launched.

The Selection popup now uses a dedicated presentation module with aligned
40-point rows, 22-point icons, right-aligned shortcut hints and an accent marker
for the selected rectangular mode. Reference choices keep their ordering; the
working Deselect extension follows them. A height-bounded scroll area keeps lower
actions reachable on short windows. Free-form, invert, transparent selection and
selected-pixel deletion each remain disabled with one availability explanation.
All 144 window-free UI tests and strict workspace Clippy pass; see
[the Selection-menu record](validation/selection-menu-headless-2026-10-22/validation.json).
Checks cover light/dark at scales 1/2, down to 360x360, with repeated layout passes.
Working Select all/Deselect publish once each; preview clicks preserve document,
tool and history. No selection kernel or native/rendered parity is claimed, and
no application windows or screenshot previews were launched.

The status zoom popup now uses 40-point rows and 15-point labels in a narrow
scrollable list, following the supplied upper/lower preset references. Viewport
height bounds the popup, and the active preset has an accent outline and matching
accessible selected state. Percentages retain up to two fractional digits while
omitting trailing zeros: 73%, 1000% and 12.5% display consistently after typing
or selecting a preset. The fourteen presets and shared zoom actions are unchanged.
All 145 window-free UI tests and strict workspace Clippy pass; see
[the zoom-popup record](validation/zoom-popup-headless-2026-10-22/validation.json).
Checks cover both list endpoints, selected outlines, typed values, fit/shortcuts
and safe Escape dismissal, with document/history preservation, light/dark at
scales 1/2 and windows down to 360x360. Native/rendered parity remains unverified;
no application windows or screenshot previews were launched.

The Classic skin chooser now lives in the Classic presentation crate. It uses
three cards on wide windows and stacked cards in a bounded scroll area on narrow
ones, above fixed Continue/Apply and Cancel actions. Original schematics follow
the active theme and selected cards have accent outlines and radio metadata.
Choices stay staged until acceptance; Cancel, window close and Escape dismiss
them. First-visit cancellation leaves the default active for this run and prompts
again next time. Changer Apply and first-visit Continue retain host persistence.
Plain text and Escape in the chooser now preserve unpublished canvas text.
All 147 window-free UI tests and strict workspace Clippy pass; see
[the skin-chooser record](validation/skin-picker-headless-2026-10-22/validation.json).
Checks cover bounds, scrolling, stable footers, cancellation bytes, acceptance and
restart at scales 1/2 in both themes down to 360x360. The chooser remains modeless;
its schematics are not native captures. Window-close dismissal is implemented but
not separately tested. No application windows or screenshot previews were launched.

Classic canvas handle previews now cover all eight edge/corner positions with
white outlined markers outside the image. Their hover targets touch the edge
without consuming ordinary first-pixel painting. Each is accessibly disabled,
shows a not-allowed cursor and provides one explanation directing users to the
working View → Canvas size command. They remain previews, not drag-resize controls.
The overlay uses a nonallocating child UI so it does not shift canvas layout;
Studio keeps its independent canvas presentation.
All 148 window-free UI tests and strict workspace Clippy pass; see
[the canvas-handle record](validation/canvas-handles-headless-2026-10-22/validation.json).
Checks inspect all marker positions, size/color, hover/click safety and stationary
layout in both themes at scales 1/2. First-pixel painting still publishes once.
Native/rendered parity remains unverified and no application windows or screenshot
previews were launched.

Classic v11 now moves quick actions to a second row when available chrome width
is below 600 points. File/Edit/View and the adjacent interface changer/Settings
stay on the first row. This fixes Clear Canvas overlapping the changer at 480
points and being clipped at 360. Both rows stay visible with an automatically
hidden ribbon and participate in hover reveal; wide and historical-skin layouts
retain their existing code path.
All 149 window-free UI tests and strict workspace Clippy pass; see
[the compact-chrome record](validation/compact-chrome-headless-2026-10-22/validation.json).
Checks cover on-screen, nonoverlapping actions, menu spacing, actual Settings and
skin-chooser clicks/cancellation, stationary positions and narrow ribbon
hide/reveal, at scales 1/2 and both themes down to 360x500. Document/history remain
unchanged. The responsive row is PaintCraft presentation policy, not native-parity
evidence; no application windows or rendered previews were launched.

Classic's Drawing on label now stays on one truncated line, keeping property
editors fixed when a layer has a long name. It exposes the full name in one
explicit tooltip; egui's automatic elision tooltip is disabled there to avoid
duplication. Layer rows now use inactive frameless styling, hover feedback and a
selected accent outline, with their own full-name tooltip. Layer identity and
committed rename/opacity behavior are unchanged.
All 150 window-free UI tests and strict workspace Clippy pass; see
[the layer-name record](validation/layer-names-headless-2026-10-22/validation.json).
A real 900-character rename checks stationary name/opacity fields, one-line
status, selected metadata/painted outlines and single tooltips in light/dark at
scales 1/2. Subsequent hovering preserves document/history, and existing
visibility/order/rename-owner/undo checks pass. Thumbnails remain open work; no
native windows or rendered previews were launched.

Classic status labels now disable automatic elision tooltips while retaining
their explicit hints. This prevents truncated pointer/selection/canvas values
from adding a second raw-value tooltip beside the full named hint. Long status
messages similarly retain one complete message/recovery tooltip.
All 151 window-free UI tests and strict workspace Clippy pass; see
[the status-tooltip record](validation/status-tooltips-headless-2026-10-22/validation.json).
The new narrow-layout probe reproduced the duplicate before the fix and passes
afterward in light/dark at scales 1/2, including long messages and large numeric
values. Hover emits no zoom action and leaves controls stationary. Synthetic
facts qualify presentation; existing app status/zoom checks retain their own
document/history evidence. Native/rendered parity remains unverified and no
application windows or screenshot previews were launched.

Edit colors now uses 40-point RGB/HSL dropdown rows with inactive hover styling
and selected accent outlines. Popup height is bounded and choosing a model
explicitly closes it. Numeric conversions and color-adoption policy are unchanged.
All 152 window-free UI tests and strict workspace Clippy pass; see
[the color-model record](validation/color-model-headless-2026-10-22/validation.json).
Actual-dialog checks cover both themes and scales 1/2 down to 360x360, including
on-screen rows, selected metadata, nested Escape dismissal, stable footer
placement and exact staged hex preservation across model switches. Cancel keeps
active color, raw preferences and document/history unchanged. The reference shows
the closed control; open-menu/native color parity remains unqualified. No
application windows or rendered previews were launched.

V11 File/Edit/View menus now use 40-point rows and 22-point icons, including
the reference Zoom entry. Shared command rendering accepts frontend icon metrics
while retaining labels, shortcuts, enabled gates and checkmarks; its default size
remains unchanged for other callers. Edit now scrolls to keep lower commands
reachable on short displays. Classic submenu anchors are clipped to visible
entry bounds, preventing a scrolled-off Zoom entry from opening under a stationary
pointer. Escape/Left dismiss a child without closing its parent, with deliberate
keyboard/click or pointer movement required before hover reopens it.
All 153 window-free UI tests, six shared-action tests and strict workspace Clippy
pass; see [the menu-row record](validation/menu-rows-headless-2026-10-22/validation.json).
Checks inspect actual icon shapes/row sizes, lower-command reachability, submenu
bounds and keyboard navigation in light/dark at scales 1/2 down to 360x360. The
narrow auto-hide regression also verifies activation after scrolling. Document
semantics are unchanged; clipboard/platform previews remain disabled. Native
AppKit/rendered parity remains unverified and no application windows or rendered
previews were launched.

Classic floating controls now use dedicated Stroke Size and Opacity SVG symbols
instead of the paintbrush and blur-effect icons. Size headers identify the active
tool through accessibility labels and a single tooltip; opacity has its own
labelled header. These are original static UI illustrations, not stroke previews.
The generator, manifest/gallery and asset registry now cover 91 SVGs in total
(89 generated plus the two separate chrome assets).
All 154 window-free UI tests, three CPU icon tests and strict workspace Clippy
pass; see [the tool-symbol record](validation/tool-symbols-headless-2026-10-22/validation.json).
Regeneration is byte-identical. Checks cover all six size-controlled tools plus
brush opacity, 22-point headers and single tooltips in both themes/scales 1/2.
Header clicks add no document/history/gesture changes; option editing still works
and canvas layout stays fixed. Brush kernels and media coverage are unchanged.
No application windows or rendered previews were launched.

Escape now discards an uncommitted typed zoom percentage instead of applying it
through the field's focus-loss path. The editor captures Escape while focused,
restores the actual formatted percentage and releases focus. This also keeps the
same Escape from cancelling pending canvas text. Enter/Tab commits and existing
zoom bounds/action routing are retained.
All 155 window-free UI tests and strict workspace Clippy pass; see
[the zoom-cancellation record](validation/zoom-cancel-headless-2026-10-22/validation.json).
The added regression failed before the fix and now verifies valid/invalid draft
cancellation, current zoom restoration and pending text/document/history
preservation at scales 1/2 with repeated layout. Tab still applies a valid draft
without a document edit. This qualifies normal focused-field Escape, not arbitrary
native coalescing or IME. No application windows or rendered previews were launched.

Escape now discards a staged layer-name edit instead of committing it through
focus loss. The shared Classic/Studio field captures Escape, restores the current
committed name and releases focus, preserving pending canvas text and publishing
nothing. Normal Enter/focus-loss acceptance retains its existing layer-owner
routing and pending-text resolution policy.
All 156 window-free UI tests and strict workspace Clippy pass; see
[the rename-cancellation record](validation/rename-cancel-headless-2026-10-22/validation.json).
The cancellation regression failed before the fix and now passes in both
frontends, both themes and scales 1/2 with repeated layout. A second Escape can
then cancel the canvas preview without publication; an ordinary accepted rename
still publishes once. Existing old-owner rename/undo checks pass. Native event
coalescing, IME and rendered parity remain unqualified; no application windows or
rendered previews were launched.

Escape now discards typed layer-opacity drafts in Classic and Studio without
publishing or cancelling pending canvas text. Numeric edits remain staged until
acceptance; cancellation persists through repeated layout so a widget's private
text buffer cannot publish the discarded value on a later pass. Pointer drags
retain one publication on release.
All 157 window-free UI tests and strict workspace Clippy pass; see
[the opacity-cancellation record](validation/opacity-cancel-headless-2026-10-22/validation.json).
The cancellation prefix failed before the fix. The regression covers both
frontends, light/dark at scales 1/2, pending text/history preservation, accepted
Enter, Undo and release-only pointer publication. Native event coalescing, IME
and rendered parity remain unqualified; no windows or rendered previews were launched.

The text toolbar now follows the supplied text-settings reference with 40-point
controls, a wider family selector and size field, and matching outlined selected
rows in both font-family and point-size popups. Size choices retain bounded
scrolling, custom values and close-on-selection behavior; font search remains
fixed above the virtualized family list.
All 158 window-free UI tests and strict workspace Clippy pass; see
[the text-dropdown record](validation/text-dropdowns-headless-2026-10-22/validation.json).
The added metric regression fails on the previous 36-point control. Current checks
cover selected-size paint, short-window last-preset reachability, acceptance and
Escape, plus the existing large-catalog/search regression in repeated layout.
Native appearance, full reference fidelity and text rasterization parity remain
unqualified; no windows or rendered previews were launched.

Typed font sizes now remain local until acceptance. Escape restores the saved
size without cancelling unpublished canvas text or creating document history;
frame-wide cancellation prevents the numeric widget's private buffer from being
applied during repeated layout. Enter still applies the preference.
All 160 window-free UI tests and strict workspace Clippy pass; see
[the text-input record](validation/text-input-headless-2026-10-22/validation.json).
The size regression fails before the fix because typing prematurely saves the
preference. A separate narrow-toolbar regression verifies existing horizontal
wheel scrolling to Background fill, isolated toggling and scrolling back to the
font selector. Native coalescing, IME and visual/rasterization parity remain
unqualified; no windows or rendered previews were launched.

Classic layer rows now show real committed-content previews over a transparency
checker, positioned at the leading edge without moving name/opacity fields.
A single CPU worker uses the display compositor's box filtering on an immutable
selected-layer snapshot, with at most 64x64 output samples and aspect-correct
32-point presentation. Hidden/zero-opacity layers keep identifiable previews.
Only visible images are retained (up to 64), source changes reject stale results,
and panel close/interface switch cancel work and evict images. Shared admission
retains declared working/source charges through completion and image charges
through eviction; this does not qualify physical GPU/RSS usage or native latency.
All 161 window-free UI tests, four thumbnail-renderer tests, two worker tests,
five existing display-compositor regressions and strict workspace Clippy pass;
see [the layer-thumbnail record](validation/layer-thumbnails-headless-2026-10-22/validation.json).
The UI regression observes real image meshes through paint/Undo while preserving
property geometry and document/history ownership. Pixel checks cover clipping,
source alpha and partial edges; worker checks cover stale completions, portrait
aspect changes and reservation release. Studio/document-tab previews, the
separate View Thumbnail window, live tool previews and native visual parity
remain open. No windows, GPU tests or rendered previews were launched.

Windowed and GPU visual review is now authorized by the user (2026-10-22).
The first fresh scene comparison found filled blue shapes where the supplied
Paint references use monochrome outlines. Classic's 23 gallery entries now use
original individual outline SVG variants, tinted for the theme; colored sheet
icons keep their existing sources. Variant-aware cache keys prevent aliasing.
All 161 window-free UI tests, five icon tests, a wgpu-backed theme scene export
and strict workspace Clippy pass. The generator reproduces all 23 outline
sources and manifest exactly. See [the outline review record](validation/classic-outlines-gpu-2026-10-22/validation.json)
for raw light/dark before/after scenes. The isolated desktop launch exited
successfully, but its OS window capture failed; native menu text and desktop
appearance remain visually unqualified. Existing ignored window/GPU checks may
now be run selectively. Full reference fidelity remains partial, including
caption/text contrast/scale, other icons and native interaction checks. The scene
exports also expose a clipped canvas-edge gradient requiring investigation.

The clipped canvas-edge gradient is fixed. egui's default scroll fade was painted
after the document and tinted image pixels. Plain/ruled Classic and Studio now
disable that effect within their own canvas presentation scopes, preserving
ordinary scroll cues in caller UI and menus. Studio's scope is document-stable;
Classic retains its shared plain/ruled scope and navigation identity.
All 162 window-free UI tests and strict workspace Clippy pass; see
[the canvas fade record](validation/canvas-fades-gpu-2026-10-22/validation.json).
Pre-fix headless and GPU probes both fail. The current GPU regression verifies
60 white framebuffer samples across centers/edges before and after actual
two-axis panning in all three presentations, light at scale 1 and dark at scale 2.
Fresh whole-application light/dark scenes confirm the visible strip is gone.
Caller scroll policy is unchanged. These checks qualify the fade fix, not every
image color, native menus/driver paths, pen/IME or full Windows visual parity.

Classic v11 ribbon groups now use larger 14-point captions with readable theme
shades. The Select and Brush controls use centered 32-point SVG images in
64x60 icon-only buttons, retaining accessible names, hover tooltips and selected
frames. Their names are no longer painted inside the button as well as below
the group. Shared widgets, Studio and historical skins retain their presentation.
All 163 window-free UI tests, a fresh wgpu-backed theme scene export and strict
workspace Clippy pass; see [the ribbon caption record](validation/ribbon-captions-gpu-2026-10-22/validation.json)
for raw light/dark before/after scenes. The caption-size probe fails before the
change; current checks cover all seven captions, image size/centering, duplicate
text removal, tool activation and document/history preservation at scales 1/2
with repeated layout. Full Windows parity and native menu/desktop appearance
remain unqualified; other chrome and feature work remains on the roadmap.

Classic v11 File/Edit/View menus now use contiguous 40-point rows, removing the
five-point dead gap observed between adjacent entries. Body text is 16 points;
primary/shortcut colors remain distinct and readable in each theme. Separators,
shared action metadata/gates, submenu behavior and existing extensions are retained.
All 164 window-free UI tests, a fresh wgpu-backed menu scene export and strict
workspace Clippy pass; see [the menu density record](validation/menu-density-gpu-2026-10-22/validation.json)
for raw light/dark File/Edit before/after captures. The old geometry probe fails
on the extra gap. Current checks also click within that former gap to activate
Gridlines, confirm its checked state and preserve document/history in repeated
layout at scales 1/2. Menu icon styling is a separate remaining fidelity task;
full Windows parity, native menu appearance and unavailable operations remain open.

Classic v11 main menus now opt into original monochrome outline SVG glyphs,
including disabled previews, submenu anchors and Zoom children. The shared
menu helper keeps its colorful default; shared action metadata, gates, shortcuts
and checks retain their existing path. Colored/chrome sources and shape variants
keep separate texture identities in the bounded cache. The 23 new menu sources
and manifest are reproduced by `scripts/generate_classic_menu_icons.py`.
All 164 window-free UI tests, six icon tests, two wgpu-backed menu capture tests
and strict workspace Clippy pass; see [the outline-menu record](validation/menu-outlines-gpu-2026-10-22/validation.json).
Raw light/dark File/Edit comparisons and View/Zoom scenes are archived. Source
checks cover transparency and white tintable output at multiple densities, while
cache checks prevent colored/outline aliasing. The Save as pencil has a local
mask to separate underlying disk strokes. Full reference parity remains partial:
import/recent/property glyph identities, thumbnail/fullscreen marker presentation,
native menus and unavailable operations remain open.

Classic v11 View checkboxes now reserve the same fixed icon column as Zoom.
Full screen retains its outline arrows while unchecked and shows a check while
selected. A regression compares actual visible glyph positions before/after
toggling, verifies the unchecked image and selected accessibility state, and
checks the viewport command without document/history mutation. All 165 normal
GUI tests and strict workspace Clippy pass. Fresh light/dark GPU View captures
are archived in [the marker record](validation/view-markers-gpu-2026-10-22/validation.json).
Thumbnail and extension-row presentation, native appearance and full reference
parity remain open.

Classic menu previews now have dedicated Import image, Recent, Desktop background,
Image properties and Thumbnail SVGs, retaining all prior artwork. The registry
contains 96 IDs and 28 menu sources; the menu generator/manifest owns the additions.
Recent and Desktop background show disabled outline submenu chevrons. Active
anchors share the outline chevron, and no-icon View extension rows reserve the
label column. All 166 normal GUI tests, six icon tests, both GPU menu capture tests
and strict workspace Clippy pass. Distinct real-row textures, disabled marker/input
behavior and extension-label alignment are verified; fresh raw light/dark File/View
comparisons are in [the menu identity record](validation/menu-identities-gpu-2026-10-22/validation.json).
Unavailable operations remain disabled. Exact glyph geometry, native appearance
and full reference parity remain incomplete.

Classic v11 top bars now use 16-point labels, wider menu padding and 40-point
controls. Short windows below 420 points high retain 32-point targets so the
full ribbon and real canvas input remain usable. Windows below 720 points wide
keep quick actions in a second row alongside stable Settings/changer placement.
Save/Share/Undo/Redo/Clear Canvas use theme-tinted outline menu artwork through
optional shared helpers, retaining action gates, accessibility, single tooltips
and hover surfaces. Default colored widgets and historical skin metrics retain
their prior path. Six geometry/theme/DPI cases verify these controls and
canvas availability; existing real canvas-click and cursor-status regressions
pass. All 166 normal GUI tests (19 GPU/export tests ignored) and strict workspace
Clippy pass. Twelve fresh GPU scenes cover both themes at wide, compact and
short sizes, including hover: [the top-bar record](validation/classic-topbar-gpu-2026-10-22/validation.json).
An intermediate short-window canvas regression is documented with its failing
logs and corrected in the final source. Full Windows/native parity remains open.

Classic's Image group now follows the two-row reference grouping, with Crop/
Rotate above Remove background/Flip and a centered larger Resize preview
spanning both rows. Nine original glyphs have paired light/dark SVGs that retain
blue accents, separate from colored/menu sources in the bounded cache. The
Image generator and manifest own these 18 sources. Rotate/Flip dropdowns now use
the same artwork, contiguous 40-point rows, 16-point labels and bounded 240-point
bodies. Disabled operations and single availability tooltips remain explicit.
Repeated-layout geometry/state checks and existing short-window popup tests
verify actual glyph centering, width, scrolling and safe dismissal. All 167
normal GUI tests, seven icon tests, two GPU review exports and strict workspace
Clippy pass. Raw before/after wide workspaces, compact/short layouts and both
transform popups are in [the Image ribbon record](validation/classic-image-ribbon-gpu-2026-10-22/validation.json).
Transform semantics, exact native glyph geometry and full reference parity remain
open. No document or history behavior was added in this presentation slice.

Native macOS naming is now independently checked through Accessibility menu
titles and CoreGraphics window metadata. The unbundled executable previously
showed lowercase `paintcraft` in the application menu. A desktop-only build
script now embeds versioned application metadata with `CFBundleName` and
`CFBundleDisplayName` set to `PaintCraft`; document/window titles stay independent.
A fresh ordinary native session reports application menu `PaintCraft` and window
`Untitled - PaintCraft`. Native Quit routes successfully and exits zero. The
embedded plist is decoded independently with `otool`/`plistlib`; the existing
document-bound title regression and strict workspace Clippy pass. The metadata
adds no runtime dependencies or unsafe code. [Apple's naming guidance](https://developer.apple.com/library/archive/qa/qa1544/_index.html)
identifies `CFBundleName` as the canonical application name used by the main menu.
See [the native naming record](validation/macos-app-identity-2026-10-22/validation.json).
Native appearance remains unqualified: the successful window capture contains a
blank client, activation returned false, and raising the review window failed.
The raw image is archived explicitly as unqualified evidence; native surface
presentation/capture needs further investigation. GPU client-scene exports remain
separate evidence. The full UI/reference roadmap remains incomplete.

Native presentation was rechecked on 2026-10-23. Fresh ordinary desktop launches
now produce complete light/dark native window captures with ribbon, tool panel,
canvas and status UI. Both report regular activation policy and active state,
application menu `PaintCraft`, window `Untitled - PaintCraft`, and successful
native Quit with exit zero. These production runs use no forced readback or
frame-count shortcut. The activation-policy hypothesis was disproved; no runtime
startup/renderer change was needed. The exact trigger of the prior inactive
blank captures is not established, and they remain archived as historical evidence.
An explicit [native capture example](../apps/paintcraft/examples/README.md) now
uses the real App/native services and requests a GPU framebuffer even if occluded.
Light/dark outputs are verified; missing arguments and write failures return
nonzero exits. Strict workspace Clippy, formatting and source-length checks pass.
See [the native presentation record](validation/native-presentation-2026-10-23/validation.json)
for raw native/framebuffer captures, activation/name reports and scope limits.
This qualifies a native presentation baseline on this Mac; full Windows parity,
all native menu interactions and physical input/text-device qualification remain open.

Classic's Shape outline/fill triggers now use original monochrome pen-over-box
and bucket menu variants at an explicit 22-point size. Colored tool sources
keep their existing artwork. The two working choices in each popup have bounded
220-point bodies, contiguous 40-point rows and 16-point text, preserving selected
outlines and hover/open surfaces. Repeated-layout tests in light 1x/dark 2x and
a 360x360 dark layout verify glyphs, bounds, aligned labels, selected states,
Escape and Both/Filled/Outline option transitions without document/history edits.
The narrow case reveals the outer ribbon before selecting Rectangle; the existing
filled-shape test still checks actual published pixels. All 167 normal GUI tests,
seven icon tests, the expanded GPU scene export and strict workspace Clippy pass.
Eight raw light/dark popup states are archived in [the shape paint record](validation/shape-paint-gpu-2026-10-23/validation.json).
The menu generator now reproduces all 30 menu/control sources. Textured styles,
additional shapes and full native/reference parity remain incomplete.

The Classic brush gallery now follows the supplied close-up's proportions with
a 320-point body, 56-point rows, 16-point labels and inset stroke illustrations.
The popup stays bounded and scrolls to the Square brush extension below the nine
reference rows. Oil artwork has distinct strands; broad/tapered bands use shared
mesh vertices and feathered edges to remove per-segment seams. Brush ends are
round. These are UI illustrations, not implemented media or document previews.
Classic v11 primary Brush/Select controls now use original monochrome variants
while retaining centered metrics, selected state, accessible names and actions.
Colored tool sources remain available to other presentations. The gallery is
shared Classic chrome; historical skin fidelity remains unqualified.
All 168 normal GUI tests, seven icon tests, the expanded GPU gallery export and
strict workspace Clippy pass. Wide/narrow repeated-layout cases verify every row,
label/sample clearance, disabled media/one tooltip, reachable working tips and
unchanged document/history. Raw light/dark comparisons and scrolled/selected tip
states are in [the brush gallery record](validation/brush-gallery-gpu-2026-10-23/validation.json).
Unavailable media, full reference parity and the remaining roadmap are still open.

The Classic Selection popup now has a bounded 260-point body, 16-point labels/
shortcuts and consistent primary/secondary contrast. Rectangle, Free-form,
Select all, Invert and Delete have distinct monochrome SVG artwork; unavailable
rows retain disabled state and fade once. Classic action artwork refines Select
all while preserving default colorful mappings. The v11 ribbon's duplicate
selection-action row is removed, so choosing Select keeps the canvas viewport
stable; Selection/Edit menus and shortcuts retain the working commands.
All 168 normal UI tests, seven SVG/cache checks plus the full contact-sheet
exporter, GPU dropdown export and strict workspace Clippy pass. Four theme/DPI/
window cases verify width, fonts, real glyph textures, selection marker, safe
previews and one publication per Select all/Deselect action. A real-pixel test
continues to verify paint restrictions and removing them through the popup.
The icon contact sheet now includes all 98 default IDs in nine rows, with
normalized size and contrasting tiles for white artwork; it previously clipped
entries beyond 72. The registry has 35 menu/control sources. Raw comparisons,
selected popup states and the full sheet are in [the Selection record](validation/selection-menu-gpu-2026-10-23/validation.json).
Unimplemented selection operations, historical parity and full Windows fidelity
remain open. Shared Classic popup styling is not historical-skin qualification.

### Text toolbar spacing and font chevron (2026-10-23)

The text bar now gives the family, size, formatting, alignment and background groups the spacing shown in `mspaint-text-settings.png`. Its rounded panel has 20-point horizontal and 10-point vertical insets; the background checkbox is 24 points, and the family picker uses a thin outline chevron. The existing 40-point controls, searchable installed-font list, staged size entry and editable canvas text behavior are preserved.

Validation: 168 UI tests passed (19 opt-in tests ignored), including narrow scrolling, repeated layout, font list virtualization and cancellation coverage. Light/dark GPU theme exports and strict Classic Clippy passed, along with formatting and source-length checks. Raw captures and logs: `docs/validation/text-toolbar-spacing-2026-10-23/validation.json`. This is a presentation refinement; native Windows fidelity and remaining text semantics are still incomplete.

### V11 Tools outline artwork and medium ribbon layout (2026-10-23)

The six Tools controls now use original tintable SVGs for Pencil, Fill, Text, Eraser, Color picker and Magnifier. These are distinct from the colored sheet and the menu glyphs; the cache includes their variant. The v11 controls have centered 24-point artwork, 40-point targets, theme tint and selected outlines. Picker/magnifier stay disabled with their existing availability hints. Historical skins and Studio retain their existing tool artwork.

At medium widths below 1200 points, group gaps tighten to keep the complete ribbon and Layers reachable. Larger windows keep the roomier spacing; small windows retain horizontal scrolling. An initial wider-button layout broke medium-width Layers access and was corrected before committing.

Validation: 168 normal UI tests pass (19 opt-in tests ignored), eight ordinary SVG/cache tests pass, both light/dark GPU theme exports pass, and strict workspace Clippy passes. Tests cover actual centered glyph rectangles, target sizes and tool routing without history publication under repeated layout. The six sources regenerate byte-identically and render with transparency/tint at four densities. Raw scenes and logs: `docs/validation/classic-tools-gpu-2026-10-23/validation.json`. Full Windows fidelity and unavailable tool semantics remain open.

### Desktop Shapes and Colors proportions (2026-10-23)

Classic v11 now uses a roomier ribbon at widths of at least 1200 logical points. Shape cells grow from 24 to 28 points, glyphs from 19 to 22, and their scrolling viewport from 78 to 90 points. Palette cells grow from 20 to 24 points with six-point horizontal gaps and correctly sized hover rings. Edit colors has a 40-point target and 32-point artwork. The desktop group content baseline is 108 points so captions align. Compact dimensions and scrolling remain available below the breakpoint. Existing shape ordering, availability, palette colors, named tooltips and action routing are preserved.

Validation includes repeated resizing across both sides of the breakpoint in light 1x/dark 2x with forced repeated layout. Geometry, caption alignment, final shape-row access, Layers, primary/secondary selection, Rectangle and color-dialog cancellation are checked without document/history publication. All 169 normal GUI tests pass (20 opt-in tests ignored), plus strict workspace Clippy and formatting/source checks. Eight fresh desktop/compact light/dark GPU captures and logs are archived in `docs/validation/gallery-density-gpu-2026-10-23/validation.json`. This is a presentation refinement, not full Windows parity or additional shape-engine support.

### Floating slider shells and accent centers (2026-10-23)

Classic floating size and opacity controls now use neutral gray circular shells with solid accent centers, following the supplied full-window Paint reference. Their input lanes grow to 32 points; the center responds to hover, keyboard focus and pressed state. egui still owns interaction, range handling, keyboard/accessibility events and rail fill. A small decoration reads the updated value after the widget and follows egui 0.37 circle-handle geometry. The size slider remains 1..64 while custom numeric values through 1024 remain intact. Numeric footers remain a PaintCraft extension.

The alignment test compares the actual egui shell shape and decorated center instead of computing an expected position from the implementation. It covers endpoints, midpoint, custom oversized values, accessibility changes, keyboard steps, pressed pointer drags and release in light 1x/dark 2x under repeated layout, without document/history changes. All 170 normal UI tests pass (21 opt-in tests ignored), with an additional final targeted pass after extending drag coverage. Strict workspace Clippy, formatting, source-length and whitespace checks pass. Eight raw light/dark GPU states and logs are in `docs/validation/slider-thumbs-gpu-2026-10-23/validation.json`. This does not qualify full native/Windows appearance or add brush semantics.

### Edit colors proportions, contrast and footer (2026-10-23)

The Classic color modal now follows `11111imagekr.png` more closely: desktop padding is 30 points, the heading 24 points, body labels 16 points and numeric/model/acceptance targets 40 points. The desktop preview grows to 54 points; the brightness lane is wider. Basic and custom circles grow to 24 points with 14-point gaps and clearer borders. The model picker uses an outline chevron; the custom add action is larger and right aligned. A separate theme-aware footer surface holds OK/Cancel outside the body scroller. Compact windows retain denser stacked layouts, scrolled access and fixed acceptance actions.

All 170 ordinary GUI tests pass (21 opt-in tests ignored). Final responsive geometry/cancellation checks pass after the last border/geometry assertions; strict workspace Clippy and source/format checks also pass. Eight fresh light/dark desktop/model-popup/compact-scrolled GPU scenes plus an explicitly different-size historical before capture are archived in `docs/validation/color-dialog-gpu-2026-10-23/validation.json`. Staged color/custom-slot behavior, invalid-input rejection, RGB/HSL changes, nested dismissal and document/history isolation remain covered.

The existing 28 custom slots remain a PaintCraft extension. Centered resting DragValue text, gradient corners/marker details and complete native Windows fidelity remain open; this slice adds no color-management or engine semantics.

### Rounded picker gradients and brightness endpoints (2026-10-23)

The hue/saturation square and brightness bar now use rounded textured rectangles, with egui antialiasing their edges. The hue marker is larger and clipped to the square. Brightness has a gray shell and white center; its inset travel range keeps the entire thumb inside the lane, and pressing its displayed endpoint retains the value. Existing pointer/keyboard input and staged color semantics remain intact. Two context-local textures have a bounded 257 KiB RGBA payload: 256×256 for hue/saturation and 1×256 for brightness. Brightness color changes update the existing texture rather than adding cache entries.

Validation: all 171 normal UI tests pass (21 opt-in tests ignored), seven Classic unit tests pass, and strict workspace Clippy/format/source checks pass. A new repeated-layout endpoint test covers press stability, marker bounds, keyboard changes and cancellation at 1x/2x. GPU checks verify actual rounded-corner background pixels and an interior hue sample, with eight refreshed light/dark desktop/popup/compact captures. Matched earlier desktop captures and raw logs are archived in `docs/validation/picker-gradients-gpu-2026-10-23/validation.json`.

Numeric resting values still use centered egui DragValue text; its public API does not expose alignment. Full native Windows fidelity and color-management work remain open. This slice makes no rendering-performance claim.

### Ribbon display-options chevron (2026-10-23)

Classic v11 now exposes its existing toolbar visibility modes through the lower-right chevron visible in the full-window reference. The 28×24-point trigger follows the ribbon caption row, aligns to the far edge when space permits and stays inside the horizontal scroller on narrow windows. Its 280-point popup body uses contiguous 40-point rows, selected markers and the existing Always show toolbar / Automatically hide toolbar actions. It shares persistence with View and retains accessible naming plus hover/open feedback.

All 172 normal UI tests pass (22 opt-in tests ignored), with explicit trigger/row bounds, selected-state, Escape, persistence, hide/reveal and document/history isolation checks at wide/medium/narrow sizes under repeated layout. Existing gesture-coordinate safety and Studio isolation checks pass. Strict workspace Clippy and source/format checks pass. Eight raw light/dark desktop/compact/hidden GPU captures and logs are in `docs/validation/ribbon-visibility-gpu-2026-10-23/validation.json`. The popup uses shared Classic conventions; no direct close-up of the native chevron popup was supplied, so exact Windows popup parity is not claimed.

### Classic Layers outline controls (2026-10-23)

The v11 ribbon Layers button now uses original paired light/dark outline SVGs with a blue accent, following the full-window reference. Its 44-point image, 60-point button and existing selected/hover surfaces are preserved. The shared Classic Layers panel now uses monochrome Close/Delete and new Add/Move up/Move down vectors through the established outline button helper. Default colored artwork and Studio controls remain available unchanged. Historical Classic panel consumers share this styling without a historical-parity claim.

All 172 ordinary GUI tests pass (23 opt-in tests ignored), including existing layer operations, property/draft behavior and thumbnail isolation. Panel actions additionally check centered 22-point theme-tinted glyphs in light 1x/dark 2x with repeated layout. Eight ordinary SVG/cache tests and strict workspace Clippy pass; both generators reproduce assets byte for byte. The default registry still has 98 IDs; outline menu/control sources total 38 and paired accented ribbon sources total 20 files for 10 logical IDs. Six fresh light/dark hover/open/action GPU captures and raw logs are archived in `docs/validation/classic-layer-controls-gpu-2026-10-23/validation.json`.

The supplied Classic reference establishes the ribbon glyph, not the complete Layers panel. Panel action art follows shared Classic conventions; the Studio Layers screenshot is not evidence of native Classic panel parity. This adds no layer operation or performance claim.

### Classic footer and zoom artwork (2026-10-23)

Classic metric/zoom controls now use original paired theme SVGs for pointer, selection, canvas dimensions, fit, zoom in and zoom out. The dimensions glyph depicts canvas bounds rather than a picture file, and the zoom glyphs preserve blue accents. Existing 16-point metric and 22-point action sizes remain. A shared internal artwork enum extends action buttons to the status family while preserving enabled/checked state and the single shortcut tooltip. Default colored and Classic-menu button APIs keep their existing mappings; shared Classic zoom consumers inherit the status family.

All 172 ordinary GUI tests pass (24 opt-in tests ignored), including desktop/compact metric geometry, current-frame pointer facts, real selection dimensions, zoom behavior and document/history isolation. Nine ordinary SVG/cache tests pass, including both status themes at four densities, coverage/accents and independent cache identities. The 98 default IDs remain; this adds 12 status-family sources. Generation is byte-identical. Strict workspace Clippy and source/format checks pass. Six light/dark desktop/hover/compact GPU captures and logs are archived in `docs/validation/classic-status-icons-gpu-2026-10-23/validation.json`. Percentage-field grouping, slider style and full native Windows/footer and historical parity remain open; this slice adds no zoom-engine semantics or performance claim.

### Joined zoom selector and Classic thumb surfaces (2026-10-23)

The footer percentage field and preset trigger now form a joined 30-point surface, with larger entry text, shared fill, complementary corners and an outline chevron. The staged percentage widget lives in `zoom/percentage.rs`; acceptance, invalid-input handling and Escape cancellation retain their existing behavior. Floating tool and horizontal zoom sliders share gray-shell/accent-center decoration, while egui owns input/accessibility. Zoom follows its existing logarithmic range. Desktop windows at least 1200 points wide reserve 400 points for zoom controls with a 160-point track; smaller windows retain 340/100-point dimensions. Status message allocation reserves the matching width.

All 173 normal UI tests pass (24 opt-in tests ignored). The new light 1x/dark 2x test checks joined edges/height, actual canvas zoom and the accent center against egui shell geometry at four logarithmic values. Existing vertical-slider, typed draft, pending-text Escape, wide/compact bounds, metric alignment and history-isolation checks pass. A five-point vertical selector regression found by the full suite was corrected before committing. Strict workspace Clippy and source/format checks pass. Six fresh light/dark desktop/hover/compact GPU scenes, matched earlier desktop captures and logs are archived in `docs/validation/classic-zoom-surfaces-gpu-2026-10-23/validation.json`. Exact Windows focus/hover/popup and historical parity remain unqualified; no zoom-engine or performance claim is made.

### Brightness accessibility and keyboard correctness (2026-10-23)

The color brightness control previously exposed a slider role without handling accessibility value requests or providing complete range metadata. It now uses the existing egui Slider for input/focus/range/accessibility behavior, retaining the custom gradient and marker paint. Coalesced clicks remain supported through click-and-drag interaction; the endpoint mapping follows egui circle geometry. Arrow events are consumed after egui handles them, preventing duplicate steps on discarded layout passes. A focused thumb has an accent ring. No dependency or color-engine change is involved.

All 174 ordinary UI tests pass (24 opt-in tests ignored). A new 1x/2x test compares single/repeated layout, accessibility values/ranges, visible focus, staged changes, one-arrow consistency, Cancel preservation and OK adoption without document/history publication. Existing pointer/endpoint/RGB/HSL/responsive cases pass. Strict workspace Clippy and source/format checks pass. Ten fresh light/dark desktop/popup/compact/focus GPU captures and logs are in `docs/validation/brightness-input-gpu-2026-10-23/validation.json`; rounded-corner pixel checks also pass. Native egui keyboard stepping replaces the old manual fixed increment.

This qualifies synthetic egui/AccessKit requests and metadata, not physical VoiceOver/Narrator operation. Complete native accessibility and Windows appearance remain open.

### Shared slider quick-click correctness (2026-10-23)

New regression tests reproduced ignored coalesced primary press/release rail clicks in Classic size/opacity and zoom sliders; keyboard layout equivalence already passed. A shared private slider input path now completes those quick taps, preserves native changed flags and consumes handled axis-arrow events. Ordinary drags retain egui smart aim and accessibility handling. Tool values round to their integer ranges; zoom retains its logarithmic mapping and centipercent precision. Changed taps request relayout/repaint to settle native and decorated thumb surfaces together. Brightness uses the common path as well.

All 177 ordinary GUI tests pass. New 1x/2x checks cover tool/opacity taps, zoom midpoint and both endpoints, single/repeated-layout keyboard consistency and document/history isolation. Existing brightness accessibility and pointer/geometry cases remain green. The full run ignored 24 opt-in tests; one further review exporter was then added and run separately, with final strict all-target workspace Clippy covering it. Six light/dark GPU captures show actual tap results, and the failing-before log is retained in `docs/validation/classic-slider-input-gpu-2026-10-23/validation.json`. Formatting/source checks pass. This is an input correction, not a physical-device, Windows-parity or performance qualification.

### Classic Settings gear, navigation and text contrast (2026-10-23)

V11 Settings now uses an original outline gear matching the supplied full-window reference. The Classic Settings dialog uses 22-point gear/canvas/text navigation artwork, 40-point sidebar choices and explicit selected outlines. Its heading and body/description text have clearer contrast. Styling follows the current host experience, not the staged interface radio choice; Studio retains its colorful icons, dimensions and text treatment. Historical Classic top-bar artwork stays on its existing path while shared Classic Settings navigation inherits the refinement.

All 178 ordinary UI tests pass (25 opt-in tests ignored). A new light/dark regression check measures two description text colors against their actual painted opaque card backgrounds and requires a 4.5 contrast ratio; this is not a whole-UI compliance claim. Existing theme preview/cancel/apply/restart, compact navigation/footer, preferences, workspace-switch and document/history tests pass. Nine SVG/cache tests, reproducible generation, strict workspace Clippy and source/format checks pass. Six relevant fresh Classic/Studio light/dark GPU scenes and logs are archived in `docs/validation/classic-settings-chrome-gpu-2026-10-23/validation.json`. No preference or persistence semantics changed. The supplied reference establishes the gear; full native Classic Settings layout and historical parity are not claimed.

### Canvas dimension/default dialog surfaces (2026-10-23)

Canvas size and Default canvas size now use the current Classic dialog vocabulary: 16-point margins, 12-point rounded frames, 24-point headings, 16-point body text, clearer secondary text and 40-point fields/actions. The Dimensions group spans the body. Roomy dialogs shrink to content; short displays retain a scrolled body and fixed acceptance actions. A reserved, visible scrollbar makes the lower startup-policy choices discoverable without covering fields. Resize/default behavior stays with the unchanged host.

All 178 ordinary UI tests pass (26 opt-in tests ignored), including existing wide/compact, light/dark, 1x/2x repeated-layout dialog checks. Scrolled actions stay fixed/reachable; cancellation preserves dimensions, policy, preference bytes and history. Actual default/remembered-size behavior and restart checks pass. Strict workspace Clippy and source/format checks pass. Ten fresh light/dark current/default/compact/scrolled-policy GPU scenes and logs are in `docs/validation/canvas-dialog-surfaces-gpu-2026-10-23/validation.json`. This refines app-specific shared dialog presentation, not native Paint Properties parity or resize semantics.

### Shared confirmation surfaces and measured action rows (2026-10-23)

Save/discard/cancel prompts now use the current dialog surfaces: 16-point margins, 12-point rounded frames, 24-point headings, 16-point body/button text and 40-point actions. Button-row versus stacked layout uses measured labels with padding/frame allowance. Body space accounts for wrapped headings, and long explanations use a reserved visible scrollbar above the fixed action area. Caller-owned choices, Escape/backdrop behavior and default focus are unchanged. Shared Studio consumers inherit this presentation refinement.

All 178 ordinary UI tests pass (27 opt-in tests ignored), including long/unbroken messages, 1x/2x light/dark compact/desktop bounds, stable action placement during scrolling, exactly-once choices under repeated layout, dirty-close cancellation and interface switching. Strict workspace Clippy and source/format checks pass. Twelve fresh real-context New/Close/interface-switch light/dark desktop/360×360 GPU captures and logs are in `docs/validation/confirmation-surfaces-gpu-2026-10-23/validation.json`; each review prompt is cancelled and its document stays dirty. No native prompt close-up was supplied, so exact native/historical parity and OS accessibility are not claimed. No save/discard workflow or auto-accept behavior was added.

### Modal skin chooser and responsive surfaces (2026-10-23)

The Classic skin chooser now gates background command/canvas input and uses a modal focus layer with a dimmed workspace. A regression test reproduced background Undo while choosing a skin before the fix. The chooser follows current dialog typography, margins and rounded surfaces, with a 24-point heading, outline Close button and 40-point actions. Compact choices scroll above a fixed footer. Selection remains staged until Apply/Continue; Close, Cancel and Escape retain the prior preference and unpublished canvas text. Thumbnails remain original schematics rather than rendered previews.

All 179 ordinary UI tests pass (28 opt-in tests ignored), including 1x/2x input isolation, painting resumption, history/preference preservation, repeated-layout bounds, acceptance/restart and pending-text Escape. The old dropdown-state test now explicitly chooses a skin rather than relying on first-visit click-through. Strict workspace Clippy and source/format checks pass. Eight light/dark desktop/compact GPU scenes, the failing-before log and a native macOS wgpu first-visit framebuffer are archived in `docs/validation/skin-chooser-surfaces-gpu-2026-10-23/validation.json`. The authorized native capture used isolated preferences/recovery and closed automatically with exit code zero. This is client presentation/input evidence, not full historical fidelity, physical accessibility or performance qualification.

### Zoom popup row alignment and initial range (2026-10-23)

The supplied `imag111111e.png` shows a left-aligned zoom list with a thin scroll cue and the current range visible. Classic now aligns percentage labels to the leading edge and scrolls to the nearest current preset only when opening. A custom 73% value reveals the vicinity of 75% without selecting it or changing the actual zoom. Users can still scroll away; reopening follows the current value again. Popup styling provides a visible resting scrollbar. The preset renderer lives in `zoom/presets.rs`, separate from draft acceptance.

All 180 ordinary UI tests pass (29 opt-in tests ignored). Added light 1x/dark 2x desktop/compact checks cover painted/clipped nearest rows at 73%, 1000% and 12.5%, leading insets, 40-point rows, free scrolling, reopening, Escape and document/history preservation. Existing preset adoption, typed drafts, fit, text preservation and joined controls pass. Strict workspace Clippy and source/format checks pass. Twelve fresh light/dark GPU captures and the failing-before log are archived in `docs/validation/zoom-popup-opening-gpu-2026-10-23/validation.json`. Full native Windows/historical fidelity and physical accessibility remain open; no zoom-engine or performance change is claimed.

### Text dropdown selection visibility and joined size control (2026-10-23)

Classic font family and size popups now share leading-aligned 16-point labels, 40-point rows and visible resting scroll cues. Opening reveals the selected family in the virtualized catalog, or the nearest size preset without adopting it. Search changes still reset to the top. Family-list height uses measured header space so the count footer stays visible. The size field and arrow form a 90×40-point control with complementary corners, following the text-toolbar reference. Existing typed-size drafts and host-owned preference updates remain intact.

All 181 ordinary UI tests pass (30 opt-in tests ignored). Added light 1x/dark 2x desktop/compact checks cover selection at family 900 of 1000, painted/clipped rows, leading insets, footer visibility, joined geometry, searching, scrolling and Escape. A custom 37-point value remains unchanged while its nearest 36-point preset is revealed without selection. Existing virtualization, long-name truncation, persistence and unpublished text/draft cancellation pass. Strict workspace Clippy and source/format checks pass. Eight deterministic-catalog popup GPU captures, two actual-app Selawik preview captures and the failing-before log are archived in `docs/validation/text-dropdown-surfaces-gpu-2026-10-23/validation.json`. Fixture catalog images do not qualify OS font enumeration; no text rasterization, IME, physical accessibility, full native/historical fidelity or performance claim is added. Resting DragValue number alignment remains unchanged.

### Leading numeric fields in resting and edit modes (2026-10-23)

Classic color RGB/HSL fields and the text point-size field now align their values to the leading edge with 14-point horizontal padding and 16-point body text, following the supplied color/text references. Explicit leading layout plus `DragValue::min_size` avoids the centered layout imposed by `add_sized`; egui still owns native numeric editing, ranges, dragging and accessibility. The color hex/model selector and model popup labels use matching leading insets. Font-size arrow padding remains separate so the joined control stays 90 points wide. This resolves the centered resting-value gap recorded in earlier slices.

All 183 ordinary UI tests pass (31 opt-in tests ignored). Two new light 1x/dark 2x regressions reproduce the prior centering and verify painted insets/font sizes, RGB resting/focused alignment, HSL alignment, accessibility values, joined geometry, cancelled font drafts and preference/document/history preservation. Existing compact layouts and color/text staging checks pass. Strict workspace Clippy and source/format checks pass. Twelve raw actual-app light/dark RGB/focus/model/HSL/font-size GPU states and the failing-before log are archived in `docs/validation/numeric-field-surfaces-gpu-2026-10-23/validation.json`. Numeric input semantics and HSL formatting remain intact; no full native/historical fidelity, physical accessibility, IME, color-management or performance claim is made.

### Canvas edge rails, arrow navigation and drawable viewport (2026-10-23)

Classic now reserves 14-point canvas edge rails with narrow egui thumbs and original arrow glyphs, following the supplied full-window reference. Arrow presses pan by 40 logical points, repeat after 400 ms then every 50 ms while held, and avoid a duplicate step on release. Wheel and thumb interaction remain egui-owned. The helper stores per-document navigation requests and settles rail geometry through relayout; repeated passes apply one step. V11 removes the inherited 20-point workspace inset, placing overflowing canvas content and rails flush with the workspace. Host viewport metrics now use the scrolling content clip rather than including scrollbar space. Studio keeps its own canvas presentation.

All 185 ordinary UI tests pass (32 opt-in tests ignored). New plain/ruled 1x/2x checks cover reserved geometry, named scrollbar roles, disabled boundaries, exact steps, native thumb dragging, held repeat and release behavior. App checks preserve document/history/preferences and start no painting from rail controls. Existing rulers, wheel/zoom, modal isolation, toolbar/status visibility and Studio tests pass. Strict workspace Clippy and source/format checks pass. Ten light/dark actual-app vertical/two-axis/ruled/compact/panned GPU captures plus independent clipped-edge white-pixel checks are archived in `docs/validation/canvas-edge-rails-gpu-2026-10-23/validation.json`.

Two authorized isolated native macOS wgpu launches captured a fitting 2000×1200 canvas and an overflowing 4000×2400 canvas at 2× display scale; both automatically closed with exit code zero. These qualify client presentation, not native physical pointer/tablet interaction or full Windows/OS accessibility parity. Historical Classic consumers inherit the helper without historical qualification. Resize handles remain explicitly disabled preview markers; bounds editing continues through Canvas size. No compositor performance or document semantics claim is added.

### Neutral selected toolbar controls and current-response feedback (2026-10-23)

The supplied main/text references show neutral selected control surfaces with blue outlines. Classic's v11 Tools, Selection/Brush primary buttons, Layers and shape choices, plus shared Classic text style/alignment controls, now follow that treatment. A private helper paints the neutral surface behind the ordinary egui Button and reads its current response for idle, hover and pressed feedback. Native egui input and selected accessibility metadata remain. Alignment glyphs use the strong foreground. Global blue text-selection and popup-choice fills stay intact.

All 186 ordinary UI tests pass (33 opt-in tests ignored). New light 1x/dark 2x repeated-layout checks inspect actual selected fills/outlines, distinct states, tool/Layers/text metadata, strong alignment strokes, preserved text-selection mesh colors and popup fills, and document/history isolation. The failing-before log records the former tinted Pencil surface. Strict workspace Clippy and source/format checks pass. Fourteen light/dark actual-app Pencil-state, Brush, Rectangle, text and Layers GPU captures are archived in `docs/validation/selected-control-surfaces-gpu-2026-10-23/validation.json`.

The dark references establish the treatment rather than every light/hover/pressed shade. Exact native pixel parity and OS accessibility remain unqualified; shared Classic text controls inherit styling without historical qualification. Studio/global selection colors and unavailable tool capabilities are unchanged. This adds no editing-engine or performance behavior.

### Joined Selection/Brush primary controls (2026-10-23)

Classic v11 Selection and Brushes now have 48-point-wide joined controls: a 48×48 main target and a contiguous 48×20 menu target. The selected neutral fill and accent boundary span both parts; arrow hover/open feedback stays within the lower target. A 64-point caption column preserves ribbon grouping while a 12-point top inset aligns the shorter control. Main and arrow retain separate egui Button/MenuButton identities, labels and keyboard targets. Existing Selection/Brush menus and host action routing are reused. The neutral fill calculation is shared with other Classic controls.

All 187 ordinary UI tests pass (34 opt-in tests ignored). New light 1× desktop/dark 2× compact repeated-layout checks cover exact joined bounds, the combined selected surface, independent primary/popup actions, open metadata, Space activation, Escape and document/history preservation. Existing glyph/caption tests require the centered 32-point image in its new 48-point target; tooltip, menu scroll and working selection/brush workflows pass. Strict workspace Clippy and source/format checks pass. Sixteen actual-app light/dark desktop/compact selected/open GPU captures plus the failing-before log are in `docs/validation/joined-tool-controls-gpu-2026-10-23/validation.json`. This refines the v11 triggers rather than qualifying complete native/historical fidelity or OS accessibility. Selection popup rows still need reference-specific selection treatment; no tool/media capability or performance change is claimed.

### Selection mode indicator and neutral popup row (2026-10-23)

The supplied Selection popup shows Rectangle marked while Pencil is active. Rectangle now stays checked as the supported selection mode independently of the active drawing tool; opening the menu leaves that tool intact, and choosing Rectangle still activates Select through the existing action. The selected row uses a neutral rounded surface with current-response hover/focus/press feedback, keeping its 3×22-point leading accent indicator without a tinted full-row fill or accent perimeter. Ordinary egui Button input and selected metadata remain. This resolves the popup-specific selected-row treatment noted in the preceding slice.

All 188 ordinary UI tests pass (35 opt-in tests ignored). Added light 1× desktop/dark 2× compact repeated-layout checks inspect actual fill/stroke/indicator shapes, hover feedback, Rectangle metadata while Pencil remains active, keyboard focus/Enter activation and document/history isolation. Existing bounds, scrolling, single tooltips, disabled previews and Select All/Deselect publication tests pass. Strict workspace Clippy and source/format checks pass. Eight actual-app light/dark desktop/compact idle/hover GPU captures and the failing-before fill log are archived in `docs/validation/selection-row-surfaces-gpu-2026-10-23/validation.json`. Rectangle remains the supported mode; this adds no other selection or deletion capability. Shared Classic presentation inherits the refinement without historical, full native Windows, OS accessibility or performance qualification.

### Custom ribbon palette slot targeting (2026-10-23)

Clicking an empty v11 ribbon custom-color slot now passes that slot through a typed UI action into the staged color editor. Previously the editor always targeted the first empty slot, even when another swatch was clicked. Existing generic editor entry points keep their first-empty behavior; filled swatches retain primary/secondary selection. Invalid slot indices retain the normal default selection.

All 189 ordinary GUI tests pass (35 opt-in tests ignored), with a new 1x/2x repeated-layout regression for slot 10, Cancel, OK, persistence, a single populated slot and document/history isolation. All 23 shared-UI unit tests and strict workspace Clippy pass. The supplied main/text screenshot palette samples match the existing twenty fixed RGB colors; no fixed color values changed. An isolated native Classic launch captured its framebuffer and closed successfully. This is a launch/presentation smoke check, not native interaction or OS menu/title-bar evidence. Logs and the framebuffer are archived in `docs/validation/custom-palette-target-2026-10-23/validation.json`. T15 remains partial.

### Stable brush gallery opening and overflow (2026-10-23)

Classic v11 now reveals its active round/square preset on opening the Brushes gallery, then allows free scrolling until it is closed. A thin resting scrollbar indicates overflow. The gallery uses a stable screen-relative height limit: the previous cursor-relative calculation shrank the popup repeatedly as egui moved it into a short viewport, eventually clipping the selected bottom row. Existing preset availability, dimensions and decorative samples remain.

All 190 ordinary GUI tests pass (36 opt-in tests ignored). New light 1x/dark 2x desktop and compact repeated-layout checks cover selecting Square brush, manually scrolling away, Escape/reopening, the selected row being fully visible, stability through 12 extra frames, free scrolling and document/history preservation. Strict workspace Clippy and source/format checks pass. Eight light/dark desktop/compact GPU scenes show the selected row and subsequent scrolling; logs and renders are archived in `docs/validation/brush-gallery-opening-gpu-2026-10-23/validation.json`. Square brush remains a PaintCraft extension, while other media implementations and complete native fidelity remain partial.

### Stable compact scrolling menu bounds (2026-10-23)

Classic File/Edit/View, Selection, Rotate/Flip and the brush gallery share a screen-relative popup height limit with a resting scrollbar cue. The limit reserves the main menu row and popup frame/edge clearance, retaining heading access for menu switching. The old calculation used a popup cursor that moves during area placement; the short Selection popup could keep shifting after a row had been scrolled into view. Shared bounds no longer depend on that moving cursor.

All 191 ordinary GUI tests pass (37 opt-in tests ignored). Added light 1x 480×360 and dark 2x 480×260 repeated-layout checks cover upper/lower rows in six menus, actual clipping bounds, coordinate stability over 12 additional frames and preserved document/history/settings. Existing main-menu switching, off-screen Zoom hover isolation, keyboard submenu navigation, brush reopening and disabled-preview tests pass. Strict workspace Clippy and source/format checks pass. Twenty-four actual-app light/dark GPU captures show upper/lower positions in the six menus. Logs, hashes and images are archived in `docs/validation/compact-popup-stability-gpu-2026-10-23/validation.json`. Native Windows/historical fidelity, OS accessibility and unsupported editing operations remain partial.

### Keyboard font search and staged selection (2026-10-23)

Classic font search supports Up/Down and Page Up/Down across its virtualized result list. A pending choice has an accent outline and is named in the search field’s accessibility description; the current font remains unchanged until Enter or a row click. Enter applies a matching candidate and closes the popup. Empty-result Enter leaves it open; Escape cancels. Candidate scrolling occurs on keyboard navigation or opening, preserving manual browsing. During IME composition the platform’s preedit/commit events reach TextEdit while navigation keys cannot move its composition cursor or accept the font.

All 194 ordinary GUI tests pass (37 opt-in tests ignored). New desktop 1x/compact 2x repeated-layout tests cover a 1000-family fixture catalog, arrows/pages, candidate visibility/description, acceptance/cancellation, empty results and synthetic IME routing. An actual App test verifies Enter acceptance persists Selawik without publishing or altering an editable canvas text draft or its document/history. Separate View alignment coverage now includes Canvas size at desktop width. Strict workspace Clippy and source/format checks pass. Sixteen light/dark desktop/compact component GPU captures use Selawik for UI text and a deterministic fixture catalog; they show family/size lists, pending choices and accepted labels. Evidence is archived in `docs/validation/font-keyboard-navigation-gpu-2026-10-23/validation.json`. These captures are component scenes, and synthetic IME/AccessKit checks do not qualify native OS input/accessibility. Text rasterization, complete native fidelity and the broader roadmap remain partial.

### Consistent Classic Settings surfaces and Close cancellation (2026-10-23)

Classic Settings now uses 16-point body/button text, 40-point targets, a 16-point frame margin and 12-point corners, matching the application’s newer Classic dialogs. An upper-right Close control cancels the staged draft and restores theme preview. Sidebar icons precede aligned labels, and selected navigation reuses the existing neutral Classic button surface with an accent outline. Compact content has a resting scroll cue; the footer remains fixed. Studio metrics are retained.

All 195 ordinary GUI tests pass (38 opt-in tests ignored). New repeated-layout checks cover light desktop 1x/dark desktop 2x and light 360×360 1x/dark 480×360 2x layouts: target/typography metrics, neutral fills, selection metadata, icon/text order, leading alignment, upper-right placement, one Close tooltip, mouse/Space cancellation, theme restoration and preference/document/history isolation. Existing Apply/Cancel, responsive page/footer, text contrast and interface-switching tests pass. Strict workspace Clippy and source/format checks pass. Sixteen actual-App light/dark desktop/compact GPU captures show all three pages plus Close hover. Logs, hashes and images are archived in `docs/validation/classic-settings-surfaces-gpu-2026-10-23/validation.json`. Font counts are asynchronous snapshots. Exact native Windows/historical fidelity, OS accessibility and broader T15 acceptance remain incomplete; prototype-only preferences retain their explanatory text.

### Visible Classic radio and checkbox boundaries (2026-10-23)

A scoped Classic form style now gives idle/hover/focus/pressed controls visible borders and neutral fills. Settings content, canvas dimension/default content and the text-toolbar Background fill checkbox use it. Native egui radio/checkbox input and accessibility metadata remain; application chrome, modal actions and Studio keep their styles. The previous idle Settings radio had no outline.

All 196 ordinary GUI tests pass (39 opt-in tests ignored). New light 1x/dark 2x repeated-layout checks inspect native control shapes and their actual surrounding opaque surfaces, including frames nested in shape groups. They require opaque outlines and at least 3:1 contrast against both the fill and surrounding card in idle, hover, pressed and focused states. Distinct feedback, mouse/Space toggles, draft cancellation, preference restoration, document/history isolation and unchanged global widget styling also pass. Pressed-state inspection uses one frame before release, avoiding a simulated long press. Strict workspace Clippy and source/format checks pass. Sixteen actual-App light/dark desktop/compact GPU scenes show appearance radios, a checked staged text preference, canvas startup choices and checked background fill. Evidence lives in `docs/validation/classic-form-boundaries-gpu-2026-10-23/validation.json`. This is a scoped boundary-contrast check, not complete accessibility certification, OS accessibility or native Windows/historical fidelity qualification. Broader T15 acceptance and prototype-only text features remain incomplete.

### Shared leading dimension fields (2026-10-23)

Classic Settings, Canvas size and Default canvas size reuse dimension rows with a measured common caption column, leading captions and 40-point native numeric fields. Sixteen-point values sit at a 14-point leading inset in resting and keyboard-editing layouts, matching other Classic numeric controls. Previously the Settings labels placed the fields almost six points apart, while the canvas dialog used centered child layouts. Native range handling, pixel suffixes, host-owned drafts/persistence and document commands remain. Studio keeps its existing fields.

All 197 ordinary GUI tests pass (40 opt-in tests ignored). New light desktop 1x/dark compact 2x repeated-layout checks cover the three surfaces, aligned field/caption columns, value/label rendering and font metrics, numeric accessibility values, typing/Enter and cancelled preferences/document/history. Existing resize, undo/redo and remembered-default workflows pass. Strict workspace Clippy and source/format checks pass. Twenty-four actual-App light/dark desktop/compact idle/focused GPU scenes use staged 777/512 review inputs, then discard them and verify unchanged state. Evidence is archived in `docs/validation/classic-dimension-fields-gpu-2026-10-23/validation.json`. Exact native Windows/historical fidelity, OS input/accessibility and broader T15 acceptance remain partial; this adds no resize algorithm or performance behavior.

### Ribbon stroke-size control (2026-10-23)

The supplied main screenshot includes a size control beside the shape options. [Microsoft’s 2021 Paint redesign announcement](https://blogs.windows.com/windows-insider/2021/09/28/redesigned-paint-app-for-windows-11-begins-rolling-out-to-windows-insiders/) also identifies stroke-size dropdowns. Classic v11 now exposes that control through an original tintable SVG and a 1/3/5/8 px menu with line samples. It updates the existing size preference for Line, Rectangle and Ellipse, and is disabled for other tools, missing documents or modal work. A custom value remains explicit without falsely marking a preset. Wide layouts place the control beside Outline/Fill; compact layouts stack three targets under the existing caption baseline. Medium spacing tightens while keeping existing full-size swatches/glyphs and Layer reachability. The icon generator, manifest and mapping now contain 40 menu/control variants.

All 199 ordinary GUI tests pass (41 opt-in tests ignored), together with 23 shared-UI unit tests and strict workspace Clippy. New repeated-layout checks cover light desktop 1x/dark compact 2x availability, single tooltips, shared size synchronization, a custom 37px width through resizing, sample geometry, checked metadata, keyboard/Escape, modal gates and document/history isolation. An independent line-drawing check measures different raster thicknesses after 1px/8px choices and verifies undo/redo through the existing engine. Existing density/caption/Layer workflows pass. Eighteen actual-App light/dark desktop/medium/compact GPU captures show disabled, custom-current and selected-preset states; evidence lives in `docs/validation/classic-stroke-size-gpu-2026-10-23/validation.json`. The screenshot and dated announcement establish trigger presence; exact current native popup geometry/values and original-app raster placement remain unqualified. The preset/readout presentation is PaintCraft’s implementation, with no new shape kernel, performance or complete native fidelity claim.

### Restored tool icon color accents (2026-10-23)

The earlier monochrome Tools treatment is superseded by six pairs of original light/dark SVGs. Pencil keeps a gold shaft, Fill and Eyedropper keep cyan details, Eraser keeps pink, and Magnifier keeps a blue plus. Text uses neutral themed ink. The ribbon and Settings Text navigation use untinted artwork, preserving these colors. Button geometry, selected surfaces, availability and routing remain unchanged. Regenerate with `python3 scripts/generate_classic_tool_icons.py`.

All 199 ordinary GUI tests and 23 shared UI unit tests pass; strict workspace Clippy passes. Multi-density tests cover color families, identical theme coverage and separate cache identities. Two native Wgpu windows were launched with isolated settings/recovery directories and their light/dark framebuffers reviewed. Logs, reference hashes and captures are recorded in `docs/validation/classic-tool-colors-native-2026-10-23/validation.json`. This corrects reference color families; exact Windows fidelity and T15 remain partial.

### Consistent selected layer-row surfaces (2026-10-23)

Classic Layers now reuses the neutral selected-button surface with an accent outline, replacing its blue row fill. The shared helper retains custom thumbnail atom rectangles and ordinary egui input. Idle, hovered and pressed states follow the ribbon controls; layer preview placement, truncation, tooltips and actions remain. Studio is unchanged.

All 200 ordinary GUI tests pass (41 opt-in tests ignored). The regression first reproduced the former blue fill, then verified light desktop 1x/dark compact 2x repeated layouts, feedback, selected metadata, stable 40-point rows, mouse/Space target changes and document/history isolation. Existing thumbnail, scrolling and layer operations pass. Strict workspace Clippy, format and source-length checks pass. Fourteen actual-App Wgpu captures cover both themes and compact layouts; light idle, dark pressed and dark compact scenes were reviewed. Logs and hashes are archived in `docs/validation/classic-layer-surfaces-gpu-2026-10-23/validation.json`. No supplied reference proves complete native Classic Layers panel parity; this follows shared Classic conventions. T15 remains partial.

### Responsive Classic Layers forms (2026-10-23)

Classic Layers uses scoped 16-point text and the established neutral form fills with visible native checkbox/field boundaries. Measured property captions align the name and opacity fields. Fields remain 40 points on desktop and become 32 below 650-point window height; actions reduce from 40 to 32 in short panels. The layer list reserves more footer space and overrides egui's minimum scroll viewport to 40 points. This avoids clipping the opacity editor after increasing the form metrics. Studio retains its existing editor builders and styling.

All 200 ordinary GUI tests pass (41 opt-in tests ignored), including property transactions, cancellation, long-name truncation/tooltips, fixed footer positions, thumbnails, scrolling and history isolation. Additional assertions check aligned fields, adaptive heights, 16-point captions, scoped styling and properties above the status bar; compact target-selection checks scroll rows into view. Strict workspace Clippy, format and source-length checks pass. The actual-App Wgpu exporter passed with 14 captures; four final light/dark desktop/compact images were archived and reviewed. See `docs/validation/classic-layer-forms-gpu-2026-10-23/validation.json`. The supplied references do not prove full native Classic Layers panel geometry; this follows shared Classic conventions. T15 and the overall roadmap remain partial.

### Canvas dialog header Close cancellation (2026-10-23)

Canvas size and Default canvas size now have upper-right 40-point outline Close buttons, consistent with the existing Classic Settings and skin chooser. They return the existing cancellation response, discarding staged dimensions and startup policy. A measured header height bounds the scrolling body; a 48-point minimum viewport keeps the footer reachable on short displays. Existing Apply/Cancel/Escape behavior remains.

All 201 ordinary GUI tests pass (41 opt-in tests ignored), plus strict workspace Clippy, format and source-length checks. Added light desktop 1x/dark 360×260 2x repeated-layout checks cover right-edge alignment, fixed header/footer through body scrolling, one tooltip, mouse/Space cancellation, preference-byte preservation and document/history isolation. Existing resize/undo, remembered defaults and field tests pass. The actual-App GPU exporter produced ten light/dark desktop/compact scenes; four representative images were archived and reviewed. Evidence is recorded in `docs/validation/canvas-dialog-close-gpu-2026-10-23/validation.json`. This completes a canvas-dialog chrome gap; native Paint Image properties/Resize and skew parity, OS accessibility and broader T15 acceptance remain partial.

### Resize and skew presentation preview (2026-10-23)

The v11 Image control now opens an interactive **UI preview** rather than remaining a disabled ribbon placeholder. Its grouped Percentage/Pixels, horizontal/vertical dimensions, filled aspect-ratio link and skew fields follow [Microsoft's November 2021 dialog redesign reference](https://blogs.windows.com/windows-insider/2021/11/30/paint-app-for-windows-11-update-for-windows-insiders-brings-updated-dialogs-and-more/). This is a historical primary reference, not qualification against the current Paint build. The chain glyph is original SVG artwork through the shared density-aware cache. There are now 99 logical default icon IDs and 41 Classic menu variants; the supplied source sheet is unchanged.

Draft controls work locally: scale survives unit changes, integer pixel readouts stay at least one pixel, the link couples edits to the source proportions, and skew angles can be staged. These are presentation rules, not an adopted image-resampling contract. The footer always says Preview only; OK is disabled with an availability explanation. Close, Cancel and Escape discard the draft. No transform command is sent to the engine, no resize shortcut replaces document Close, and selection-target behavior remains unimplemented. The host rejects a stale source revision.

All 203 ordinary GUI tests pass (42 opt-in tests ignored), along with 23 shared UI tests and a scoped stale-source unit check. Tests cover repeated layout, desktop/compact scales, one-pixel/unit round trips, linking, numeric edits, modal undo isolation, disabled OK, cancellation, tooltips, no-document gating and saved preference/document/history preservation. Existing unsupported-control tests now distinguish this staged dialog from actual image-edit capability. Strict workspace Clippy, formatting, source lengths and asset reproduction pass. Twelve actual-App Wgpu scenes were generated; four representative final light/dark desktop/compact images were archived and reviewed in `docs/validation/resize-skew-preview-gpu-2026-10-23/validation.json`. Applying transforms, selection targeting, engine/CLI/undo conformance, exact native raster/layout parity, OS accessibility and broader T15 acceptance remain incomplete.

### Image properties and unchanged-dimension correctness (2026-10-23)

File → Image properties and Ctrl/Cmd+E now open a staged Classic dialog through one semantic action. The native File menu registers the same action and accelerator, with frontend availability enforced. The supplied `im1age.png` establishes the entry/shortcut; the dialog body follows shared Classic conventions rather than a pinned current Paint Properties screenshot. [Microsoft's historical Paint attributes instructions](https://learn.microsoft.com/en-us/troubleshoot/dynamics/gp/prevent-image-quality-of-graphic-from-being-reduced) also describe pixel Width/Height controls, without qualifying current Windows 11 presentation.

The actual editor filename is shown with bounded text and one full-name tooltip. Resolution says Not available: the current document model does not store DPI, so Inches/Centimeters stay disabled with an explanation. Black and white conversion is also disabled; Color reflects the existing color document model. Pixel dimensions work through the existing atomic canvas-size operation, keeping the top-left corner and preserving outside content as the dialog states. This does not resample pixels. Cancel/Close preserve the unpublished text draft, preferences and history. A stale document/tab cannot receive a staged change.

A new regression reproduced unchanged OK expanding a cropped layer and dirtying a clean native file. The shared dimension host now skips requests when dimensions are unchanged, covering Image properties and Canvas size. A valid fixture with a 1000×486 canvas and 32×16 layer stays semantically identical and clean. Backend command schemas and resize policy are unchanged.

All 207 ordinary GUI tests pass (43 opt-in tests ignored), together with 23 shared UI tests, stale-source coverage for both dialogs and a native-menu identity/accelerator unit check. Tests cover one publication from each menu/shortcut path, preserved outside pixels, undo/redo, repeated light desktop 1x/dark 360×260 2x layouts, cancellation, fixed controls, bounded filenames, one tooltip, missing-metadata states and modal text/backspace isolation. Strict workspace Clippy, formatting and source lengths pass. Eight actual-App Wgpu scenes were generated from the committed source; four representative final light/dark desktop/compact captures were archived and reviewed. Logs and hashes are recorded in `docs/validation/image-properties-gpu-2026-10-23/validation.json`. Live OS menu activation, complete file/DPI/color metadata, physical units, monochrome conversion, exact current native Properties fidelity and full T15 acceptance remain incomplete.

### Ribbon alignment, compact quick bar and browser presentation (2026-10-23)

The Layers target now follows the measured vertical center of the Colors row,
matching the color editor button while retaining its 44-point glyph, 60-point
hit target and hover/selected surfaces. The v11 quick bar is 36 points tall
(32 on short windows), and the additional File/Edit/View gaps are 6 points.
The zoom rail now fills blue from its left edge to underneath the thumb;
logarithmic input, accessibility values and history isolation are unchanged.

All 207 ordinary GUI tests pass, with 43 opt-in tests ignored. The actual-App
Wgpu exporter generated 12 scenes; the reviewed wide light/dark captures are
archived with logs in
[the validation record](validation/classic-ribbon-web-preview-2026-10-23/validation.json).
Workspace all-target Clippy, formatting, source lengths and whitespace checks pass.

`apps/paintcraft-web` now reuses the real Classic presentation, bundled Selawik,
SVGs, canvas view and zoom controls in a standalone WebAssembly **UI preview**.
Its release bundle started in an isolated headless Brave session without uncaught
JavaScript exceptions; settings and dark-to-light switching were exercised and
captured. A host UI test and strict wasm32 Clippy also pass. The canvas is blank:
browser drawing, documents, history, file handling, workers and recovery remain
unconnected, with an explicit preview banner and unsupported-action feedback.
See [browser build and scope](../apps/paintcraft-web/README.md).

Tablet-specific support remains incomplete on desktop: the shared `PenSample`
contract has pressure/tilt fields, but native acquisition and brush propagation
are not connected. Ordinary pointer events are the current drawing path.
T15 remains partial; this web scaffold does not satisfy T22 browser conformance,
Windows fidelity, physical tablet or cross-browser/device qualification.
