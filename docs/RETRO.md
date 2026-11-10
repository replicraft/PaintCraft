# PaintCraft Retro

Retro is a planned frontend; implementation remains deferred under roadmap T23.

## Skins

Call the different historical presentations **skins**, consistently with Classic.
Retro must offer skins for various older versions of Microsoft Paint and a skin
for **Deluxe Paint II Enhanced 2.0**. The exact Microsoft Paint editions and their
reference coverage will be recorded before each skin is implemented.

Skins cover the application chrome, palettes, icons, menus and tool presentation.
Edition-specific tool behavior and format support need their own documented
capability choices and fixtures; a visual skin alone does not establish parity.
Keep the recognizable layout and interactions of each reference application.

The selected Retro UI font is [W95FA](https://www.dafont.com/w95fa.font).
The shared regular OpenType asset, original notices and provenance are in
[`assets/fonts/w95fa`](../assets/fonts/w95fa/README.md). It is registered as a
separate Retro font family with language fallbacks; frontend implementation
remains pending. Font choices for individual skins should follow their references,
so this default does not require the Deluxe Paint skin to use it.

## Delivery scope

These are requirements, not implemented features. Capture reference evidence for
each skin, verify readable layouts and palettes at supported display scales, and
record any deliberate compatibility differences. Classic and Studio work remains
the current implementation priority.

## Initial format profile and CLI

The explicit `retro-win95` format policy is available to CLI commands that accept
`--frontend`, while the Retro graphical frontend remains planned. Its initial
inventory is editable PCND projects and BMP/PNG raster exchange. PNG is a modern
PaintCraft convenience, not a claim about original Windows 95 Paint support.
Classic and Studio inventories and the CLI's default Studio profile are unchanged.

```sh
cargo run -p paintcraft-cli -- capabilities --frontend retro-win95
cargo run -p paintcraft-cli -- import picture.png --frontend retro-win95 -o picture.pcnd
cargo run -p paintcraft-cli -- render picture.pcnd --frontend retro-win95 -o picture.bmp
```

Existing conversion-loss and metadata consent still apply where needed. There is
no extra PNG warning. Foreign projects, JPEG/GIF/TIFF, historical PCX/MSP and other
advanced formats are refused by this initial profile even when another interface
supports their codecs. New/apply native-project commands retain their existing
engine contract; this profile does not add a new graphical interface or historical
rasterization, brush or original-application compatibility claim.
