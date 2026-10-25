# Browser and desktop parity

Full desktop parity is still in progress. The shared editor and Classic frontend
cover many operations, but the browser now exposes separate Classic and Studio document
workspaces. This ledger distinguishes connected features from remaining work.

| Area | Browser state | Remaining work |
| --- | --- | --- |
| Settings presentation | Same dialog shell, responsive navigation, cards, control sizes, close button and footer as desktop | Studio-specific canvas/navigation preference parity |
| Settings behavior | Appearance, system/light/dark theme, seven languages, Classic skins, canvas defaults, default font and About; explicit Apply/Cancel and reversible preview | Installed-font access needs a browser permission/backend; the editable-object preference remains a labelled prototype on both hosts |
| Document identity | Open shows the photo/project name in the tab title; requested project downloads use the same basename and update that title; New restores Untitled | The browser cannot observe a name changed in the browser's own download destination UI |
| Classic editing | Shared nine brush media, 23 shapes, fill, picker, text rasterization, masks, lasso, crop, five orientations and Undo/Redo | Arbitrary-angle/resampled transformations, effects and persistent editable objects remain broader roadmap work |
| Clipboard | PNG read/write, Copy/Cut/Copy visible and private ordinary Paste/new-layer Paste | Other browser/device permissions and cross-application fidelity remain unqualified |
| View and workspace controls | Canvas fit/actual/selection zoom, scroll, layers, tool controls, shared pixel grid, Classic rulers, persistent status/toolbar visibility and live thumbnail | Fullscreen and broader Studio navigation parity remain open |
| Frontends and documents | Classic 7, 10 and 11 skins; Studio toolbar, floating tools/colors and history, separate documents/history/views with a download/discard switch guard | Multiple documents within each workspace, tabs, Save All and remaining Studio navigation/parity workflows |
| File adapters | Browser pickers, lossless compact PCND downloads, PNG/BMP/JPEG input and image export | Broader native codecs, format capabilities and export controls |
| Runtime and recovery | Bounded synchronous browser editing; unload warning; preferences in local storage | Background preparation, larger document/resource handling, browser recovery/document persistence and GPU document composition |
| Platform services | Browser file/clipboard interfaces and page title | Browser equivalents for applicable desktop services such as Print; native menu/OS registration stay platform-specific |

## Settings implementation and validation

`paintcraft-classic::settings` owns presentation used by both native and web
hosts. Each host retains its own settings storage and adoption rules. Browser
preferences remain compatible with existing v1 data; the new text prototype flag
is optional on old records. Canvas defaults are validated against the browser
pixel limit before adoption. Changes do not publish document history or pixels.

Host tests exercise reversible theme/language preview, Apply, system theme,
all Classic skins, compact layouts, staged canvas defaults, invalid dimensions,
text preferences and About. Seven-language UI checks pass. Existing native
settings, canvas, Studio navigation and Japanese preview regression tests pass.

A production build was exercised in isolated HeadlessChrome 156.0.8078.4. Light
and dark Appearance, Text and About surfaces were inspected. Dark theme persisted
through a real reload. The actual Open shortcut and browser file picker loaded a
32x16 PNG and updated the tab title. Save downloaded the correctly named `.pcnd`
and updated the title; New restored `Untitled - PaintCraft`. The downloaded ZIP,
block hash and all 512 source RGBA pixels passed an independent check. No browser
exceptions were recorded. Artifacts are local in ignored
`docs/validation/web-settings-parity-2026-10-24/`.

This evidence covers these settings/identity workflows, not the unfinished parity
rows, other browsers/devices, native appearance on physical Windows, remote CI or
a Cloudflare deployment.

## Browser display controls

Pixel grid uses the same clipped overlay as desktop and never reads or changes
raster storage. Classic rulers use the shared scrolling canvas geometry. Status
bar and automatic toolbar visibility choices persist in browser preferences;
older preference records retain their original defaults. Grid and thumbnail
visibility belong to each frontend workspace. The thumbnail displays the current
prepared canvas texture, including private paint, shape and text previews.

Host tests exercise all three Classic skins in English and Japanese, actual menu
hide/reveal behavior, translated ruler/thumbnail labels, strict preference parsing,
and unchanged canonical document bytes/history tickets. The existing desktop
high-DPI grid regression also passes against the shared overlay. Production
headless-browser checks also verify actual reload restoration of rulers and
status/toolbar visibility. Runtime evidence is in ignored
`docs/validation/browser-view-controls-2026-10-24/`.
