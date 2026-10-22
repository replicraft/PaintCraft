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
