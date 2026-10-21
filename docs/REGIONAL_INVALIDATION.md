# Derived regional dependency identities and output patches

Authoritative storage remains sparse, immutable/COW 256x256 RGBA8 tiles. Each tile
also holds an initial uniform color and sixteen derived 64x64 revision stamps.
Fresh uniform cells describe their actual initial RGBA, including hidden RGB;
missing tiles describe the layer default. A COW copy preserves all cell identities.
Known writes retire the intersecting cells before mutation. Admitted preparation
without pixel writes preserves them. Import and arbitrary recorded materialization
retire every cell; installed immutable leases carry their original identities.

Stamps are process-local atomic monotonic u64 values. They are not StateIds,
serialized pixels, content hashes or logical blob identities. They never wrap:
exhaustion makes affected tiles ineligible for caching, while ordinary rendering
and edits continue. Uniform cells retain their actual initial color even when a
lease is installed in a layer with another default. Tokens retain neither decoded
pixels nor weak control blocks. Existing whole-tile weak identities remain the
history/resident lease interface. New mutation paths must mark their complete
potential write area or invalidate the full tile before changing bytes.

## Ordered pointwise composition

Cache keys retain ordered relevant layers and every intersecting dependency cell,
including implicit defaults. Document/layer identity, dimensions, origin, opacity,
default RGBA and output rectangle remain required. Structural, visibility and
policy changes retain ordinary full/prefix recomposition. Current eligibility is
flat encoded-sRGB default-v1 RGBA8; masks, groups, effects and profiles need their
own dependency plans before caching them.

For a full-output miss with matching layer geometry/policies, the cache compares
region identities with a previous complete output. Changed source cells translate
through each layer origin into output coordinates. Overlapping damage is unioned
onto nonoverlapping 64-pixel cells relative to that output tile. Unaligned origins
can conservatively invalidate several cells. Each changed region is recomposed
from all current layers in original order; unchanged pixels come from a private
copy of the previous complete output. No blending is reassociated. Input/output
regions coincide for this pointwise compositor; effect halos/global dependencies
are not inferred from these cells.

A fully invalid tile uses ordinary prefix routing. Preview rendering can patch
from committed or prior preview output without altering the committed partition.
A cancelled/failed patch never installs partial pixels or a partial dependency key.
Completed tiles may warm normally; document/preview/device acceptance still governs
presentation. Undo/redo can patch back toward recorded versions without replaying
commands. Source re-decoding can conservatively give new identities and miss caches.

`RenderStats.patched_tiles` counts reused output tiles requiring changed-region
recomposition. `recomposed_tiles` includes patched and full misses;
`recomposed_pixels` counts the actual nonoverlapping recomposed cells. Full hits and
lower-prefix hits retain their separate counters. A single local pixel can now
recompose 4096 rather than 65536 pixels while keeping 256-pixel output tiles.

## Resource and verification scope

Each decoded tile gains a fixed sixteen-stamp array and initial-color field.
Raster metadata weights include resident Tile structs; shared snapshots may charge
those structs conservatively more than once. Decoded leases still count Vec payload
bytes alone. Keys charge their region-token Vec capacities, not the obsolete weak
control-block allowance. Patch work adds an independent full output-tile copy plus
bounded changed-region scratch; returned dense output and temporary key/patch
metadata remain separate from persistent cache budgets. These weights are not RSS
or a completed process-wide metadata/allocation budget.

Tests cover immutable/COW/default identities, foreign-default installed leases,
imports, raw restoration and revision saturation. Renderer tests compare exact
scalar output after local edits, origin shifts, intersecting boundaries, unusual
31/97/1024 grids, cancellation and recorded undo/redo. Existing structural,
clipping, hidden-RGBA, opacity, cache-budget and current-preview tests remain.
The 64-pixel dependency grid is a bounded candidate, not a claim of optimal size;
composite tile granularities are measured separately before changing desktop policy.
