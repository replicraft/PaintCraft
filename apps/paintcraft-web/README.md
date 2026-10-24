# PaintCraft Web

PaintCraft Web connects the shared Classic interface, tool state machines,
transactional document session and raster compositor. Pencil, round/square brush,
eraser, fill, line/rectangle/ellipse, rectangular selection, clear canvas and
undo/redo edit real pixels. Shift constrains shapes; Ctrl/Cmd-scroll zooms. The
Layers panel adds, selects, hides, reorders and deletes real layers. Brush preview
and release use the same shared commands. Erasing restores the white Classic ground.

This is an early editor. Files, editable text, image transforms, clipboard and
recovery are not connected yet. Unsupported actions report this in the status bar.
New requires explicit discard for dirty work, and the page requests the browser's
unsaved-work confirmation before unloading. Browser policy controls whether that
confirmation appears; it is not recovery or persistence.

The current synchronous browser adapter admits at most 4,194,304 canvas pixels,
8192 points per stroke and a 64 MiB history retention budget (older entries may
be trimmed). It cancels oversized strokes rather than silently cutting them off.
Large-document workers/cancellation and physical memory/latency qualification
remain work. Browser history uses raw snapshots; native Zstandard codecs remain
native-only, while raw/LZ4 support compiles to WebAssembly. WebRunner presents the
interface through wgpu, with WebGPU and WebGL enabled; document composition is
currently CPU, not qualified GPU document rendering. No native services are used.

## Run locally

Use the pinned repository toolchain, then install the matching binding generator:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
python3 apps/paintcraft-web/build.py
python3 -m http.server 8080 --directory apps/paintcraft-web/dist
```

Open `http://localhost:8080`. Serve the generated files over HTTP, not `file://`.
WebGPU typically requires localhost or HTTPS; WebGL availability depends on the
browser and device. Initialization failures are displayed on the page. Generated
files stay in the app-local ignored `dist/` directory. `cargo run` still starts
the desktop app by default.

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
This single-browser smoke check does not establish cross-browser conformance.

## Next browser slices

1. Add browser file picking and downloads, plus an explicit unsaved-work policy.
   Browser storage and permission behavior must remain separate from native rfd
   dialogs and desktop atomic saves.
2. Add browser workers, memory admission and cancellation before large documents.
3. Connect document GPU composition and qualify WebGPU/WebGL behavior, touch,
   keyboard, browser zoom and pen pressure across actual browsers/devices.

Desktop tablet support is also incomplete: `PenSample` defines pressure and tilt,
but the canvas currently consumes ordinary pointer events, without native pen
sample acquisition or pressure/tilt propagation into brush rendering.
