# PaintCraft

A familiar place to draw, paint and edit images. Built in Rust.

- Windows 7, 10 and 11 Classic skins.
- Studio with document tabs, Save All and floating panels.
- Desktop, browser and command-line apps.
- **MCP support:** let AI assistants paint, edit layers, preview images and save projects.
- Nine brushes, pencil, eraser, fill and color picker.
- Shapes, curves and text.
- Rectangular and free-form selections, copy and cut.
- Paste images onto the canvas or into new layers (macOS and supported browsers).
- Layers, flattening, undo/redo, crop, rotate and flip.
- Recent files with quick reopening.
- Native printing on macOS.
- Light and dark themes in seven languages.

| Classic, Windows 11 | Studio |
| --- | --- |
| ![PaintCraft Classic, Windows 11](assets/repo/classic-windows-11.png) | ![PaintCraft Studio](assets/repo/studio.png) |

| Classic, Windows 10 | Classic, Windows 7 |
| --- | --- |
| ![PaintCraft Classic, Windows 10](assets/repo/classic-windows-10.png) | ![PaintCraft Classic, Windows 7](assets/repo/classic-windows-7.png) |

## Paint with MCP

Connect your MCP client to PaintCraft to create canvases, paint with brushes and shapes, edit layers, undo changes, import pictures and save or export the result.

```sh
cargo run --release -p paintcraft-cli -- mcp
```

[Set up your MCP client](docs/MCP.md). Each connection works on its own headless document.

## File formats

**PCND (`.pcnd`)** is PaintCraft's compact, lossless project format. It keeps layers, transparency and canvas size intact, while being compressed and lightweight. Save as PCND to keep working on your project.

- **Classic and Studio:** open and export PNG, JPEG, BMP, still GIF and single-page TIFF.
- **Studio:** also open and export TGA, still WebP and supported single-surface DDS images.
- **Studio exports:** lossless JPEG XL, lossy AVIF, QOI, PAM and farbfeld. JPEG XL and AVIF opening is not available yet.
- **Extra imports:** QOI, Netpbm, farbfeld and PCX, plus an explicitly selected DCX page in Studio. Classic also imports PCX.
- **Layered imports:** Studio opens a limited historical Paint.NET (`.pdn`) subset as converted copies. Studio and Classic 11 can import a limited Windows Paint (`.paint`) subset. Save converted projects as PCND; writing PDN or PAINT is not supported.

Image exports flatten layers. Conversion prompts explain supported losses such as reduced color depth, palette changes or discarded metadata. See [format support and limits](docs/FORMAT_INTEROPERABILITY.md) for the supported subsets.

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
