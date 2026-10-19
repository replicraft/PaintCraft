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
rasterizer. Pencil retains exact pixel placement. See [brush rendering](BRUSH_RENDERING.md).

The full ribbon, brush media gallery, staged Edit colors dialog, clipboard,
selection variants, transforms and resize handles remain active development
work. This frontend is not yet a finished reproduction of Windows Paint.
