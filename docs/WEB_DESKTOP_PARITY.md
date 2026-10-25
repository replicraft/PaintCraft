# Browser and desktop parity

Full desktop parity is still in progress. The shared editor and Classic frontend
cover many operations, but the browser now exposes separate Classic and Studio document
workspaces. This ledger distinguishes connected features from remaining work.

| Area | Browser state | Remaining work |
| --- | --- | --- |
| Settings presentation | Same dialog shell, responsive navigation, cards, control sizes, close button and footer as desktop | Studio-specific canvas/navigation preference parity |
| Settings behavior | Appearance, system/light/dark theme, seven languages, Classic skins, canvas defaults, default font and About; staged Apply/Cancel for theme, language and skin | Installed-font enumeration and rendering use permission-gated Local Font Access in supporting desktop browsers; bundled fonts remain available elsewhere. The editable-object preference remains a labelled prototype on both hosts |
| Document identity | Open shows the photo/project name in the tab title; requested project downloads use the same basename and update that title; New restores Untitled | The browser cannot observe a name changed in the browser's own download destination UI |
| Classic editing | Shared nine brush media, 23 shapes, fill, picker, text rasterization, masks, lasso, crop, five orientations and Undo/Redo | Arbitrary-angle/resampled transformations, effects and persistent editable objects remain broader roadmap work |
| Clipboard | PNG read/write, Copy/Cut/Copy visible and private ordinary Paste/new-layer Paste | Other browser/device permissions and cross-application fidelity remain unqualified |
| View and workspace controls | Canvas fit/actual/selection zoom, scroll, layers, tool controls, shared pixel grid, Classic rulers, persistent status/toolbar visibility, live thumbnail and browser fullscreen | Broader Studio navigation parity remains open |
| Frontends and documents | Classic 7, 10 and 11 skins; Studio toolbar, floating tools/colors and history, separate documents/history/views with a download/discard switch guard | Multiple documents within each workspace, tabs, Save All and remaining Studio navigation/parity workflows |
| File adapters | Browser pickers, lossless compact PCND downloads, PNG/BMP/JPEG input and image export | Broader native codecs, format capabilities and export controls |
| Runtime and recovery | Bounded synchronous browser editing; unload warning; preferences in local storage | Background preparation, larger document/resource handling, browser recovery/document persistence and GPU document composition |
| Platform services | Browser file/clipboard interfaces, page title and private-image Print | Physical printing and broader browser/device qualification; native menu/OS registration stay platform-specific |

## Settings implementation and validation

`paintcraft-classic::settings` owns presentation used by both native and web
hosts. Each host retains its own settings storage and adoption rules. Browser
preferences remain compatible with existing v1 data; the new text prototype flag
is optional on old records. Canvas defaults are validated against the browser
pixel limit before adoption. Changes do not publish document history or pixels.

Host tests exercise staged theme/language/skin choices, Apply, system theme,
all Classic skins, compact layouts, staged canvas defaults, invalid dimensions,
text preferences and About. Seven-language UI checks pass. Existing native
settings, canvas, Studio navigation and Japanese Apply/Cancel regression tests pass.

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

## Browser fullscreen

Full screen uses the browser's actual DOM fullscreen state. The View menu and
F11 request entry/exit, Escape provides an application exit fallback, and
fullscreen-change events refresh the checkmark after an external exit. F11's
browser default is prevented so the browser's separate window-fullscreen action
cannot also run. Event subscriptions are removed when the editor is destroyed;
there is no fullscreen polling timer or stored fullscreen preference.

Requests await the native promises through a small helper in `src/lib`. A denied
or unavailable request leaves the actual mode unchanged and reports a localized
status in all seven languages. Host tests preserve the document/history ticket;
helper tests cover pending requests, entry/exit, denial and unavailable support.
Dedicated headless Chromium production checks verify actual menu entry/exit,
F11 entry/exit with its browser default prevented, Escape exit, external exit,
menu checkmarks and caught request rejection without runtime exceptions. Evidence
is in ignored `docs/validation/browser-fullscreen-2026-10-24/`. This is scoped
Chromium evidence; broader browser/device qualification remains open.

## Narrow layouts, themes and project storage

Studio's browser Interface and Settings controls now occupy the upper right,
matching Classic and the desktop host. Settings use distinct Appearance,
Interface and About icons, 22-point section headings, and a GitHub link that opens
in a new tab. Paint 7 forces a light presentation in Classic while retaining the
preferred system/light/dark setting; leaving that skin resolves the retained
preference again. Studio remains free to use its preferred theme.

On narrow screens, Interface becomes an accessible icon button and the zoom row
fits the available width. All seven languages retain reachable header controls
at 320 × 568. The Classic canvas supports queued panning, and the browser maps
middle-button dragging, Space-dragging and two-finger touch movement to frontend
navigation. Space remains text input while using the Text tool. Touch gestures
beginning over another UI layer do not pan the canvas. Both canvas edges remain
reachable after shrinking a zoomed desktop window to phone width. A production
390-pixel-wide headless browser check also exercised actual two-finger input.
Physical touch hardware and broader browser support remain unqualified.

