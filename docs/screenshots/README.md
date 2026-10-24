# Screenshots

These are actual PaintCraft desktop UI frames, rendered offscreen through the
application's egui/Wgpu test renderer. They are not concept art or screenshots of
Microsoft Paint. Window borders and the OS menu bar are outside this renderer.

Both captures open the same project. The mountain illustration was built with
PaintCraft's real shape, contour and oil-brush commands and saved/reopened as a
PCND project. `classic-windows-11.png` uses English and `classic-japanese.png`
uses Japanese. Both use the Windows 11 skin with the dark theme.

Regenerate explicitly with GPU access:

```sh
PAINTCRAFT_SCREENSHOT_DIR="$PWD/docs/screenshots" \
  cargo test -p paintcraft --test gui export_readme_screenshots -- --ignored
```
