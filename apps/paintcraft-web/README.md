# PaintCraft Web

PaintCraft Web connects the shared Classic interface, tool state machines,
transactional document session and raster compositor. Pencil, all nine brush media and square brush,
eraser, fill, color picker, all 23 Classic shape choices, rectangular selection, selection Crop, clear canvas
and undo/redo edit real pixels. Shift constrains shapes; Ctrl/Cmd-scroll zooms. The
Layers panel adds, selects, hides, reorders and deletes real layers. Brush preview
and release use the same shared commands. Erasing and Clear Canvas restore the
solid project ground, with white used for new canvases and imported images that
have no opaque default ground.

The Color picker samples a document-image pixel on release into the primary
(left click) or secondary (right click) slot, then restores the previous tool.
It ignores the selection and viewport decorations, preserves composite RGBA and
creates no pixel edit/history entry. Picking from a clean document keeps it clean.
See [sampling and ownership rules](../../docs/COLOR_PICKER.md). Studio's layer/
neighborhood options remain in the desktop frontend.

Classic v11 now has the same floating size and brush-opacity controls as desktop.
Pencil thickness is independent of brush/eraser thickness; each gesture captures
its settings at press. Control drags and Ctrl/Cmd-scroll over those panels do not
paint or zoom the canvas. The canvas keeps stable clearance across tool changes.

View → Canvas size and File → Image properties edit staged pixel dimensions.
Resizing preserves off-canvas content, expands editable layer bounds and is
undoable. Unchanged dimensions do not publish history. The dialog enforces the
browser pixel limit before Apply. Physical units, resolution and monochrome
conversion remain unavailable.

