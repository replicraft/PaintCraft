# PaintCraft Retro

Retro is PaintCraft's historical interface. Its first implemented skin is
**Win95-inspired**, available on desktop and in the browser. Choose
**Interface → Retro**; use the same **Interface** control to return to Classic or Studio.

Each interface keeps its own documents, undo history, tools and view. When an
interface has changed documents, the switch asks you to save or discard them, or
cancel. In the browser, downloading a project copy does not confirm a disk save;
you must still choose explicitly whether to discard before switching.

## Win95-inspired controls

The left toolbox has sixteen choices: free-form and rectangular selection,
eraser, fill, color picker, magnifier, pencil, brush, airbrush, text, line, curve,
rectangle, polygon, ellipse and rounded rectangle. Brush and Airbrush are
distinct choices over the shared Brush tool, selecting Plain or Airbrush media.
The selection choices select rectangular or free-form coverage; these choices do
not imply separate per-button stored option presets.

Choose colors from the two-row palette. Left-click sets Color 1; right-click sets
Color 2. The current color swatches also select which color to edit. Double-click
a palette color to open its color dialog. Choosing colors does not change the
image or its undo history.

**View** shows or hides Tools, Colors, Status bar and Text toolbar. It also offers
real zoom controls, Fit window, Fit selection and Pixel grid. Scroll the canvas,
drag with the middle button, or hold Space to pan. Text uses the document's font
settings; the small historical UI font does not replace fonts in your image.

**Image** offers flip, rotate, crop, clear and image properties. Canvas-size and
color dialogs keep their edits private until you accept them; Cancel leaves the
image unchanged. **Help** opens documentation and About. The separate
**Settings** and **Interface** controls open settings and the interface chooser.
Recent files and Exit are desktop choices; the browser keeps its ordinary
download and tab behavior.

**Image → Resize and skew** applies nearest-neighbor stretch followed by
horizontal and vertical skew as one undoable edit. Without a selection, it
transforms every layer, including hidden/off-canvas pixels and stored own-layer
and filter masks. With a selection, it transforms only selected pixels on the
active layer, preserving holes and unrelated pixels; the canvas stays unchanged.

**Cancel** leaves the image unchanged. A changed source owner, revision or target
refuses a stale dialog/queued request, and a resource-limit failure does not
publish a partial transform. Editable filters remain editable; stored coordinate
Noise retains its explicit-bake requirement. See [resize/skew details](RESIZE_SKEW.md)
for sampling, padding and limits.

## Projects and image files

Use **PCND** to save an editable project. The initial Retro profile opens and
exports **BMP** and **PNG** pictures. PNG is a modern PaintCraft convenience,
not a claim about original Windows 95 Paint support.

Other interfaces retain their existing formats. This Retro profile refuses
foreign projects, JPEG/GIF/TIFF, historical PCX/MSP and advanced formats even when
another interface supports them. Existing conversion-loss and metadata choices
still apply where needed; PNG does not get an extra compatibility warning.

The same explicit profile is available in CLI commands that accept `--frontend`:

```sh
cargo run -p paintcraft-cli -- capabilities --frontend retro-win95
cargo run -p paintcraft-cli -- import picture.png --frontend retro-win95 -o picture.pcnd
cargo run -p paintcraft-cli -- render picture.pcnd --frontend retro-win95 -o picture.bmp
```

## Skins and compatibility

Call the different historical presentations **skins**, consistently with Classic.
Other older Microsoft Paint skins and **Deluxe Paint II Enhanced 2.0** remain
required future work. Each needs its own reference coverage, tool behavior and
format choices; the Win95-inspired skin does not complete those skins or the
whole Retro roadmap.

This first skin follows the recognizable menus, toolbox and palette of classic
Paint, with PaintCraft's working editor underneath. It does not claim pixel-exact
Windows 95/98 behavior, historical brush rendering, indexed-color documents or
complete original-application compatibility.

The UI uses [W95FA](https://www.dafont.com/w95fa.font), registered separately with
language fallbacks. The asset, original notices and provenance are in
[`assets/fonts/w95fa`](../assets/fonts/w95fa/README.md). Other skins should choose
fonts from their own references; Deluxe Paint need not use this font.
