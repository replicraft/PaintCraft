# PaintCraft colorful vector icons

68 standalone SVG adaptations of the user-supplied generated artwork
`docs/temp/paint-references/Colorful Graphics Editor Icon Sheet.png`.
The user identified the sheet as generated and requested individual SVG versions.
Source SHA-256: `3d203f189a515cc15c073258131b4ba8e14e575b84b3dc14cdf26cf43d9d4dc9`.

These are hand-authored path/shape/gradient versions with transparent backgrounds,
64×64 view boxes, and no embedded raster images, external references or font
requirements. They retain the sheet's colorful outlined visual direction rather
than claiming a pixel-exact automatic trace. Names are PaintCraft's interpretation
of the sheet's visible glyphs. The source sheet is reference material; these SVGs
and their generator are project assets under the repository license.

[gallery.html](gallery.html) displays the full set. [manifest.json](manifest.json)
maps sheet adaptations to their source row and column. Original Clear Canvas, Share, Edit Colors, Remove Background, Classic shape-gallery and additional rotation icons
use null source coordinates and are generated alongside the sheet icons. Regenerate the SVGs,
manifest and gallery with `python3 scripts/generate_icons.py` from the repository
root. The named Rust asset mapping lives in `src/icons/assets.rs`.

Shared tool and toolbar buttons use these SVGs in Classic and Studio. Implemented
menu actions use corresponding icons. The full set is available through
`paintcraft_ui::icons::Icon`; assets for unimplemented tools/effects are retained
with disabled previews for the current UI/theme pass.

Two additional project-authored SVGs, `settings.svg` and `layers.svg`, provide
application chrome. They are not adaptations of sheet cells and are intentionally
absent from the source manifest/generator. The Rust asset set contains
88 icons and the same vector/transparency tests cover the two additions.

The UI uses [resvg 0.48.1](https://docs.rs/resvg/0.48.1/resvg/) without default
features, rasterizing bundled sources at display density. Per-context derived
textures have a 64-entry/8 MiB LRU bound and are regenerated for density changes.
This is a correct baseline, not a measured latency or physical GPU-memory claim.
Tests parse and render every icon at 24, 64 and 128 pixels, check transparency and
reject raster/font/external-reference content. An ignored review test writes a
contact sheet:

```sh
cargo test -p paintcraft-ui icons::review::render_all_icons_for_review -- --ignored
```

Output: `/tmp/paintcraft-svg-icons.png`. Native GPU workspace captures separately
verify the widgets using the assets.