The browser offers a localized Download for desktop CTA, with a compact entry
inside Interface on small screens. It opens `/download` in a new tab. The page
detects desktop platforms and opens `replicraft/paintcraft/releases/latest` as the
requested placeholder; mobile and unknown devices retain manual platform links.
The page currently uses English copy. Individual installer asset URLs and actual
automatic file downloads are not connected yet.

Browser, desktop and CLI normal saves now use [compact binary PCND](PCND_STORAGE.md).
A production browser project was independently decoded and reopened through the
file picker, then saved byte-identically. This does not close the remaining
format breadth, native service, GPU composition or cross-browser roadmap gates.
Evidence is in ignored `docs/validation/ui-storage-polish-2026-10-24/`.

## Browser printing

File > Print and Ctrl/Cmd+P now capture the visible document with private draft
text, preserve dirty/history state and use a dedicated image-only print frame.
Waiting, cancellation and denial controls are translated. Actual production
Chromium lifecycle and independent PNG/PCND pixel checks pass; visible printer
dialogs, physical output and other browsers remain unqualified. See
[browser printing](BROWSER_PRINTING.md).

## Header, toolbar and mobile preferences

The desktop-download control appears only in Classic 11. Its target is vertically
centered beside Settings, with distinct hover, keyboard-focus and pressed colors.
At compact widths the same route remains in Interface. Classic 7/10 and Studio
omit it. Browser File menus omit Exit through an explicit host capability.

Auto-hide now retains the full revealed ribbon and text-options hover region.
Moving from the menu bar into a tool or font control keeps it visible; pointer
down/up frames still hold the canvas mapping. Settings > Appearance > Canvas
controls offers **Show brush size and opacity bars** on desktop and web. The
choice stays staged until Apply, persists across restart/reload, and removes the
floating controls' canvas clearance when disabled. Tool values and history stay
unchanged. Old preference records default to showing the bars.

Theme, language and skin choices now remain staged until Apply. Paint 7's forced
light theme begins with its applied skin, and the saved theme returns only after
another skin is applied. Cancel leaves the active UI unchanged.

The follow-up passes 137 browser host tests, 49 shared UI tests (one capture
ignored), 362 desktop library tests (8 ignored) and 280 desktop GUI tests
(64 explicit captures ignored). Strict host/Wasm Clippy, Rust formatting, source
length checks and all 978 messages in seven locales pass. The production build
contains 22 assets below the per-file Pages limit. Isolated Chromium checks cover
actual hover/layout, staged Paint 7/11 changes, toolbar traversal, browser menus
and mobile bar hiding with real local-storage persistence and reload. Selecting
a skin before Apply leaves the rendered top bar byte-identical. Evidence is
local in ignored `docs/validation/browser-chrome-2026-10-25/`. This remains scoped
headless Chromium evidence rather than native Windows reference qualification.


## Settings typography and installed browser fonts

Ordinary settings cards use 18-point headings; About retains its 22-point heading.
Theme, skin and desktop interface radio rows use 20-point horizontal spacing and
10-point wrapped-row spacing. Apply/Cancel behavior stays staged.

Web Text preferences now offer **Load installed fonts**. The browser permission
is requested only from that button; previously granted access is restored on
startup. The family list merges installed and bundled fonts. Only the selected
installed family is read, including its style faces, with a 32 MiB face / 64 MiB
family limit. Font bytes stay in memory on the device and feed the same
Cosmic Text/Swash document renderer. Fonts arriving asynchronously invalidate
shaping, fallback masks and the canvas preview without publishing document edits.
Both independent workspaces share available fonts and keep their text state.

Arial is the default for new web preferences and old records without a font
choice. Explicit saved choices are preserved. Where local access is unsupported,
denied, or Arial is absent, the bundled collection supplies fallback glyphs;
Selawik remains interface typography. Local Font Access currently requires a
secure context and a supporting desktop browser, such as Chrome or Edge; Safari,
Firefox and mobile browsers retain bundled fonts. See the
[Local Font Access API documentation](https://developer.chrome.com/docs/capabilities/web-apis/local-fonts).

The follow-up passes 138 browser host tests, 49 shared UI tests (one capture
ignored), 10 desktop settings GUI tests (one capture ignored), and 17 JavaScript
tests. Host/Wasm Clippy, Rust/Biome formatting and the 800-line source limit pass.
The production build contains 22 assets, largest 14,251,426 bytes. Isolated
Chromium enumerated 184 actual local families, read only the selected Arial or
Courier New faces, rendered document text with those faces, and refreshed a
private preview after deliberately delayed font arrival. Denial and unsupported
API checks retain bundled fonts and leave a blank document clean. Evidence is
local in ignored `docs/validation/browser-local-fonts-2026-10-25/`; other browser
engines and physical platform rendering are not qualified by these checks.
