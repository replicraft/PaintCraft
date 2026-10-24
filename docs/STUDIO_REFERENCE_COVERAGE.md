# Studio reference coverage

The user-supplied `docs/temp/paint-references/PLAN.md` guides the next UI slices.
The original 15 supplied images were inspected on 2026-10-19.
Seven additional Classic photos were inspected later that day; see
[the Classic inventory and current priority](CLASSIC.md). The pack stays in its
reference directory; this inventory records relevant observations and implemented
coverage, rather than copying the specification or proprietary artwork.

## Screenshot inventory

Paths below are relative to `docs/temp/paint-references/photos/`.

| Screenshot | Observed interface |
|---|---|
| `im5age.png` | Paint.NET 5.1.12 workspace: seven menus, thumbnail switcher, options, four floating windows, status/zoom |
| `ima5ge.png` | Studio File menu, including Save All, acquisition, print and close |
| `imag5e.png` | Studio Edit menu: clipboard variants and selection operations |
| `5image.png` | Studio View menu: fit/selection zoom, pixel grid, rulers and units |
| `i5mage.png` | Studio Image menu: crop, resize, canvas size, transforms, profile and flatten |
| `ima5ge5.png` | Studio Layers menu: creation, duplication, merge, transforms, navigation and properties |
| `i5mag5e.png` | Thirteen Studio adjustments and their shortcut labels |
| `im5a5ge.png` | Nine effect categories; individual submenu contents are not shown |
| `5i5mage.png` | Settings sidebar and User Interface controls |
| `5im5a5g5e.png` | Two-column Studio tool window with selection, navigation, painting and drawing tools |
| `image.png` | Modern Paint workspace: grouped ribbon, palette, brush size/opacity and canvas |
| `im1age.png` | Modern Paint File menu, including import, project save and OS integrations |
| `imag1e.png` | Modern Paint Edit menu and copy-visible-layers action |
| `ima2ge.png` | Modern Paint View menu, toolbar behavior, rulers/grid and fullscreen |
| `mspaint-text-settings.png` | Additional Classic text toolbar: family/size, B/I/U/S, alignment, background fill; inspected 2026-10-19 |
| `im2age.png` | Scrolled modern Paint shapes gallery |

The version string in `im5age.png` establishes the visible Paint.NET build.
The supplied images establish layout and visible actions, not pixel-processing
semantics or an independently executable behavioral oracle. The modern Paint
build number is not visible. Pinned executable captures and platform-specific
behavior remain unqualified.

The separately supplied generated icon sheet has been adapted into 68 standalone
SVGs shared by Classic and Studio. See [the icon gallery and provenance](../crates/presentation/paintcraft-ui/assets/icons/README.md). It supplies project artwork, not
independent behavioral reference evidence.

## Current coverage and next slices

| Scope | Status | Working behavior / remaining scope |
|---|---|---|
| Shared engine and CLI | partially working | Typed atomic edits, PCND round trips, CPU/GPU composition and batch-v1 oracle work; richer operations remain in the main roadmap |
| Studio workspace | partially working | Separate frontend, toolbar/options, floating Tools/Colors/History/Layers and centered canvas; reference spacing, theme and several tools remain |
| Document switching/close | working | Independent frontend document workspaces with save-gated switching; sessions, histories, view/tool state, owner-routed saves/recovery and save/discard/cancel document close; text tabs rather than reference thumbnails |
| History | working | Recorded labels, undo/redo and guarded direct navigation; branching/spill is separate work |
| File workflows | partially working | New/Open/Save/Save As/Export and native dialogs work; shared metadata and menu/shortcut dispatch cover existing operations; Save All and desktop recent files now work; native macOS Print is now connected; other-platform printing and acquisition remain |
| Colors and layers | partially working | Primary/secondary swatches, swap, wheel, palette and More RGBA/hex controls work; layer selection/visibility/rename/opacity/reorder/add/delete/duplicate and staged properties work; custom palettes, thumbnails, merge/transforms and broader blend modes remain |
| View menu | partially working | Zoom in/out, actual size, fit window/selection and a display-only pixel grid work; pan, temporary Space/middle-button navigation, keyboard panning, overscroll and drag-edge auto-scroll also work; rulers, units and native/reference qualification remain |
| Settings | partially working | Light/Dark/System with live preview and Apply/Cancel, experience/version and canvas defaults; text prototype preference; utility-window size persistence and remaining reference pages pending |
| Selection/clipboard/image/adjustments/effects | partially working | Rectangle selection, crop and exact 90°/180° rotations/flips work; richer selections, clipboard, custom-angle/resampling/skew transforms, adjustment/effect registries and preview lifecycles remain |
| Classic v11 fidelity | in progress | Subpixel brush input and percentage zoom controls work; staged RGB/HSL color picker and persistent custom palettes work; full ribbon and remaining editing workflows remain |

Current user priority is completing missing editing features before UI cleanup, then focusing on Studio. See [Classic UI scope](CLASSIC.md).

Finish one functional slice at a time. Shared action descriptors and menu/shortcut equivalence now cover existing
operations, respecting text/IME and modal ownership. Subsequent slices add missing
Studio editing operations before
exposing their menu entries. Every new operation needs engine/history tests and
actual UI dispatch; visible placeholder menus do not count as completed features.

## Evidence

Desktop controller and executable egui tests cover independent document histories,
mode switching, native-completion ownership, same-destination FIFO writes,
background checkpoint admission, modal cancellation, close failures and the empty
workspace. See [Studio](STUDIO.md) for the save/recovery ownership contract.
`tabs::render_document_tabs_for_review` writes a fixed-size native-GPU review
capture to `/tmp/paintcraft-studio-tabs.png`. Its text tabs and current widget styling
are visibly different from the supplied reference. This is implementation review
evidence, not visual or behavioral parity. Full roadmap gates remain open in
[the implementation ledger](ROADMAP_STATUS.md).
