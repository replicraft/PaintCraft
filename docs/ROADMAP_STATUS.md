# PaintCraft roadmap implementation status

This is the live application ledger for the reference pack's 37 tasks. The
[roadmap](temp/paintcraft-implementation-spec-v5-hyperfine/ROADMAP.md) owns milestone
intent; [roadmap-status.json](roadmap-status.json) preserves task dependencies,
acceptance and evidence. A partial implementation does not complete its milestone.

Work proceeds on `main` in logical commits. CPU conformance, preservation, admission
and truthful file outcomes remain prerequisites. Reference builds, signing identities,
external-author trials and physical devices retain explicit evidence gaps while other
implementation work continues. The optional M5 products are separate roadmap tranches,
not implicit requirements to enable accounts or external services.

Latest Classic web controls connect staged primary/secondary/custom colors,
bounded canvas dimensions and Image properties, configured/remembered startup
sizes, persistent browser preferences and floating size/opacity controls. Shared
resize/default/ground logic keeps native semantics; eraser and Clear Canvas now
restore a loaded project's ground. Thirty web tests pass, alongside native
controller/settings/canvas regressions and strict workspace/wasm checks. An
isolated browser verified dimensions/Undo/reload, theme/skin/palette persistence,
color painting and malformed/denied preference storage. The final bundle was
rebuilt and started successfully. See [the controls evidence](validation/web-classic-controls-2026-10-23/validation.json).
Preferences contain no document pixels. Text, transforms, workers/recovery, actual
GPU document composition and cross-browser/native fidelity remain work. T15 stays
partial and the T22 conformance fixture/interruption gate stays not-started.

Latest web work replaces the blank preview with a bounded Classic editor: shared
document/session/tools, matching raster previews and commits, selection, real
layers and undo/redo. Browser picking opens PCND/PNG/BMP/JPEG; project downloads
and confirmed PNG flattening/export are connected. Downloads retain unsaved state,
and New/Open require explicit discard of dirty work. Native codec/filesystem
services are excluded from wasm. Thirteen web tests, 69 format tests (one ignored
helper), strict workspace/wasm checks and release bundling pass. Isolated headless
Brave verified drawing/release/undo pixels plus project download, PNG import and
project reopen. See [the browser editor evidence](validation/web-editor-2026-10-23/validation.json).
Editable text, transforms, browser workers/recovery and actual GPU document
composition remain unfinished. This single-browser smoke check does not implement
the native-equivalent fixture/interruption matrix: T22 stays not-started and T15
stays partial. Earlier preview-only entries below are historical.

Latest desktop Thumbnail work includes active paint, shapes and editable text
through immutable private raster preparation. Same-gesture older results are
marked updating; document/state/epoch/owner checks reject cancelled or superseded
drafts. Main-canvas shapes and bitmap text share their final raster commands.
254 desktop library tests and 212 ordinary GUI tests pass (8/45 ignored), with
strict workspace/wasm32 Clippy and four reviewed actual-App Wgpu scenes. See
[the live overview evidence](validation/classic-live-thumbnail-gpu-2026-10-23/validation.json).
T15 stays partial; logical admission is not a physical-memory/latency or native
rendering-parity qualification. The following records describe earlier slices.

Latest Classic View work enables the separate Thumbnail window for the complete
committed canvas. Its bounded worker/cache has source-checked publication,
pre-admitted output ownership and cancellation/drain tests. The movable/resizable
window clears floating tool controls, follows light/dark themes, captures pointer
input and preserves editable text. Live drafts remain unconnected. All 210 GUI
tests pass on the final source (44 opt-in tests ignored), together with worker,
renderer/shared UI/web tests and strict workspace Clippy. Three final Wgpu captures
were reviewed; see [the Thumbnail record](validation/classic-thumbnail-gpu-2026-10-23/validation.json).
T15 remains partial; no native parity, physical resource/latency or broader browser
conformance gate is inferred.

Latest Classic polish aligns Layers with the measured Colors row, reduces the
quick-bar height and menu gaps, and fills the zoom rail blue up to the thumb.
All 207 ordinary GUI tests pass; light/dark actual-App GPU captures were reviewed.
A separate `apps/paintcraft-web` WebAssembly **UI preview** now starts in a served
browser bundle with the shared Classic chrome and theme/skin/zoom controls.
Host UI tests, strict workspace/wasm32 Clippy and release bundling pass; real
headless Brave startup/settings/theme interactions were captured. Drawing and
browser document/file services are unconnected. Tablet pressure/tilt acquisition
and rendering are also unfinished. See [the scoped evidence](validation/classic-ribbon-web-preview-2026-10-23/validation.json)
and [web build/next slices](../apps/paintcraft-web/README.md). T15 stays partial;
T22 conformance remains not-started because its fixtures/interruption gates have
not been implemented by this presentation scaffold.

Latest Classic Properties work enables File → Image properties and Ctrl/Cmd+E,
with real undoable pixel dimensions and staged cancellation. Resolution/physical
units and monochrome conversion remain unavailable. Unchanged OK no longer
expands cropped layers or dirties clean files. All 207 ordinary GUI tests,
23 shared UI tests, targeted stale/native-menu checks and strict Clippy pass;
four committed-source GPU captures were reviewed. See
[the Image properties record](validation/image-properties-gpu-2026-10-23/validation.json).
Complete native Properties fidelity and T15 remain partial.

Latest Classic Image work adds an interactive Resize and skew **UI preview**,
using Microsoft's historical dialog layout, staged units/link/angle controls and
original SVG link artwork. Preview status stays visible and applying remains
disabled. All 203 ordinary GUI tests, 23 shared UI tests, stale-source rejection,
GPU scene export and strict workspace Clippy pass; see
[the Resize/skew preview record](validation/resize-skew-preview-gpu-2026-10-23/validation.json).
Transform/selection semantics and complete T15 acceptance remain incomplete.

Latest canvas-dialog work adds fixed header Close controls that discard staged
dimensions and startup policy. All 201 ordinary GUI tests, the actual-App GPU
exporter and strict workspace Clippy pass; four representative captures were
reviewed. See [the canvas Close record](validation/canvas-dialog-close-gpu-2026-10-23/validation.json).
Native Paint properties fidelity and broader T15 acceptance remain partial.

Latest Classic Layers form work aligns property fields, raises body text to 16
points, adds visible native form boundaries and keeps both editors above the
status bar with adaptive compact metrics. All 200 ordinary GUI tests, the
actual-App GPU exporter and strict workspace Clippy pass; four final captures
were reviewed. See [the layer form record](validation/classic-layer-forms-gpu-2026-10-23/validation.json).
Complete native panel fidelity and T15 remain partial.

Latest Classic Layers work replaces the blue selected-row fill with the neutral
Classic surface and accent outline, retaining thumbnails and native button input.
All 200 ordinary GUI tests, the actual-App GPU exporter (14 captures) and strict
workspace Clippy pass; see [the layer surface record](validation/classic-layer-surfaces-gpu-2026-10-23/validation.json).
Complete native panel fidelity and T15 remain partial.

Latest Classic Tools artwork restores reference color accents using paired light/dark
SVGs. All 199 ordinary GUI tests, 23 shared UI unit tests and strict workspace
Clippy pass; two native Wgpu framebuffer captures were reviewed. See
[the tool color record](validation/classic-tool-colors-native-2026-10-23/validation.json).
Exact Windows fidelity and broader T15 acceptance remain partial.

Latest Classic ribbon work adds the missing stroke-size dropdown, backed by
existing line/rectangle/ellipse size options, with an original SVG and adaptive
placement. All 199 ordinary GUI tests, 23 shared-UI unit tests, 18 actual-App GPU
scenes and strict workspace Clippy pass; see [the stroke-size record](validation/classic-stroke-size-gpu-2026-10-23/validation.json).
Exact native popup/raster qualification and broader T15 acceptance remain partial.

