# Screenshots

These are actual PaintCraft desktop UI frames, rendered offscreen through the
application's egui/Wgpu test renderer. They are not concept art or screenshots of
Microsoft Paint. Window borders and the OS menu bar are outside this renderer.

The captures open the same project. The mountain illustration was built with
PaintCraft's real shape, contour and oil-brush commands and saved/reopened as a
PCND project. All four captures use English. The Windows 11 skin uses the dark
theme. The Windows 7/10 captures
use the light theme and their own historical ribbon controls. `studio.png` opens
the same illustration in Studio with its dark theme, document tab and floating
Tools, Colors, Layers and History panels. The export verifies that every capture
opens the same saved document and selects the requested interface.

Regenerate explicitly with GPU access:

```sh
PAINTCRAFT_SCREENSHOT_DIR="$PWD/assets/repo" \
  cargo test -p paintcraft --test gui export_readme_screenshots -- --ignored
```

Screenshots are committed in [`assets/repo`](../../assets/repo).