Image → Crop and Edit → Crop use the selected canvas region. Crop discards
outside content from every layer, including hidden layers, preserves layer IDs
and metadata, and clears the selection. Undo restores the complete previous
state. Straight RGBA and hidden RGB survive project saving.
See [Crop's semantics and limits](../../docs/CROP_PLANNING.md).

Rotate/Flip execute right/left 90°, 180° and both mirrors. With no selection they
reorient the whole canvas and all layer extents. A rectangular selection transforms
the active layer's selected pixels at its top-left position, clearing the source
to the project ground and preserving overlap from captured samples. Undo restores
the exact previous state. New padding is transparent; off-canvas content remains
stored. See [selected-pixel transform semantics](../../docs/SELECTION_TRANSFORMS.md).
Custom angles, skew, resampling and a persistent floating selection remain work.

View → Default canvas size chooses a configured size or remembers the last size
created/resized/cropped/rotated. Undo/Redo preserve the last authored size. The initial default is 1000 × 486. Startup dimensions, theme,
Classic skin and custom swatches persist in this browser when local storage is
available. Preferences are validated before use; malformed or unavailable storage
falls back to defaults with a status message. Preference storage contains no
document pixels and provides no document recovery. Primary/secondary color and
other tool choices remain session state. Opened filenames update the page title.

Open uses a browser file picker for `.pcnd`, PNG, BMP and JPEG. Image metadata or
bit-depth losses require acknowledgement before replacing the current document.
Save/Save as immediately download an editable `.pcnd` copy without an extra modal;
Export confirms flattening and
downloads PNG. Inputs and project output are limited to 64 MiB. New/Open require
explicit discard when dirty; cancelling or failing an import retains the current
session. Native save/recovery services remain outside the browser build.

Text uses the same Cosmic Text shaping and Swash grayscale rasterization as
desktop, with bundled Selawik regular/bold and Noto Sans JP fallback. Click the canvas to start one editable
draft; the shared toolbar changes its style. Enter adds a newline,
Ctrl/Cmd+Enter or Place text commits, and Escape cancels. Backspace and
Ctrl/Cmd+A replacement work; arbitrary caret movement/range selection remain
unimplemented. Switching tools or editing layers places the draft first.
Downloads include a rasterized snapshot while retaining the editable draft and
its unsaved state. Failed placement retains the draft. Installed browser fonts,
multiple editable text objects and the native editable-until-save preference
are not implemented here. IME has synthetic event coverage, not device qualification.

Closed shapes include rounded rectangle, triangles, diamond, pentagon, hexagon,
four directional arrows, three stars, three callouts, heart and lightning.
They support fill/outline/both, stroke size, secondary colors and live Shift
constraints. Preview and release share their commands; placement is undoable.
Polygon starts with a dragged edge; subsequent clicks add vertices. Click its
start/last vertex or press Enter after at least three vertices to finish;
Backspace removes the last vertex. Curve starts with a dragged line and then
accepts two control-handle placements/drags. Enter can finish a curve early.
Escape cancels either draft. Save/Open/New/Export retain unfinished constructions
and explain how to finish/cancel first. Tool changes cancel unfinished drafts.
Completed shapes are raster transactions, not persistent editable vector objects.
The older Classic skins expose the same 23-entry gallery. Curved preset contours use
bounded fixed-point flattenings, not a Windows pixel-fidelity guarantee.

This is an early editor. Custom transforms and recovery
are not connected yet. Unsupported actions report this in the status bar.
The page requests the browser's
unsaved-work confirmation before unloading. Browser policy controls whether that
confirmation appears; it is not recovery or persistence. Downloads never mark the
tab saved: a download request cannot confirm disk persistence. Verify your copy
before closing or explicitly discarding the current document.

The current synchronous browser adapter admits at most 4,194,304 canvas pixels,
8192 points per stroke and a 64 MiB history retention budget (older entries may
be trimmed). It cancels oversized strokes rather than silently cutting them off.
Large-document workers/cancellation and physical memory/latency qualification
remain work. Browser history uses raw snapshots; native Zstandard codecs remain
native-only, while raw/LZ4 support compiles to WebAssembly. WebRunner presents the
interface through wgpu, with WebGPU and WebGL enabled; document composition is
currently CPU, not qualified GPU document rendering. No native services are used.

## Validation

```sh
cargo check --locked -p paintcraft-web --target wasm32-unknown-unknown
cargo clippy --locked -p paintcraft-web --target wasm32-unknown-unknown -- -D warnings
cargo test -p paintcraft-web
```

The ordinary UI test runs without opening a browser or native window. Compiling
to WebAssembly and testing the shared view on the host do not establish browser
compatibility; a served bundle still needs live browser review.

A 2026-10-23 local review built the optimized bundle and exercised startup,
settings and dark-to-light switching in isolated headless Brave. The screenshots,
commands and scope limits are recorded in [the validation evidence](../../docs/validation/classic-ribbon-web-preview-2026-10-23/validation.json).
The subsequent editor checks verified brush preview/release/undo pixels and real
project download, PNG import and painted project reopening. Host tests cover the
PNG export confirmation and codec output. See [the editor evidence](../../docs/validation/web-editor-2026-10-23/validation.json).
The next controls/preferences slice exercised typed dimensions and Undo, remembered
startup size after reload, actual theme/skin selection, custom-color persistence
and painting, and malformed/denied local storage. Host regressions also cover a
reopened colored-ground project. See [the controls evidence](../../docs/validation/web-classic-controls-2026-10-23/validation.json).
These single-browser smoke checks do not establish cross-browser conformance.

## Next browser slices

1. Add browser workers, memory admission and cancellation before large documents.
2. Connect document GPU composition and qualify WebGPU/WebGL behavior, touch,
   keyboard, browser zoom and pen pressure across actual browsers/devices.

Desktop tablet support is also incomplete: `PenSample` defines pressure and tilt,
but the canvas currently consumes ordinary pointer events, without native pen
sample acquisition or pressure/tilt propagation into brush rendering.

## Language and font preferences

Settings shares desktop's Appearance, Canvas, Text preferences and About layout.
Theme and language preview immediately; Apply stores the draft and Cancel restores
the previous choices. System, light and dark themes, all Classic skins, canvas
startup policy and default font are supported. English, German, Spanish, Hindi,
Japanese, Brazilian Portuguese and Simplified Chinese are available. Shared
Selawik and Noto fonts cover these scripts without installed-font access.

Opening an image updates the browser tab title. Project downloads keep its
basename and update the title to the requested project filename. Image exports
leave the project title unchanged; New returns it to Untitled. See the
[desktop parity ledger](../../docs/WEB_DESKTOP_PARITY.md) for remaining gaps.

Brushes use the same deterministic coverage and batch commands as desktop and CLI.
See [brush media](../../docs/BRUSH_MEDIA.md) and
[localization scope](../../docs/LOCALIZATION.md) for current behavior and limits.

## Run and build

Use Node.js 24 and pnpm 12.10.1. From the repository root:

```sh
pnpm --dir apps/paintcraft-web install --frozen-lockfile
pnpm --dir apps/paintcraft-web run dev
```

Open http://localhost:8088/. To create the static production site:

```sh
pnpm --dir apps/paintcraft-web run build
```

Astro and `vite-plugin-wasm` bundle the editor.
Rust sources and tests live in `src/paintcraft-web/`, separately from Astro pages
and JavaScript. The pnpm build prepares the pinned
Rust/Wasm package automatically, bootstrapping Rust and wasm-bindgen if needed.
The site is written to `apps/paintcraft-web/dist`; fonts come from the shared root
assets and are verified before the editor starts. Run `pnpm run build:wasm` in this
directory after changing Rust while the Astro dev server is running.

Run `pnpm run format` in this directory to format the browser source and build
scripts with Biome; `pnpm run format:check` checks the same files in CI.

## Cloudflare Pages

Choose Astro, set the root directory to `apps/paintcraft-web`, build command to
`pnpm run build`, output directory to `dist`, `NODE_VERSION` to `24` and
`PNPM_VERSION` to `12.10.1`. CI also
uploads the built site as a `paintcraft-web` artifact. Every asset is checked
against the 25 MiB limit. See [deployment settings](../../docs/WEB_DEPLOYMENT.md).

Ordinary Paste uses Ctrl/Cmd+V and private positioning before Apply/Enter. Repeated
drags keep frozen source pixels; Escape cancels and placement is one undoable
existing-layer edit. See [placement and limits](../../docs/FLOATING_PASTE.md).