Latest Classic dimension-field work aligns captions and numeric columns across
Settings and canvas/default-size dialogs, including keyboard editing. All 197
ordinary GUI tests, 24 actual-App GPU scenes and strict workspace Clippy pass; see
[the dimension field record](validation/classic-dimension-fields-gpu-2026-10-23/validation.json).
Range/persistence/resize semantics remain; native fidelity and broader T15 gates remain partial.

Latest Classic form work makes radio/checkbox boundaries visible in Settings,
canvas-default choices and the text toolbar. Scoped native-shape checks cover
contrast, feedback and input without changing chrome or Studio styling. All 196
ordinary GUI tests, 16 actual-App GPU scenes and strict workspace Clippy pass; see
[the form boundary record](validation/classic-form-boundaries-gpu-2026-10-23/validation.json).
Full accessibility/native fidelity and broader T15 acceptance remain partial.

Latest Classic Settings work aligns dialog typography and targets, puts a cancelling
Close action at the upper right, and gives sidebar icons/labels consistent leading
alignment and neutral selected surfaces. All 195 ordinary GUI tests, 16 actual-App
GPU scenes and strict workspace Clippy pass; see [the Settings surface record](validation/classic-settings-surfaces-gpu-2026-10-23/validation.json).
Full native fidelity and broader T15 acceptance remain partial.

Latest Classic font-search work adds staged arrow/page navigation, Enter acceptance
and Escape cancellation, with virtualized candidate visibility and IME routing
guards. All 194 ordinary GUI tests, 16 component GPU scenes and strict workspace
Clippy pass; see [the keyboard font record](validation/font-keyboard-navigation-gpu-2026-10-23/validation.json).
Native IME/accessibility qualification and full UI fidelity remain partial.

Latest Classic popup work shares stable screen-relative bounds across scrolling
menus, reserves the main headings for menu switching and exposes overflow with a
resting scrollbar. All 191 ordinary GUI tests, 24 light/dark compact GPU scenes and
strict workspace Clippy pass; see [the compact popup record](validation/compact-popup-stability-gpu-2026-10-23/validation.json).
Full native fidelity and broader T15 acceptance remain partial.

Latest Classic brush-gallery work reveals the selected preset on opening, keeps
manual scrolling free, and fixes a shrinking popup on compact displays. A thin
resting scrollbar exposes overflow. All 190 ordinary GUI tests, eight GPU scenes
and strict workspace Clippy pass; see [the brush opening record](validation/brush-gallery-opening-gpu-2026-10-23/validation.json).
Broader media implementations and native fidelity remain partial.

Latest Classic palette work targets the empty custom-color slot that was clicked,
while preserving staged Cancel/OK preferences. All 189 ordinary GUI tests, 23
shared-UI unit tests and strict workspace Clippy pass. An isolated native Classic
launch also passed; see [the palette target record](validation/custom-palette-target-2026-10-23/validation.json).
Full native fidelity and broader T15 gates remain partial.

Latest Selection popup work keeps Rectangle marked independently of the active
drawing tool and uses a neutral selected row with a leading accent indicator.
All 188 ordinary UI tests, eight light/dark desktop/compact GPU scenes and strict
workspace Clippy pass; see
[the Selection row record](validation/selection-row-surfaces-gpu-2026-10-23/validation.json).
Existing selection capabilities and the broader incomplete fidelity gates remain.

Latest v11 Selection/Brush controls join their primary/menu targets under one
selected outline while keeping separate actions and keyboard focus. Caption
columns stay stable. All 187 ordinary UI tests, sixteen light/dark desktop/compact
GPU scenes and strict workspace Clippy pass; see
[the joined control record](validation/joined-tool-controls-gpu-2026-10-23/validation.json).
Popup-specific selection treatment and full native fidelity remain partial.

Latest Classic control styling uses neutral selected surfaces with accent outlines
and distinct current-response hover/press feedback. Alignment glyph contrast is
stronger while text-selection and popup-choice highlights stay accented. All 186
ordinary UI tests, fourteen light/dark GPU scenes and strict workspace Clippy pass; see
[the control surface record](validation/selected-control-surfaces-gpu-2026-10-23/validation.json).
Complete native Windows/historical fidelity remains partial.

Latest Classic canvas work adds reserved edge rails with narrow egui thumbs and
working press/repeat arrows, removes the v11 workspace inset and reports the
actual drawable viewport. All 185 ordinary UI tests, GPU captures/edge-pixel
checks, native macOS fit/overflow captures and strict workspace Clippy pass; see
[the canvas rail record](validation/canvas-edge-rails-gpu-2026-10-23/validation.json).
Resize handles remain disabled previews; complete native fidelity is still partial.

Latest Classic numeric controls use leading-aligned values in both resting and
edit modes, matching the color/text reference insets while retaining native
numeric input. All 183 ordinary UI tests, twelve light/dark GPU states and
strict workspace Clippy pass; see
[the numeric field record](validation/numeric-field-surfaces-gpu-2026-10-23/validation.json).
This resolves the earlier centered-value gap; complete native fidelity remains partial.

Latest Classic text dropdown work reveals selected families/nearby sizes, aligns
choice labels and joins the size field/arrow. The font-count footer has measured
space. All 181 ordinary UI tests, fixture dropdown/real-app text GPU scenes and
strict workspace Clippy pass; see
[the text dropdown record](validation/text-dropdown-surfaces-gpu-2026-10-23/validation.json).
Font discovery and text rasterization are unchanged; full fidelity remains partial.

Latest Classic zoom popup work aligns percentage rows with the reference, keeps
a resting scroll cue and reveals the nearest current preset when opening.
All 180 ordinary UI tests, twelve light/dark GPU scenes and strict workspace
Clippy pass; see
[the zoom popup record](validation/zoom-popup-opening-gpu-2026-10-23/validation.json).
Custom zoom values and document/history semantics remain unchanged.

Latest Classic skin chooser work blocks background undo/tool/canvas input and
uses consistent dialog surfaces with a fixed compact footer. All 179 ordinary
UI tests, eight light/dark GPU scenes, an isolated native macOS wgpu capture
and strict workspace Clippy pass; see
[the chooser record](validation/skin-chooser-surfaces-gpu-2026-10-23/validation.json).
Preferences remain staged until acceptance; historical fidelity remains partial.

Latest shared confirmations use consistent dialog typography/padding and
measured action labels for row/stack layout, with visible long-message scrolling.
All 178 ordinary UI tests, twelve real-context light/dark GPU scenes and strict
workspace Clippy pass; see
[the confirmation record](validation/confirmation-surfaces-gpu-2026-10-23/validation.json).
Save/discard decisions, cancellation and default focus remain caller-owned.

Latest canvas dimension/default dialogs use consistent Classic headings,
padding, fields and contrast, with content-sized desktop bodies and visible
compact scrolling cues. All 178 ordinary UI tests, ten light/dark GPU states
and strict workspace Clippy pass; see
[the canvas dialog record](validation/canvas-dialog-surfaces-gpu-2026-10-23/validation.json).
Resize/default policies stay unchanged; exact native Properties parity is unqualified.

Latest Classic Settings work replaces the colorful top gear with outline art,
aligns navigation symbols/selection styling and improves heading/description
contrast. All 178 ordinary UI tests, nine SVG/cache tests, Classic/Studio theme
GPU scenes and strict workspace Clippy pass; see
[the Settings chrome record](validation/classic-settings-chrome-gpu-2026-10-23/validation.json).
Preference behavior stays unchanged; full native Settings fidelity remains partial.

Latest Classic slider input work fixes ignored coalesced rail clicks through a
shared completion path while preserving native drags and accessibility updates.
All 177 ordinary UI tests, light/dark tap-result GPU captures and strict workspace
Clippy pass; the two failing-before cases are retained in
[the slider input record](validation/classic-slider-input-gpu-2026-10-23/validation.json).
Device/OS accessibility and full reference qualification remain open.

