# PaintCraft Web

The first browser slice is a **Classic UI preview**, using the same ribbon,
Selawik font, SVG artwork, canvas presentation and zoom controls as desktop.
It has its own frontend state and does not depend on the desktop application or
native platform services. WebRunner renders the interface through wgpu, with
WebGPU and WebGL browser backends enabled.

This is not a browser editor yet. The canvas is a blank 1000 × 486 preview.
Tool selection, ribbon dropdown options, palette swatches, zoom and theme/skin
settings are interactive. Drawing, layers, document commands, import, export,
saving, recovery and editable text are not connected. Unsupported shared actions
report that limitation in the status bar. Nothing is saved across page reloads.

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

## Next browser slices

1. Connect a bounded document/session and the shared tools, preserving identical
   preview and committed raster semantics. Avoid copying the desktop controller.
2. Add browser file picking and downloads, plus an explicit unsaved-work policy.
   Browser storage and permission behavior must remain separate from native rfd
   dialogs and desktop atomic saves.
3. Add browser workers, memory admission and cancellation before large documents.
4. Connect document GPU composition and qualify WebGPU/WebGL behavior, touch,
   keyboard, browser zoom and pen pressure across actual browsers/devices.

Desktop tablet support is also incomplete: `PenSample` defines pressure and tilt,
but the canvas currently consumes ordinary pointer events, without native pen
sample acquisition or pressure/tilt propagation into brush rendering.
