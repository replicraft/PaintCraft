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

[gallery.html](gallery.html) displays the set from `generate_icons.py`; registry
additions and themed variants are documented below. [manifest.json](manifest.json)
maps sheet adaptations to their source row and column. Original Clear Canvas, Share, Edit Colors, Pointer, Stroke Size, Opacity, Remove Background, Classic shape-gallery and additional rotation icons
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
91 colored/chrome icons before the menu additions below; the same
vector/transparency tests cover the chrome additions.

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

Classic's 23 shape-gallery choices additionally use original monochrome outline
SVGs in [classic-shapes](classic-shapes/manifest.json), following the supplied
Windows Paint references. These have transparent interiors and are tinted at
paint time for each theme. They are separate variants of the existing icon IDs;
the 91 colored/chrome sources retain their original mapping. The shared cache
keys include the variant so colored and outline textures cannot alias.
Regenerate the outline sources with `python3 scripts/generate_classic_shape_icons.py`.
These vectors are project-authored adaptations of the visible shape vocabulary,
not extracted native assets or a claim of exact Windows rendering parity.

Classic v11 menus and primary/shape-paint controls additionally opt into 40 original tintable outline sources in
[classic-menus](classic-menus/manifest.json). Regenerate them with
`python3 scripts/generate_classic_menu_icons.py`. The menu helper's default stays
colorful; its Classic option preserves shared action metadata/gates and uses
theme-tinted menu variants. Unmapped icons retain their original artwork. The
same bounded cache keeps colored, shape-outline and menu-outline identities
separate. These are original PaintCraft glyphs following the supplied references,
not extracted Windows assets or a pixel-exact compatibility claim.

Import image, Recent, Desktop background, Image properties and Thumbnail have
dedicated semantic icon IDs and original monochrome artwork. Their default
source is the same SVG as their Classic menu variant; they are not sheet
adaptations. Together with the selection IDs below, the registry contains 98 IDs, retaining
all 91 prior sources, with 23 shape variants and 40 menu/control sources. The generated menu manifest owns
these five additional sources.

Classic's Image ribbon and transform dropdowns use nine original glyphs with
paired light/dark sources in [classic-image](classic-image/manifest.json).
These retain blue accents while changing the outline ink for the current theme.
Regenerate all 18 SVGs and their manifest with
`python3 scripts/generate_classic_image_icons.py`. They are variants of existing
IDs; colored, shape, menu and Image light/dark textures have independent cache
identities under the same bounds. They are original artwork following the
supplied Image group, not extracted Windows assets or exact native glyphs.

The Shape outline and Shape fill triggers use separate monochrome pen-over-box
and bucket SVG variants for the existing Pencil/Bucket IDs. Colored tool art
retains its original mapping. These dropdown glyphs render at an explicit
22-point size and use the same variant-aware cache and theme tint as menu art.

Classic v11's primary Brush uses an original vertical outline brush variant,
while Select uses the existing dashed rectangle variant. The source sheet
versions stay available to Studio and historical toolbars. Gallery strokes are
separate deterministic UI illustrations; they do not implement the previewed
brush media.

Select all and Invert selection have dedicated semantic IDs and original grid
artwork; Free-form and Delete have separate monochrome variants of existing
LassoSelect/DeleteLayer IDs. The Selection popup uses these SVGs, while default
colored action mapping remains available to other frontends. The contact-sheet
review now sizes its rows from the full registry, normalizes previews to a
64-pixel square, and places default monochrome glyphs on dark tiles so all 98
default sources are visible. It previously clipped entries beyond 72.