Latest Classic brightness work replaces role-only input with egui's real slider
actions/range metadata, retaining quick clicks and adding stable arrow stepping
and visible focus. All 174 normal UI tests, synthetic accessibility/staging checks,
light/dark GPU states and strict workspace Clippy pass; see
[the brightness input record](validation/brightness-input-gpu-2026-10-23/validation.json).
Physical screen-reader qualification and full native parity remain open.

Latest Classic zoom work joins the percentage selector, shares neutral-shell
thumb styling and lengthens the desktop track while keeping compact bounds.
All 173 normal UI tests, logarithmic shell-alignment checks, light/dark GPU scenes
and strict workspace Clippy pass; see
[the zoom surfaces record](validation/classic-zoom-surfaces-gpu-2026-10-23/validation.json).
Typed drafts and pending-text Escape behavior remain; exact native parity stays partial.

Latest Classic footer work uses theme-aware metric and accented zoom glyphs
through shared action gates and tooltips. All 172 normal UI tests, nine SVG/cache
tests, desktop/compact light/dark GPU captures and strict workspace Clippy pass;
see [the status artwork record](validation/classic-status-icons-gpu-2026-10-23/validation.json).
Metric layout and zoom semantics remain; full native footer fidelity is partial.

Latest Classic Layers controls use an accented outline ribbon glyph and
monochrome panel actions. All 172 normal UI tests, eight SVG/cache tests,
light/dark hover/open/action GPU captures and strict workspace Clippy pass;
see [the layer controls record](validation/classic-layer-controls-gpu-2026-10-23/validation.json).
The existing button size and layer semantics remain; full native panel fidelity
is not established by the supplied ribbon reference.

Latest Classic ribbon work adds the lower-right display-options chevron,
routing to the existing persisted visibility modes. All 172 normal UI tests,
light/dark desktop/compact/hidden GPU states and strict workspace Clippy pass;
see [the ribbon visibility record](validation/ribbon-visibility-gpu-2026-10-23/validation.json).
Narrow windows retain scrolling access; full native reference parity stays partial.

Latest Classic picker work rounds gradient edges, enlarges markers and keeps
brightness thumbs inside their lane without endpoint jumps. All 171 normal UI
tests, seven Classic unit tests, actual rounded-corner GPU pixel checks and
strict workspace Clippy pass; see
[the picker record](validation/picker-gradients-gpu-2026-10-23/validation.json).
Numeric resting-field alignment and full Windows fidelity remain open.

Latest Classic Edit colors work adds reference-proportioned padding, fields,
palettes and a separate footer surface, with clearer contrast and model chevrons.
All 170 normal UI tests, final responsive geometry/cancellation checks, fresh
light/dark desktop/compact GPU captures and strict workspace Clippy pass; see
[the color editor record](validation/color-dialog-gpu-2026-10-23/validation.json).
The 28-slot custom palette remains an extension; full Windows fidelity is partial.

Latest Classic floating size/opacity sliders have gray shells and solid accent
centers, with larger targets and hover/focus/pressed feedback. All 170 normal
UI tests, final pointer/keyboard/accessibility alignment checks, light/dark GPU
states and strict workspace Clippy pass; see
[the slider record](validation/slider-thumbs-gpu-2026-10-23/validation.json).
Custom oversized sizes remain intact; full reference parity stays partial.

Latest Classic desktop Shapes/Colors galleries use larger cells, glyphs and
color controls while retaining compact dimensions below 1200 logical points.
Captions align and Layers stays reachable through repeated resizing. All 169
normal UI tests, eight light/dark desktop/compact GPU captures and strict
workspace Clippy pass; see [the gallery density record](validation/gallery-density-gpu-2026-10-23/validation.json).
Unavailable shape operations and full Windows parity remain open.

Latest Classic v11 Tools use six independently cached outline SVGs and 40-point
controls, with medium-width spacing that keeps Layers reachable. All 168 normal
UI tests, eight SVG/cache tests, light/dark GPU scenes and strict workspace
Clippy pass; see [the Tools record](validation/classic-tools-gpu-2026-10-23/validation.json).
The text toolbar also has refined spacing and a font-family chevron; see
[its record](validation/text-toolbar-spacing-2026-10-23/validation.json).
Picker/magnifier remain disabled and T15 remains partial.

Latest Classic Selection work bounds popup width, uses distinct outline glyphs
and improves text contrast. V11 selection no longer adds a duplicate toolbar row
or shifts the canvas. All 168 normal UI tests, SVG/cache/contact-sheet checks,
fresh GPU popup states and strict workspace Clippy pass; see
[the Selection record](validation/selection-menu-gpu-2026-10-23/validation.json).
The full default icon sheet now includes all 98 IDs. Unavailable selection
operations and full reference parity remain open.

Latest Classic brush-gallery work aligns row/text/sample proportions with the
close-up and removes seams from decorative stroke bands. V11 primary Brush/
Select controls use outline glyphs. All 168 normal UI tests, seven icon tests,
fresh gallery GPU states and strict workspace Clippy pass; see
[the gallery record](validation/brush-gallery-gpu-2026-10-23/validation.json).
Round/square tips retain working behavior; other media remain disabled previews.

Latest Classic shape-paint dropdowns have monochrome trigger glyphs, contiguous
40-point rows and consistent text/selection styling. All 167 normal UI tests,
seven icon tests, fresh GPU popup captures and strict workspace Clippy pass; see
[the shape paint record](validation/shape-paint-gpu-2026-10-23/validation.json).
Solid/no-outline/no-fill choices retain their working semantics; textured styles
and full reference parity remain open.

Latest native checks (2026-10-23) capture complete ordinary light/dark desktop
windows, with correct menu/window names, regular active state and clean native
Quit. An explicit framebuffer review example adds an independent readback path.
Build, failure-path checks and strict workspace Clippy pass; see
[the native presentation record](validation/native-presentation-2026-10-23/validation.json).
The earlier blank captures were not reproduced in the current active launches;
their exact trigger is unproven. Full native/Windows reference parity remains partial.

Latest Classic Image controls follow the two-row reference grouping with a
spanning Resize preview and theme-aware blue-accent SVGs. Rotate/Flip popups
have bounded width, contiguous rows and larger text. All 167 normal GUI tests,
seven icon tests, both GPU exports and strict workspace Clippy pass; see
[the Image ribbon record](validation/classic-image-ribbon-gpu-2026-10-22/validation.json).
Native appearance and complete reference fidelity remain partial.

