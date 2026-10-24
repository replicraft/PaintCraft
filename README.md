# PaintCraft

A familiar place to draw, paint and edit images. Built in Rust.

- Classic with Windows 7, 10 and 11 skins.
- Studio with layers, document tabs, Save All and floating tool panels.
- Pencil, eraser, fill, color picker and nine brush styles.
- Shapes, curves and text.
- Rectangular and free-form selections, inversion and delete.
- Crop, rotate and flip.
- Undo, redo, editable projects and PNG, BMP and JPEG images.
- Desktop recent files with one-click reopening.
- Light and dark themes in seven languages.
- Desktop, browser and command-line apps.

![PaintCraft Classic, Windows 11](assets/repo/classic-windows-11.png)

![PaintCraft Classic in Japanese](assets/repo/classic-japanese.png)

## Run

Desktop:

```sh
cargo run --release -p paintcraft
```

Command line:

```sh
cargo run --release -p paintcraft-cli -- --help
```

Browser:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
python3 apps/paintcraft-web/build.py
python3 -m http.server 8088 --directory apps/paintcraft-web/dist
```

Open [localhost:8088](http://localhost:8088).
