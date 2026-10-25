# PaintCraft

A familiar place to draw, paint and edit images. Built in Rust.

- Windows 7, 10 and 11 Classic skins.
- Studio with document tabs, Save All and floating panels.
- Desktop, browser and command-line apps.
- Nine brushes, pencil, eraser, fill and color picker.
- Shapes, curves and text.
- Rectangular and free-form selections, copy and cut.
- Paste images onto the canvas or into new layers (macOS and supported browsers).
- Layers, undo/redo, crop, rotate and flip.
- Recent files with quick reopening.
- Native printing on macOS.
- Light and dark themes in seven languages.

| Classic, Windows 11 | Studio |
| --- | --- |
| ![PaintCraft Classic, Windows 11](assets/repo/classic-windows-11.png) | ![PaintCraft Studio](assets/repo/studio.png) |

| Classic, Windows 10 | Classic, Windows 7 |
| --- | --- |
| ![PaintCraft Classic, Windows 10](assets/repo/classic-windows-10.png) | ![PaintCraft Classic, Windows 7](assets/repo/classic-windows-7.png) |

## File formats

**PCND (`.pcnd`)** is PaintCraft's compact, lossless project format. It keeps layers, transparency and canvas size intact, while being compressed and lightweight. Save as PCND to keep working on your project.

- **PNG (`.png`):** open and export, with transparency.
- **JPEG (`.jpg`, `.jpeg`):** open and export, with adjustable export quality.
- **BMP (`.bmp`):** open and export.
- **GIF (`.gif`):** open still images.
- **TIFF (`.tif`, `.tiff`):** open single-page images.

Image exports flatten layers into one picture.

## Install

See [Releases](https://github.com/replicraft/PaintCraft/releases) for packaged downloads and installation notes. Choose the build for your operating system and processor. On macOS, open the DMG and drag PaintCraft to Applications. On Windows, use the MSI or extract the portable ZIP.

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
pnpm --dir apps/paintcraft-web install --frozen-lockfile
pnpm --dir apps/paintcraft-web run dev
```

Open [localhost:8088](http://localhost:8088).