Current focus is Classic UI fidelity and themes. The [UI progress record](CLASSIC.md)
covers the palette, floating tool panels, document titles, shape icons, text options,
File/Edit menus and compact dropdowns. Settings now uses grouped sections, responsive
navigation and a fixed Apply/Cancel footer. Working persistent toolbar visibility
modes now reclaim/reveal ribbon space while holding gesture layout. Classic owns a
scrollable Layers list with icon actions and fixed property controls. Shared
confirmation prompts now have responsive actions and scrollable explanations.
Canvas-size dialogs also have grouped scrollable fields and a fixed action footer.
Rotate/Flip popups now have larger rows and distinct additional rotation icons.
The v11 quick bar follows Save/Share/Undo/Redo, with sharing disabled and Clear
Canvas retained as an extension; New/Open/picture export stay in File.
Color wells now use centered primary/secondary proportions and named RGB tooltips,
with a dedicated Edit colors icon. Classic v11 now owns stable document-metric
fields and a responsive zoom/status layout with current-frame coordinates. Font
search stays fixed over virtualized choices, and popup input preserves text drafts.
The Image resize preview is now distinct from View’s working canvas-bounds command;
unsupported Image tools explain their availability on hover. Magnifier is also
an explicit preview, while status/View fit remains working; brush-gallery hover
and selected-tip surfaces now follow the shared styling. The 23-item Shapes gallery
now follows reference row ordering with readable names and selected outlines.
Ribbon dropdown triggers show hover/open states, and shape paint choices have
larger selected rows. The Selection popup now has aligned reference rows, a
selected-mode marker and height-bounded scrolling with explanatory disabled tooltips.
The zoom preset popup now has tall reference rows, bounded scrolling, selected
outlines and compact percentage text. The skin chooser now adapts card layout
and scrolling to narrow windows, with staged choices and safe cancellation.
Canvas previews now show eight outlined handles outside the image, with clear
availability feedback and no first-pixel painting interception. Narrow v11 windows
now keep top actions in two reachable rows without changer/quick-action overlap.
Long layer names no longer move Classic property fields; selected rows have
accent outlines and full names are available through single tooltips. Truncated
status values/messages now retain one complete hint without duplicate elision tooltips.
The RGB/HSL dropdown now has larger selected rows and verified nested dismissal
that preserves staged color values. Main v11 menus now use larger rows/icons,
Edit scrolls, and clipped submenu hover plus keyboard dismissal preserve reachability.
Floating size/opacity controls now use dedicated labelled SVG symbols. Typed zoom
drafts now cancel with Escape while preserving pending canvas text. Shared layer-name
edits now have the same staged cancellation without publishing changes. Typed layer
opacity also cancels safely across repeated layout, preserving release-only drag commits.
Text controls and preset/family rows now share 40-point metrics with selected outlines.
Typed font sizes now stage until acceptance and safely cancel while retaining canvas
text; narrow-toolbar scrolling and option isolation are explicitly checked.
Classic layer rows now show aspect-correct committed-content thumbnails over
checkers, with visible-only caching, shared admission and stale-result rejection. 164 window-free egui tests and strict
workspace Clippy pass. The user authorized windowed/GPU review on 2026-10-22.
Fresh light/dark scene exports now qualify the Classic gallery outline change.
Canvas-only scroll-fade removal also has headless and GPU pixel checks across
plain/ruled Classic and Studio, including two-axis panning;
Classic ribbon captions are now larger and brighter, and primary Select/Brush
controls use centered icons with one group caption. Complete native menu/desktop
parity remains unverified. Classic main-menu rows now have contiguous hit targets,
larger text and clearer shortcuts, verified through pointer routing and fresh
light/dark menu captures. Classic main-menu glyphs now use opt-in monochrome SVG variants, reviewed in
File/Edit/View/Zoom GPU scenes. Reference-specific glyph identities and marker
presentation still need work. Earlier captures are historical.
T15 remains partial; further performance experiments are deferred while UI work continues.

The native metadata-waiting implementation and its validation are recorded in
[scoped metadata demand](METADATA_WAITING.md); its [matched overhead comparison](../benchmarks/results/metadata-waiting-2026-10-22/DECISION.md)
retains mixed results and slow controls. [Reserved metadata retry admission](RESERVED_PAINT_METADATA.md) now protects known
capacity before dispatch. [Complete built-in batch preparation](BATCH_PLANNING.md)
now validates structural/logical plans and reserves the new decoded peak before
replay. Its [matched measurements](../benchmarks/results/batch-preparation-2026-10-22/DECISION.md)
retain substantial diagonal/mixed regressions and increased metadata/RSS.
Exact [descriptor sharing](PAINT_PLAN_SHARING.md) reduces repeated-plan declared
payload; [before/shared measurements](../benchmarks/results/plan-sharing-2026-10-22/DECISION.md)
retain memory benefits, mixed timings and slow controls. Joint multi-resource reservations,
remaining performance optimization and native latency remain open.

[Source-bound logical lineage](PAINT_LINEAGE.md) now preserves immutable branch
inputs and expansion/default proofs. Its library comparison retains mixed results;
complete editor batch compilation and decoded peak/cumulative planning remain open.

Current user priority: shared backend correctness, performance and reliability;
see the [backend audit and progress](BACKEND_PROGRESS.md). Classic toolbar/UI work
is deferred during this tranche. [Classic coverage](CLASSIC.md) retains the frontend
fidelity target, and Studio remains available on the shared editor. [Retro](RETRO.md) remains deferred, with W95FA selected as its
default UI font and planned skins for older Microsoft Paint versions and
Deluxe Paint II Enhanced 2.0.

[Dependency choices](DEPENDENCY_CHOICES.md) records the selected libraries and their
actual consumers. The current batch-v1 wire policy and canonical PCND fixture remain
unchanged by dependency and crate refactors.

