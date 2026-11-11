# Color Overlay commands and copies

The native CLI and headless MCP session use the shared atomic layer style
commands. These operate on ordered, layer-local style instances, independently
from filters and layer blend modes. Color Overlay uses RGB bytes, byte opacity,
and an effect blend mode (`normal`, `multiply` or `screen`). Its effect changes
post-filter RGB; source alpha stays unchanged. The own-layer mask, layer opacity
and layer blend follow the complete style stack.

Discover the closed grammar with `paintcraft-cli schema batch-layer-styles` or
MCP `get_layer_style_schema` with `{"kind":"batch"}`. Other kinds are `manifest`,
`result`, `file-command`, `inspect`, `mcp-inspect` and `export-disclosure`.
Existing schema aliases keep their old meanings.

The explicit profile is `paintcraft.batch/layer-styles-v1`. It requires
`required_filters` plus an exact sorted `required_layer_features` inventory for
the operations in that batch. A style operation declares `layer-styles.v1`;
mask/blend operations in the same batch also declare their own features. MCP
`apply_operations` captures the current session ticket and supplies these
inventories through the same strict typed parser. Direct CLI batch files must
supply them explicitly.

The operations are `layer.style.add`, `.update`, `.set_enabled`, `.move`,
`.duplicate` and `.remove`. Each has `layer_ref`; existing instances use
`style_ref`. References contain exactly one UUID `id` or current-batch alias
`ref`. Add/Duplicate create a new alias with `ref`; Add/Update use the descriptor:

```json
{"kind":"color-overlay-rgb-v1","color":[220,40,0],"opacity_u8":128,"effect_blend_mode":"normal"}
```

Add defaults `enabled` to true. Add/Duplicate may provide `to_index`; Move
requires a final `to_index`. Disabled and zero-opacity entries remain stored
intent. Explicit Duplicate mints a fresh deterministic layer-scoped style ID;
duplicating a layer preserves its own style IDs. Edits preserve source pixels
and are undoable. No-op results and last-style removal still use the explicit
style result profile and include `style_aliases`.

Nonempty stacks save as PCND9, including disabled and zero-opacity entries.
Removing the last entry allows the minimum remaining project version again.
Style-aware inspect results list every layer's ordered styles and blend mode.
The old blend-only PCND8 inspect/MCP profile remains closed. This extension
adds no browser or GPU style-rendering route.

Styled foreign copies currently require PNG and explicit compound consent:

```sh
paintcraft-cli render styled.pcnd -o copy.png --appearance-copy bake \
    --allow-loss styles-baked,filters-baked,layer-appearance-baked,flatten --json
```

Acknowledge only the losses actually reported for the source. `styles-baked`
is always required for a styled PNG; `filters-baked` is also required when
filters exist, and `layer-appearance-baked` when filter masks, own masks or
non-Normal layer blends exist. Existing flatten/metadata/alpha policies remain
independent. MCP uses `appearance_copy:"bake"` and `allow_losses:[...]`.
`appearance_copy` defaults to `refuse`; loss names alone do not grant it.
The existing raw `--allow-loss filters-baked` authority for unstyled filter
copies keeps its original behavior.

A successful styled copy reports the actual Formats compound inventory in
`engine`, using `paintcraft.export-disclosure/layer-styles-v1` inside the new
file-command profile. It identifies the captured source state, target, inventory
digest and stored counts, and which appearance components were baked. These
values describe the operation; they are not reusable export authority. The
source project and session dirty state are unchanged by exporting.
