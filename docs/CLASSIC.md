# PaintCraft Classic

Classic is the current frontend priority. The additional Windows 11 Paint photos
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

## Current behavior

Classic has separate v7/v10/v11 presentation presets, SVG tool icons, centered
small canvases, native file dialogs, layer editing, typed edits and undo/redo.
The status bar provides percentage entry, 12.5–1000% presets, a zoom slider,
fit-to-window and zoom buttons. Invalid percentage drafts are discarded on
commit. View menu actions and keyboard shortcuts use the same dispatch path.
Zoom and the optional pixel grid do not change document pixels or history.
Keyboard zoom controls the canvas, preserving the size of the application UI.

The round brush preserves pointer positions in 1/256-pixel units and stores
antialiased coverage in the document. Preview and publication use the same
rasterizer. Brush opacity is adjustable from 0–100% and is captured for the whole
gesture. Zero opacity leaves pixels and history unchanged; Eraser retains its
explicit erase mode. Pencil retains exact pixel placement. See [brush rendering](BRUSH_RENDERING.md).

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
now has compact Image/Tools groups, a two-row shapes gallery, circular color wells,
a three-row palette, Settings gear and a collapsible Layers panel. Crop, transforms,
color picker and unsupported shapes are disabled previews. Resize handles are
visual markers; View → Canvas size remains the working resize path.

Settings provides Light, Dark and System appearance, with live theme preview,
Apply/Cancel, interface/version selection and canvas defaults. Both frontends use
the selected theme and bundled Selawik. Cancel restores the previous theme;
preferences do not alter document history. Existing settings files default to
System when the optional appearance field is absent.

Classic and Studio own separate canvas layout modules, scroll IDs and document
workspaces. Switching requires saving every changed document; the transition
offers Save and Cancel, with no discard option. Save cancellation/failure keeps
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

**Text styling is currently a UI prototype.** Changing these preferences does not
change the bitmap text renderer. Settings → Text preferences includes an explicitly
labelled prototype for keeping text editable until save. The preference persists,
but it does not yet create editable text objects or rasterize them on save.
That implementation needs styled text objects, font-face resolution/shaping,
editable overlays, undo/redo and an atomic save-time rasterization policy that
preserves live edits on failed/cancelled saves. It follows the UI/theme pass.