| Task | Gate | Scope | Status | Remaining work / evidence |
|---|---|---|---|---|
| T01 | M0 | Compiled Rust scaffold and dependency lock | implemented | Native/headless workspace builds and default desktop run are verified. |
| T02 | M0 | Native input and presentation spike | partial | Actual-pixel and multipass tests exist; direct-shell comparison, native pen/IME traces and hardware changes remain. |
| T03 | M0 | Reference acquisition feasibility | external-evidence-pending | Legal pinned Windows Paint/Paint.NET reference binaries and repeatable captures are not present. |
| T04 | M1 | Raster domains and blob leases | partial | Sparse COW tiles retain byte leases through documents/history/readers; default decoded payload admission is process-shared at 2 GiB. Guarded native maintenance visits inactive tabs/parked frontends. Coalesced budget wakes and known render demand reach native contexts; footprint-aware retries preserve fixed-limit failures. Explicit reservations transfer into independently owned tile leases; accepted decoded-failed edits reserve known capacity on retry. Logical blob authority, first-attempt whole-request planning/native multi-window pressure qualification, adaptive metadata/allocation budgets and RSS qualification remain. Paint descriptor and replay-index payload capacities now have a configurable fixed per-plan bound, separate from decoded residency. Global metadata, allocator/hash overhead and aggregate compound admission remain open. Flood classification now bounds bitmap/frontier/output capacities under a separate configurable per-call scratch policy with fallible growth. Aggregate batch/service leases, allocator/transient/RSS accounting and other geometry remain open. Rectangle/ellipse editor geometry now streams into bounded fallible paint descriptors without a separate span vector. Legacy collectors, stroke unions, fonts and aggregate geometry/service budgets remain outside this change. Checked stroke unions now share a configurable per-operation row-map/vector/merge-heap scratch bound. Font/input geometry, allocator controls and aggregate batch/service admission remain open. Deterministic bitmap text now streams directly into admitted descriptors without an intermediate geometry vector. Installed-font resources and aggregate geometry/service memory remain outside this policy. Owned paint projections now retain bounded source-independent descriptor/tile indices and expose declared payload/touched coordinates without pixel ownership. Aggregate program/source-chain/geometry leases and whole-batch decoded peak planning remain open. Logical paint now appends into one bounded program, charging retained descriptor/index capacities plus incoming descriptor capacity during transfer. This aggregates retained plans within one program; shared cross-program/source/scratch leases and physical budgets remain open. Paint descriptors and indices now carry uniquely owned shared-budget accounts in a provisional 256 MiB process realm. Descriptor clones share immutable charged storage; mutation copies under admission and preserves capacity. Aggregate program payload is live-owned across callers/layers, while geometry/font/flood/allocator/physical budgets remain open. |
| T05 | M1 | Immutable document and split identity | partial | Flat immutable snapshots and split state/history identity exist; full session identity and richer nodes remain. |
| T06 | M1 | Atomic batch and operation oracle | implemented | Batch v1 oracle vectors, atomic failure and no-op behavior pass; v2 remains an independently documented extension. |
| T07 | M1 | Recorded history navigation | partial | Recorded hot/cold linear undo/redo and guarded worker restoration preserve IDs and pixels, including leased disk payloads. Recorded restoration pins reusable blobs and reserves its complete missing set before decoding; feasible native undo/redo/seek failures retain guarded FIFO requests. Branching graphs, process-wide residency and full metadata admission remain. |
| T08 | M1 | PCND and headless round trip | partial | Canonical PCND and CLI round trips pass; full native subset/hostile-archive scope remains. |
| T09 | M1 | First complete editor slice | partial | Complete pencil/session/display/save slice passes; native input qualification depends on T02. |
| T10 | M2 | Ordered save and recovery journal | partial | Streaming native saves, bounded FIFO, captured-state guards and claimed recovery pass; write-ahead snapshots/receipts survive process exit; restarted destination inspection and explicit durability confirmation retain historical identity without adopting current state; degraded-store policy, benchmarks and full fault/platform matrix remain. |
| T11 | M2 | Bounded history and store GC | partial | Preserving history handles large snapshots through bounded classification/transfer. Native guarded storage maintenance reclaims eligible inactive-owner history without content/cursor/save changes and yields to queued foreground edits. Shared byte-budget notifications and known render demand coordinate native consumers across contexts; footprint-aware retries preserve fixed-limit failures. Accepted decoded-failed edits retain FIFO/file barriers and can reclaim their own history before a reserved retry. Initial worker-admission races retain captured tasks and FIFO order. Decoded navigation now retains guarded FIFO requests and retries with reserved capacity; cleanup completion is consumed only in the coordinator poll phase. History plans reserve full missing sets and waiting navigation reprices after activity or periodic fallback. Ordered paint operations now reserve touched-tile capacity before writes. Whole-batch/effect planning, other resource retry lifecycles, native multi-window qualification, all-service budgets, relocation and native latency qualification remain. Per-plan descriptor/index growth now has a fixed metadata limit before tile replay, a prerequisite for retaining compound plans. Whole-batch planning/reservation and shared aggregate metadata accounting remain open; fixed metadata failures are not decoded-capacity retries. A read-only paint projection and fallible flood source can inspect earlier logical writes without allocating decoded tiles; recovered descriptors replay under ordinary admission. The editor still executes sequentially; bounded complete-program ownership and upfront compound reservation remain open. A combined logical program can recover one ordered plan whose whole touched set is admitted before decoded replay; the two-operation quota regression allocates no private prefix. Editor batch integration, structural lineage and complete decoded peak planning remain open. Shared paint metadata accounts now survive projection retention, replay and descriptor recovery; cancellation/error paths free storage before charge release. Metadata pressure is classified separately from decoded history pressure. Known metadata demand is scoped across attempts; native FIFO waits and reserved known-capacity retries preserve file barriers and class isolation. Whole-batch/lineage integration and joint resource admission remain open. Current built-in batches now compile complete structural/logical plans and simulate COW/private/shared tile ownership before one decoded-peak reservation. Exact cumulative-work and recycled-capacity tests pass. Joint metadata/history/service/physical budgets remain open. |
| T12 | M2 | Ordered renderer and region effects | partial | Ordered CPU/GPU flat composition, exact regional CPU output, bounded output/lower-prefix caches and caller-owned CPU band/tile partitions exist with local measurements; CLI exports accept explicit worker limits. Derived 64-pixel identities preserve untouched COW cells and patch complete cached output tiles without changing storage granularity. Finer operation rectangles, ROI effects, parallel cache-miss integration, sparse GPU working tiles, display-texture pooling, hierarchical refinement and group plans remain. GPU composition now retains bounded clipped layer sources and uploads only changed row strips; composition still recomputes each requested region and GPU crossover qualification remains. Native-resolution windows consume completed premultiplied GPU textures directly; straight readback/export remains available separately. Desktop CPU/GPU display now requests exact visible bounds, keys completions by region and positions each texture at its own document bounds; a bounded CPU composite-before-box-filter path now handles zoomed-out regions without a full native source surface; Tiled presentation remains. Completed private compositor buffers support explicitly bounded reuse; fresh allocation remains the default after mixed/regressing native process evidence. Public display textures retain independent ownership. Coarse caches include level/source identity, preserve preview/committed isolation and use metadata uniform proofs; a million-square fixture has bounded native pixel work. Historical sampling parity remains unqualified. Exact bounded GPU scaled composition now passes CPU/Metal/framebuffer oracles with wide integer sums and uniform proofs; automatic scaling stays CPU after measured tiny/dense regressions. Explicit native GPU scaling remains available. Sparse GPU working sets, asynchronous reduction and hierarchical refinement remain. Bounded optional regional GPU sources now preserve multiple exact views per layer, patch views independently and protect the complete scaled request under existing limits. The original source policy stays default after measured upload/allocation reductions failed to establish a throughput gain. Optional ordered GPU batches now preserve exact per-layer rounding under 16-layer, 1 MiB source and bounded-key windows. Native/readback/scaled paths share the compositor; independent frame textures and completed scratch ownership remain intact. Serial defaults remain after mixed five-layer timing and higher RSS. Shared completion handling now retires the GPU epoch on poll, readback callback or mapping failure before returning an error. Successful rendering/cancellation semantics stay unchanged; retained frames and later requests reject the retired lifetime. Native GPU calls now attach their existing runtime resource lease to synchronous PaintCraft submissions. Unobserved work retains ownership through queue completion callbacks; successful waits and returned completed frames do not prolong the task charge. Native window-owned completion observers now poll deferred admission callbacks without canvas repaints, using private renderer/frame identities rather than cross-instance queue equality. An explicit native-resolution single-layer path now converts a fully covering cached source directly into an independent display texture, preserving alpha/opacity arithmetic and falling back for partial/multiple contributors. It avoids intermediate composition only; straight exports and scaled output retain their paths. Mixed process/control/RSS evidence keeps ordered defaults. |
| T13 | M2 | Scheduler and GPU preparation | partial | Shared native render/edit workers prepare validated stroke/text plans from immutable previews. Revision-keyed weak snapshots avoid recollecting unchanged repaints; queued edits, file barriers and completion invalidate inputs. Guarded no-op results and compatible progress have tests. Preview-capture process costs are locally measured; changing-input cost is inconclusive. Device epochs reject replaced/retired GPU completions; Metal destruction and fresh-device host-history reconstruction pass. Native edit/render pools share declared-byte admission with worker/result guards surviving executor drop. Footprint-aware render retries and shared demand guide decoded pressure maintenance. Initial worker-admission races retain captured batch/tool/navigation tasks; decoded-failed edits and recorded navigation retry reserved known capacity under owner/base guards. History navigation plans fresh missing sets and reprices stale cache-dependent demand. Ordered paint plans admit operation tiles and replay contiguous rows; scoped measurements retain diagonal non-win controls. Native window-device recovery, priorities, physical/all-service budgets, changed-preview costs and native latency/interference qualification remain. Detached execution now checks cancellation between commands and within smooth coverage and planned paint indexing/preparation/replay; wall-clock cancellation tails and other kernels remain unqualified. Desktop preview staging now reuses one bounded-charge incremental sample union, scans appended capsules and routes worker cancellation into execution; measured growing/retrace CPU replay gains retain tiny non-win controls. Full incremental staging and native latency remain unqualified. Bounded device-local source views now reuse unchanged uploads, stream protected-frame overflow and retire partial transfers on cancellation/epoch failure. Native admission reads immutable cache allowances; sparse GPU atlas/output residency and asynchronous transfers remain. Native texture registration validates document/preview/device identity on the UI, retires old IDs on fallback/switch/drop and avoids normal host readback/re-upload. Display-texture pooling and native event/display tails remain unqualified. Native output admission now follows requested viewport area; stale region completions cannot replace a panned view and CPU fallback preserves region bounds. Full operation staging and native latency remain unqualified. Configurable compositor scratch retention has completion/cancellation guards and native allowance accounting; default retention is zero following the measured decision. Physical pressure-driven GPU eviction remains open. Scaled display jobs now declare coarse output dimensions plus bounded cache/key/reduction scratch, include sampling level in freshness and preserve the native GPU while using the CPU reference. Hardware tests admit million-square reduced output; Physical/latency qualification remains. Completed GPU scaled preparation and real native fallback now include sampling-level checks, bounded scratch declarations and old-frame ownership. Explicit GPU scaling uses 64-sample tiles; default CPU scaling avoids measured regressions. Physical budgets, asynchronous completion and native device reconstruction remain. GPU source eviction protection now spans complete display requests rather than resetting for each internal block. Over-cap requests preserve early views and stream overflow; per-layer completion waits and physical GPU admission remain unchanged. Native admission includes an immutable extra 4 MiB pending-resource allowance when batching is explicitly enabled. Cache identities publish after successful batch completion; cancellation drops unsubmitted work and cannot publish partial frames. This is synchronous bounded batching, not asynchronous transfers or physical GPU/RSS admission. Completion failures now quarantine the shared lifetime instead of leaving an uncertain device eligible for later submissions. Actual Metal invalid-submission failure and native CPU fallback preserve host state. Timeout policy is tested without hanging hardware; physical outstanding-work leases and automatic native device reconstruction remain open. Explicit resource leases now keep both pool-local and shared declared bytes alive after result consumption, cancellation or executor Drop. Native GPU failure/unwind transfers scope ownership to queue completion observation; ending the queue context releases callback ownership. Cancellation-token clones observe leases weakly. Window-owned completion observers now provide pending-only polling independently of repaints and compositor retirement. Private renderer/frame identities prevent cross-instance queue-ID aliasing; weak callback links and nonjoining shutdown avoid context cycles. Observer startup/poll failure degrades to external observation, preserving charges. Physical driver accounting, asynchronous transfers and native device recreation remain open. Optional direct single-layer presentation shares existing source/completion ownership and conservative admission, without exposing mutable cached sources as returned frames. Native worker and actual egui framebuffer parity pass; physical allocation and input/display tails remain unqualified. Flood classification now yields during bitmap initialization, wide-row scanning/marking and frontier processing, with the real worker token and fixed scratch policy. Independent oracles and private-prefix cancellation/rollback pass; callback work counts do not qualify native wall-clock cancellation tails. Rectangle/ellipse visitors now check the actual worker token per clipped row, including rows with no output; failed sinks or cancelled geometry discard private batch prefixes. Wide ellipse arithmetic is exact for admitted large coordinates. Native wall-clock tails and other kernels remain unqualified. Pencil vertex accounting and invisible path traversal now yield cooperatively; checked union rows and bounded sort/merge chunks observe cancellation. Native staging errors retire acquired preview geometry. Work-count checks do not qualify native wall-clock tails. Built-in text checks cancellation per character/row/emission; resolved-mask validation, cumulative accounting and clipping check bounded chunks, including offscreen spans. Shaping/font loading and native wall-clock tails remain unqualified. Ordinary replay and read-only projections now share the checked ordered index builder. Logical reads cancel across long overlapping/noncovering descriptor lists; fallible flood reads discard partial classification. Complete batch retention, source lineage and aggregate scheduling/admission remain open. Consuming append discards both inputs on any metadata/allocation/cancellation failure. Combined reads use one ordered tile index and reach their preceding source once, rather than walking per-operation source projections. Cross-layer ownership and scheduler/native latency remain unqualified. All editor paint factories propagate shared metadata admission, with fallible growth and correct COW snapshot lifetimes. A native regression prevents PaintMetadata denials from entering decoded-history reclamation. This adds declared ownership, not a complete scheduler/fairness or physical-budget implementation. Complete current-grammar batch preparation now precedes decoded replay, with source-bound flood reads and cancellation; write-only batches omit logical views. Scheduling fairness, physical budgets and native latency remain unqualified. |
| T14 | M2 | Tool completeness tranche | partial | Antialiased brushes, bounded queued paint with ordered compound previews, Studio primary/secondary gestures, fill, shapes, rectangle selection and captured-layer Cosmic Text/Swash previews work. Ordered operation plans share exact coverage and upfront tile admission across painting families. Complete selections, secondary eraser/text policies, editable text objects, IME and native sustained-input latency qualification remain. Detached execution now checks cancellation between commands and within smooth coverage and planned paint indexing/preparation/replay; wall-clock cancellation tails and other kernels remain unqualified. Desktop preview staging now reuses one bounded-charge incremental sample union, scans appended capsules and routes worker cancellation into execution; measured growing/retrace CPU replay gains retain tiny non-win controls. Full incremental staging and native latency remain unqualified. Every painting family now constructs plans from the admitted metadata policy; descriptor/index failures discard private prefixes without changing owner/history state. Input/geometry buffers, aggregate batch metadata and native latency remain outside this per-plan guard. Flood preparation now preserves prior staged walls and local selection/origin semantics while cooperating with cancellation and bounded scratch growth. It compiles local spans directly, without the duplicate coordinate-conversion vector. Complete batch planning, other kernels and native latency remain open. Rectangles/ellipses now stream clipped spans directly into their shared fill/outline plan. Large legal ellipse endpoints no longer overflow intermediate math; independent wide pixel oracles and recorded undo/redo pass. Stroke unions, font geometry, full selections/editable text and reference/native latency qualification remain. Pencil/smooth-stroke accounting traverses borrowed paths without copied segment/normalized-point vectors. Unsmoothed brushes and lines retain exact once-per-stroke blending through a bounded cancellable union. Other font/input geometry, full selections/editable text and native latency remain open. Bitmap text retains exact whole-pixel glyph semantics through a fallible streaming visitor; resolved grayscale masks now cancel during validation/accounting/clipping. Installed-font rasterization, editable objects and IME retain their separate scope. Metadata interference after an earlier private paint now has an editor regression for ticket/history/snapshot preservation and complete attempt storage release. Tool semantics and wire policy remain unchanged; native latency and remaining tool/text features stay open. Current built-in painting families now participate in complete batch compilation and decoded admission, with source-pinned differential and cancellation checks. Future tools, editable text/IME and native input latency remain open. |
| T15 | M2 | Classic v11 usable preview | partial | Classic palette, tool panels, native titles, shape/icon gallery, text controls, File/Edit menus, view actions and responsive settings exist. Persistent toolbar visibility modes now work without shifting active gestures. Classic now has its own scrollable layer-list presentation with icon actions and fixed property fields. Responsive confirmation prompts retain reachable actions and safe cancellation. Canvas-size dialogs now use grouped scrollable fields and a fixed action footer. Rotate/Flip preview menus now use larger rows and distinct additional rotation icons. The v11 quick bar now follows Save/Share/Undo/Redo with the Clear Canvas extension; sharing remains disabled. Color wells now have centered primary/secondary proportions and named RGB tooltips, plus a dedicated Edit colors icon. Classic v11 now owns a responsive status bar with cursor/selection/canvas fields and stable zoom controls. Font popups now keep search fixed over virtualized rows and preserve pending canvas text on typing/Escape. The Image resize preview no longer substitutes canvas-bounds editing; unsupported Image tools have clear disabled-hover explanations. Magnifier is an explicit preview distinct from fit-to-window; brush-gallery hover/selection styling is refined. The 23-item Shapes gallery follows reference ordering with readable names and selected outlines. Dropdown triggers now show hover/open states and shape paint choices have larger selected rows. The Selection popup has aligned reference rows, a selected-mode marker and short-window scrolling. Zoom presets now have tall rows, bounded scrolling and compact percentage text. The skin chooser now has responsive theme-aware cards, staged choices and safe dismissal preserving pending text. Canvas handle previews now show all eight positions outside the image with explicit unavailable feedback and safe first-pixel painting. Narrow v11 windows keep menu/settings/changer controls and quick actions in two nonoverlapping rows, including auto-hide mode. Long layer names retain fixed Classic property fields and have single full-name tooltips; selected rows show accent outlines. Truncated status fields/messages now retain one explicit tooltip without an extra automatic hint. RGB/HSL dropdown choices now have larger selected rows with nested Escape and staged-value preservation checks. V11 main menus now have larger rows/icons, scrollable Edit, clipped submenu hover and keyboard child dismissal. Floating size/opacity controls now use dedicated SVG symbols and tool-specific header labels. Typed zoom Escape now discards the local draft without applying it or cancelling pending canvas text. Layer-name Escape now discards the staged rename and preserves pending canvas text in both frontends. Classic layer rows now have real background-rendered content previews with bounded output/caching, shared admission and stale-source rejection. Classic gallery entries now use separate theme-tinted outline SVG variants, reviewed in fresh light/dark GPU scenes. Canvas scroll fades no longer tint clipped pixels in either frontend, with scoped-style and GPU pixel regressions. Classic ribbon captions now use larger readable text, while Select/Brush use centered icon-only controls with preserved accessibility and host actions. Main-menu rows now have contiguous 40-point hit targets, 16-point text and legible shortcuts, with real pointer-boundary action checks and fresh menu captures. Main-menu glyphs now use original theme-tinted outline variants through the shared action helper, with fresh File/Edit/View/Zoom scene review and separate cache identities. Resize and skew opens a staged UI preview with transform application disabled. Latest validation: 207 ordinary GUI tests, 23 shared UI tests, stale-preview rejection, GPU scenes and strict Clippy. Disabled operations remain previews; full reference fidelity, extra brush/shape/transform semantics, clipboard/platform operations, preserving-target flow and complete M2 prerequisites remain. Windowed/GPU review is authorized; fresh scene and pixel checks have documented scope, with native menu/desktop parity still unverified.  View Thumbnail now includes bounded active paint/shape/text previews, with stale-scope guards and older compatible frames marked updating. |
| T16 | M3 | Classic historical parity | not-started | Per-operation evidence records; original menu/eraser/canvas/tool lifecycles and allowed OS differences. |
| T17 | M3 | PDN and PAINT compatibility | not-started | Read/write/metadata tested independently; malicious parsing bounds; no .NET deserialization/runtime. Fresh creation and source-preserving rewrite qualify independently; retain source bindings across repeated saves. |
| T18 | M3 | Wasm effect SDK for contributors | not-started | ABI range/trap/halo/mask tests; one complete external-author workflow before SDK stabilization. |
| T19 | M3 | Studio parity tranches | partial | Studio toolbar, floating Tools/Colors/Layers, guarded History navigation and independent document tabs with owner-routed save/recovery and close handling work; shared descriptors and menu/shortcut dispatch cover existing actions; full action coverage, image operations, effects, PDN and reference parity remain. |
| T20 | M3 | Codec breadth | not-started | Per-backend/variant write+read evidence, metadata/index and worker failures; no Rust-only delays. |
| T21 | M3 | Performance decision experiments | partial | Local CPU partition/cache/region-granularity experiments retain raw statistical/process samples and desktop CPU consumer confirmation. General dispatch thresholds, spatial-index choices, GPU atlas layouts, other adoption experiments and cross-platform/native latency qualification remain. Streamed/resident real-GPU preview experiments include completed readback and an over-entry-limit working set; they do not establish CPU/GPU crossover or atlas choices. Completed native viewport textures have modest fixed/panning process gains on one Metal host; whole-canvas controls remain mixed/tied, without OS event/display claims. A 0/64 MiB compositor output-pool experiment reduces logical buffer creations but retains small-view regressions, mixed 4K results and statistical/process disagreement; global enablement is rejected. Completed CPU multiresolution/native-upload comparisons retain an existing cached full-resolution control, dense cache benefits over uncached reduction, tiny ties and a 3.12–3.20x sparse-4K process gain on one host; no GPU-scaled or OS-latency claim follows. Completed CPU/GPU scaled native experiments compare two output-tile edges with 240 raw statistical samples and 780 process observations. Tiny/dense GPU regressions, sparse-4K ties and uniform-4K advantages reject global enablement; no general crossover is claimed. Regional-source experiments retain 320 raw statistical samples and 1,020 process observations across both tile edges, including over-entry-limit and no-source-work controls. Mixed timings, host drift and mostly tied statistical samples reject global enablement despite lower source uploads/allocations. Completed batching retry retains 480 raw statistical samples and 1,500 process observations across 1/5/20/100 layers plus sparse/uniform controls. Twenty/hundred-layer gains coexist with mixed five-layer timing and higher RSS; serial defaults remain. Native experiment flags expose batching alone or with regional retention. The terminal ENOSPC attempt is preserved separately with another 480 statistical and 300 raw process observations. Completed native single-layer comparisons retain 240 raw statistical samples and 780 schema-v2 Hyperfine observations. Eligible displays remove eight intermediate allocations/submissions with identical uploads; tiny/dense statistical gains coexist with a large process regression, varying fallback controls and no general RSS reduction. Ordered defaults remain; explicit opt-in is available. Matched CPU-only stroke comparisons retain 160 Criterion samples and 720 Hyperfine observations, exact canonical output and calibration. Square/round/retrace checks incur measured overhead; this is a required correctness baseline, not a throughput win. Checked-union optimization and native latency remain open. A source-pinned real-library projection/materialization plus flood comparison retains kernel/process/resource controls. It measures preparation primitives, not editor whole-batch integration or native latency; production selection remains provisional. Completed projection/materialization preparation evidence retains 160 raw statistical samples and 540 process observations with exact all-pixel/region postflights. Tiny gains and wall ties coexist with 9–10x chained and over-4x no-op regressions; sequential editor replay remains, and a bounded combined program requires separate integration/qualification. The combined program is compared with both materialized and chained sources using identical fixture pixels/regions and no-op controls. It remains a preparation primitive until native whole-batch/source-lineage integration and contextual proof costs are qualified. Combined-program evidence retains 240 statistical samples and 780 process observations with exact required output. The 64-plan fixture improves 13.94–15.58x over chained and 1.38–1.57x over materialized preparation with 3,192 versus 19,968 retained metadata bytes. Wall tier disagreement and the roughly 4.4x no-op regression retain sequential editor defaults; native whole-batch integration remains open. Shared metadata guard overhead is compared with frozen source-matched CPU-only CLI and library artifacts, keeping identical feature graphs, canonical output, slow controls, startup calibration and original raw exports. Accounting is selected for correctness, without an unmeasured acceleration claim. Shared metadata evidence retains 480 statistical samples, 1,200 original process observations and a separately labeled 240-observation eraser follow-up. Exact canonical/library outputs and matched CPU-only feature graphs pass. Original 48.9% eraser slowdown remains despite nonrepetition in follow-ups; tiny library overhead, mixed controls and RSS remain. Ownership is a correctness adoption, not a general speedup or physical-budget qualification. Complete editor/CPU-CLI batch evidence retains 280 statistical samples and 1,860 original process observations. Exact semantic/archive/decoded checks pass. Diagonal and mixed-lineage regressions and higher retained metadata/RSS require optimization; no speedup is claimed. |
| T22 | M4 | Browser conformance | not-started | Native-equivalent fixtures, interruption tests and unsupported-capability reports. |
| T23 | M4 | RetroPaint and historical formats | not-started | Historical Paint and Deluxe Paint II Enhanced 2.0 skins; era-specific fixtures and explicit additional capabilities without changed default muscle memory. |
| T24 | M4 | Compact PCND container study | not-started | Benchmark justification, corruption tests and old-v1 corpus preserved. |
| T25 | M5 | Mobile adaptation | not-started | Device execution, orientation/background/quota/thermal evidence; not cross-compile-only. |
| T26 | M5 | Optional AI and MCP | not-started | Consent/provider/model boundaries and stale-result/undo safety; no mandatory account. |
| T27 | M5 | Paint 3D research | not-started | Separate design and capture corpus before implementation claims. |
| T28 | RELEASE | Public release readiness | not-started | Target-specific manual/native tests, security protocol, license notices and final capability manifest. Capability manifest claims require profile-scoped executed evidence, not model-test status. |
| T29 | M1 | Version-aware document admission | partial | Exact v1 admission and unsupported rejection exist; all VER cases and future preview modes remain unqualified. |
| T30 | M3 | Foreign-source lineage and preservation planner | not-started | PRES-01..10; preserved opaque bytes AND relationships; dependent/unknown edit refusal; alternating original-consumer edits and source-removal recovery. |
| T31 | M2 | Frozen resource snapshots | partial | Native recovery verifies exact input identity and retains decoded state under a source claim; general resource snapshots, source bindings, fonts/profiles and DPI/clock cases remain. |
| T32 | M2 | Truthful file receipts and save associations | partial | Stable receipt IDs, sequences, binding guards, alias checks and live/resumed reconciliation pass; native reacquisition validates path-free persisted evidence and isolates historical outcomes from current document adoption; open-to-save conflict guards, diagnostics/repair and filesystem qualification remain. |
| T33 | M1 | Capability discovery and admission checks | partial | Truthful CLI manifest exists; shared generation-aware GUI/worker capability service remains. |
| T34 | M1 | Hash-guarded one-shot automation | partial | Immutable read and pre-parse hash guard exist; complete cancellation/retry lifecycle cases remain. |
| T35 | RELEASE | Profile-scoped release qualification | not-started | QUAL-01..10; synthetic or missing oracle cannot promote a release; foreign fresh/preserving support qualified separately; relevant backend changes invalidate scope. |
| T36 | M3 | Bounded shared-component extraction review | not-started | REUSE-01..06; independent product policies/versions and CLI dependency graph; no generic Craft runtime, foreign UI, P2P or database engine requirement. |
| T37 | M0 | Scoped adoption and benchmark evidence workflow | in-progress | Dependency/task ledgers and source-scoped CPU/history/painting experiments retain raw statistical/process evidence and integrated CLI/desktop CPU confirmation. Region-grid and preview comparisons retain slow controls, memory costs, host drift and failed builds explicitly. Prior Divan-only raw-sample gates, other adoption comparisons and cross-platform/native input/display qualification remain. GPU source-reuse evidence retains raw Criterion/process samples, staged-byte counters, over-limit behavior and a flagged timed outlier without promoting native latency claims. Viewport evidence retains 120 raw Criterion samples, 420 process observations, whole-canvas controls, source/work counters and failed development validation. Completed scratch-pool comparison preserves another 120 raw statistical samples and 420 process observations, including resize retention costs and the negative default-policy decision. Multiresolution evidence retains 180 statistical samples and 600 process observations, exact compose-before-filter pixels, source/binary identities and a test-only recursive-boundary follow-up; no missing-work or failed baseline is timed as a win. GPU scaled evidence retains source/binary identity, all raw samples, independent exact required output, process calibration, slow controls and the final CPU-default policy separately from measured candidate source. Regional-source evidence preserves complete raw exports, 23 required Metal regressions, native acceptance oracles, calibration and slow controls. The measured source policy remains explicitly configurable without a general acceleration claim. Batching evidence separates frozen measured source from final native-option integration, retains all slow controls and interrupted exports, verifies source/binary hashes, and records disk headroom after dev-only Cargo cleanup. No partial export or missing variant is promoted into a completed result. Completion quarantine is a correctness change with explicit failure-policy tests, a real invalid-wait Metal case, required-GPU desktop routing and full workspace validation. It makes no performance or physical-reclamation claim. Admission-lifetime validation includes real Metal failed waits, unwind, nested charges and queue teardown, plus runtime local/shared limit retention and cheap-token release. An existing overflow rejection diagnostic was restored after a development test caught its regression. This is declared ownership correctness without a throughput/RSS claim. Completion-progress tests verify deferred release without caller polling, idle/successful zero-poll behavior, cross-instance identity isolation and owner/context teardown on Metal. The first mismatch test exposed queue equality aliasing; private renderer/frame identity fixes it. No throughput or physical-reclamation claim follows. Single-layer adoption pins committed source/binary, exhaustive alpha/opacity and independent native framebuffer/worker oracles, raw statistical/process samples, calibrated setup costs, RSS and negative controls. Hyperfine schema-v2 analysis preserves original exports; mixed evidence rejects global activation. Corrected development compile diagnostics remain archived. Paint metadata is a correctness safety baseline: capacity-weighted descriptors, usable index payload and replay vectors are bounded per plan, with no physical RSS or throughput claim. Tests cover repeated writes, table growth, all eight painting families and late compound rollback; historical timing records do not qualify new guard overhead. Flood safety validation compares 36,864 binary-image/seed/clip/tolerance cases with independent BFS, injects cancellation at every observed wide-row checkpoint and during bitmap initialization, tests scratch denials/signed endpoints and preserves owner/history after a private prefix. Safety limits and intervals are provisional, without throughput, RSS or wall-clock claims. Shape validation compares streamed results to independent pixel-centre/inset oracles, exact large-coordinate products to schoolbook limbs and clipped ellipse history under debug/release. Row cancellation and sink failures preserve owner/history. A corrected test-oracle index annotation is retained as development diagnostics; no throughput/RSS or wall-clock claim follows. Stroke evidence pins matched CPU-only feature graphs and frozen binaries, retains all raw statistical/process observations and exact archive/member postflights, and reports checked-union regressions without selecting an unsafe legacy path. Text safety evidence covers independent glyph-bit pixels, exact collector order, every observed visitor/validator checkpoint, signed boundaries and private-prefix rollback. No callback/visitor throughput, RSS or native latency qualification follows. Projection validation covers unchanged decoded usage/weak versions, exact ordered/coverage reads, admitted recovered replay, bounded indices and cancellation/source failures. An independent BFS checks 18,432 projected image/seed/tolerance cases. Whole-batch acceptance is not inferred from this prerequisite. Completed projection/materialization preparation evidence retains 160 raw statistical samples and 540 process observations with exact all-pixel/region postflights. Tiny gains and wall ties coexist with 9–10x chained and over-4x no-op regressions; sequential editor replay remains, and a bounded combined program requires separate integration/qualification. Combined-program tests cover aggregate and incoming-transfer quotas, every observed append checkpoint, ordered mixed coverage, one source read, zero-prefix replay denial and 18,432 independent projected BFS cases. The source-pinned three-way benchmark preserves all controls; no whole-batch gate is inferred. Combined-program evidence retains 240 statistical samples and 780 process observations with exact required output. The 64-plan fixture improves 13.94–15.58x over chained and 1.38–1.57x over materialized preparation with 3,192 versus 19,968 retained metadata bytes. Wall tier disagreement and the roughly 4.4x no-op regression retain sequential editor defaults; native whole-batch integration remains open. Shared ownership tests cover atomic resize/contention, reentrant release order, metadata realm migration, cloned plan sharing/COW failure, exact clamped-capacity copies, index/projection recovery/cancellation, private-prefix editor rollback and native decoded-retry isolation. Qualification remains scoped to executed profiles and measured source. Shared metadata evidence retains 480 statistical samples, 1,200 original process observations and a separately labeled 240-observation eraser follow-up. Exact canonical/library outputs and matched CPU-only feature graphs pass. Original 48.9% eraser slowdown remains despite nonrepetition in follow-ups; tiny library overhead, mixed controls and RSS remain. Ownership is a correctness adoption, not a general speedup or physical-budget qualification. Source-pinned whole-batch evidence and original diagnostics are archived with independent PCND/library semantic checks and explicit regressions. |
