# PaintCraft Studio

Studio shares the editing engine with Classic and has its own presentation. Its
presentation lives in
`paintcraft-studio`, with the desktop shell supplying the editor and native services.
Select **Studio** in the application header; the choice is saved for the next launch.
Selawik remains the interface font. RetroPaint's selected font, W95FA, is recorded
separately in the dependency choices and is not bundled yet.

## Implemented presentation slice

- Compact File/Edit/View/Layers/Window menus, file and undo/redo toolbar, and tool options.
- Document tabs with independent editors, histories, targets, tools, colors, zoom,
  scroll positions and native-file bindings. New/Open/recovery preserve other tabs.
  Close supports save, discard and cancel; closing the last tab leaves an empty workspace.
- Movable Tools and Colors windows, with real tool selection, primary/secondary
  colors, color wheel, swap, palette, expanded RGBA/hex controls, brush size/tip, shape style, fill tolerance and font text family/size/style controls.
- A movable/resizable Layers window using the existing add, select, visibility,
  rename, opacity, reorder, duplicate and delete operations. Window menu toggles restore panels.
- A movable History window with recorded action labels, current/redo rows, direct
  snapshot navigation and an explicit notice when older entries were trimmed.
- Centered canvas in both Studio and Classic when the viewport is larger than the document; scrolling and
  actual-pixel zoom retain the common mapping.
- Zoom tool with click stepping, rectangle-to-viewport fit and text-safe Z shortcut.
- Pan tool with H, temporary Space-drag, middle-button pan and hand cursors.
- Overscroll with a persisted Studio navigation preference.
- Shared staged brush previews, CPU/wgpu composition, transactions and undo/redo.
  Open, Save As and Export retain the native file-dialog and preservation paths.

Frontend controls/actions live in `paintcraft-ui`. Studio has no dependency on the
Classic presentation crate. The shell executes actions through its existing outbox;
the presentation never writes files or mutates published document state directly.
Clicks in floating windows cannot start strokes on the canvas underneath them.

Settings v3 saves the active experience independently of the Classic version and
adds Classic canvas-size preferences. V1/v2 settings migrate with their original
interface choice retained. Studio New retains its existing 320 × 320 size. Launching a
saved Studio workspace skips the Classic picker; entering Classic for the first
time still asks for its version. Switching presentation preserves the current
document, target and history; an active stroke is cancelled and active text follows
the existing explicit commit path. Panel placement and visibility currently last
for the running session, not across restarts.

## Next Studio work

1. Expand the shared action registry with the remaining Studio menus and working
   workflows described in the supplied reference plan.
2. Expand history storage and metadata admission, then qualified branching/spill
   behavior; the recorded History window is implemented on the current linear store.
3. Image/layer editing operations and richer selection interactions through typed
   commands, with clipping, atomic failure and history coverage.
4. Region-aware adjustments/effects, followed by qualified GPU implementations.
5. Shaped system-font text and IME interactions; broader codecs and PDN compatibility.

This slice is not Paint.NET parity or a completed T19/M3 gate. Document tabs are
working; thumbnail navigation, Save All, effects UI and PDN import remain deferred.
Reference-build captures, the remaining engine prerequisites and native qualification
remain separate acceptance work. Tests cover saved experience migration, preservation
on switching, floating-window input ownership, brush undo/redo, multipass action
delivery, direct history navigation and reopening Layers/History. An ignored
native-GPU test renders a review image:

```sh
cargo test -p paintcraft --test gui studio::render_studio_workspace_for_review -- --ignored
```

The image is written to `/tmp/paintcraft-studio-workspace.png`; it is a review artifact,
not an independently captured reference or cross-platform visual-parity result.

## History navigation contract

Labels are typed semantic metadata attached only to committed history entries.
They do not enter batch digests, document pixels or PCND bytes. No-op and failed
batches add no label or node. Selection-only entries retain their own history
identity even when the document StateId is unchanged. Redo restores stored output.

History-window clicks capture a stable history identity plus the originating
session owner, StateId and publication epoch. The editor validates all three before
moving the cursor. A valid jump restores the document and selection with one new
publication epoch, regardless of distance; a click on the current entry is a no-op.
Stale views, foreign sessions, trimmed entries and discarded redo targets leave the
cursor and publication intact. Pending edits run first and make older clicks stale.
An active stroke/text disables list navigation until resolved. Merely viewing the
list retains no raster leases and adds no undo step.

The list virtualizes visible rows. It still gathers metadata for all retained
entries; metadata admission and large-history performance remain unqualified.
Existing linear redo truncation and automatic memory-budget trimming are unchanged;
branching and disk spill remain roadmap work. History survives interface switching,
but is not yet persisted across process restart.

## Document ownership and native I/O

Each open document owns its editor and native save-adoption context. One bounded
native I/O queue remains shared by the window: saves to the same destination stay
FIFO, and completions acknowledge only the originating document. Background
checkpoints run for inactive documents too; admission pressure retries rather than
starving later tabs. Recovery offers are workspace-wide. Restoring one creates a
new Studio tab and preserves the existing source-retention handoff.

Native pickers retain their originating tab. Open reuses an already bound document
when its path resolves to that same file. Saving over another open document's
known destination asks for confirmation; a confirmed replacement invalidates the
other version's saved marker when its contents differ. Path comparison is a best-
effort local canonical-path check, not a universal file-identity or external-write
conflict protocol.

Closing waits for that document's pending I/O. Save-and-close captures its identity
and state; cancellation, failure, or newer edits keep it open. Workspace close
checks inactive documents as well. These tests cover in-process owner routing,
FIFO replacement, modal ownership, checkpoint backpressure, independent histories,
empty workspaces and save/close failures. The 64-document cap is provisional;
large-workspace memory and platform fault qualification remain open. Tab layout,
viewport state and history are not restored across process restart.

The native GPU review test for tabs writes `/tmp/paintcraft-studio-tabs.png`:

```sh
cargo test -p paintcraft --test gui tabs::render_document_tabs_for_review -- --ignored
```

See [reference coverage](STUDIO_REFERENCE_COVERAGE.md) for the supplied screenshot
inventory and the next functional slices.

## Shared action dispatch

`paintcraft-ui::actions` defines the semantic identities, labels, shortcuts,
frontend availability, enabled states and tool check states for existing shared
actions. Both frontend menus and file/history toolbar controls consume it. The
desktop dispatcher validates enabled state again before execution and uses the
same action outbox for shortcuts, menus and toolbars. It flushes preceding edits
before checking later events in a coalesced frame, so Undo followed by Redo works
in arrival order.

New, Open, Save, Save As, Close, Undo, Redo, Select All and Deselect have command-key
shortcuts (Ctrl on Windows/Linux, Command on macOS). Shift+Command+Z also invokes
Redo. Save As does not fall through to Save. Focused widgets and active IME
preedit retain keyboard ownership; modal/file-dialog ownership blocks background
actions. This protects input routing; full system-font/IME canvas text is still
pending. Engine-only operations remain available through their typed commands
and CLI; native document-window actions have no headless equivalent.

Save All, recent files, printing, acquisition and the remaining editing commands
are not registered as working controls yet. The registry is a first slice for
existing actions, not a claim of complete menu or CLI coverage.

## Layer workflows

The Studio Layers menu exposes working add, delete, duplicate, visibility,
navigation, stack moves and a Layer Properties dialog through the shared action registry. Its labels,
shortcuts and boundary states follow the supplied `ima5ge5.png` reference where
implemented. Navigation selects a persistent layer ID without adding history;
stack moves retain that identity and add one recorded transaction. Duplication
selects the new copy only after a successful commit, preserves hidden/off-canvas
content and retains independent copy-on-write pixels. It appears in both the menu
and floating Layers window. The CLI can invoke the same native operation through
`apply` with a batch-v2 `layer.duplicate` description.

Executable UI tests cover menu and shortcut dispatch, one-entry undo/redo,
independent pixels, hidden copied layers, stable targeting on reorder and the
empty layer stack. The engine tests cover complete storage semantics and failure
rollback; these are not independent Paint.NET reference results. Layers remain a
partial menu: merge down, import and transforms are deferred.
The floating window still lacks layer thumbnails and reference-matched controls.

### Staged Layer Properties

F4, the Layers menu and the floating Layers window open the same modal dialog.
Name, visibility and opacity are staged locally. Apply submits one
`layer.set_properties` transaction, producing one history entry when values change;
applying unchanged values adds no history. Cancel, Escape and outside dismissal
publish nothing. Names use the existing 1024-character command limit.

The dialog captures the active tab, persistent layer ID and publication revision.
It rejects stale/foreign ownership before applying, and the existing modal guard
blocks document switching, canvas input, shortcuts, and workspace close while it
is open. Native save completion may still acknowledge its captured document;
dialog Apply creates a later revision. No live document preview is advertised.
The current engine supports the dialog's name/visibility/opacity fields; blend
mode selection remains deferred with broader layer/compositor support.

UI tests exercise staged multi-field edits, single-entry undo/redo, PCND save/reopen,
Cancel/Escape, unchanged Apply and invalid names. Unit coverage rejects stale
revisions and foreign tab ownership. The ignored native-GPU review test is:

```sh
cargo test -p paintcraft --test gui properties::render_layer_properties_for_review -- --ignored
```

Its output is `/tmp/paintcraft-layer-properties.png`. The supplied reference shows
the Layer Properties menu entry and F4 shortcut, but not the dialog contents;
this dialog is a functional native baseline, not an asserted reference capture.

## Colors window

Studio now has primary/secondary selection, swapping, a hue/saturation wheel,
preset colors and More/Less controls for exact byte RGBA and `#RRGGBBAA` input.
Left-click edits the selected swatch; right-click on the wheel or palette selects
and edits secondary. Wheel edits preserve the target swatch's alpha and choose
full brightness; byte channels/hex provide darker colors. Arrow keys on a focused
wheel adjust hue and saturation. This is the current native wheel policy using
ecolor's HSV conversion, not independently verified Paint.NET color semantics.

RGBA and hex controls retain hidden RGB at alpha zero. Incomplete/invalid hex text
stays local, leaves the color unchanged and resets on focus loss; its UI draft is
scoped to the document and swatch. Swap runs once per activation even when egui
repeats layout. Colors remain per-document tool preferences; changing them does
not publish document changes, affect dirty state or add history. Actual strokes
still commit through the shared editor. More/Less and selected-swatch state last
for the running workspace; palette customization and restart persistence remain.

Tests cover straight-RGBA fidelity, invalid hex input, wheel geometry, primary and
secondary ownership/alpha, no painting through the window, tab independence and
chosen-color stroke/undo behavior. An ignored native-GPU review capture is:

```sh
cargo test -p paintcraft --test gui colors::render_studio_colors_for_review -- --ignored
```

It writes `/tmp/paintcraft-studio-colors.png`. The wheel and expanded controls
address the Colors window in `im5age.png`; panel styling/spacing and advanced
control layout still differ. This does not qualify reference or color-managed
rendering parity. Right-button canvas drawing now works for the supported paint tools described below.

## Secondary-color canvas gestures

Studio Pencil, Brush, Fill, Line, Rectangle and Ellipse use secondary color when
started with the right button. Both-style shapes reverse outline and fill colors.
The swatches themselves stay unchanged. Gesture colors and options are captured
at pointer-down; unrelated button releases cannot publish the gesture. The same
staged brush path is used before and after publication, and a completed gesture
adds one undo entry. Escape, document switching and modal entry cancel safely.

See [pointer ownership](INPUT_GESTURES.md) for normalized input APIs, tests and
limits. Secondary selection, text and eraser behavior and Classic button policies
remain separate compatibility work; this is not full Paint.NET tool parity.

## View menu and pixel grid

Zoom In/Out, Zoom to Window, Zoom to Selection, Actual Size and Pixel Grid now
share the Studio action registry. Zoom shortcuts use Command/Ctrl plus/minus,
Command/Ctrl+B, Shift+Command/Ctrl+B and Command/Ctrl+0. Plus accepts the keyboard's
Shift modifier and the Equals-key alias. These shortcuts zoom the document;
egui's built-in whole-interface zoom shortcuts are disabled in Studio.

Fractional scaling allows zoom-out and fitted views. Actual Size remains one
physical screen pixel per document pixel at any display scale. Step zoom preserves
the visible document center; fit commands center the document or real nonempty
selection using the scroll area. Native zoom is bounded to 1/65536–64×; this range
and fit padding are current implementation policies, not qualified reference values.
Zoom changes cancel an active stroke so its remaining samples cannot use a changed
mapping. Text drafts are retained.

Pixel Grid is an overlay at magnification of at least four physical pixels per
document pixel. It draws only visible boundaries with one-physical-pixel strokes,
without scanning the document area or changing the rendered image buffer. Zoom,
grid and pending viewport centering belong to each document. None changes pixels,
selection, history, dirty state or PCND bytes. Rulers, units/DPI metadata,
pan tools and persistent viewport settings remain separate work.

Tests cover physical scaling, fractional drawing coordinates, actual size, fit at
multiple DPI scales, fitting/scrolling to a selection, menu/shortcut/preset dispatch,
per-tab ownership and exact history/PCND preservation. They also guard against a
scrolled canvas accepting clicks on the menu bar outside its visible clip.
The ignored native-GPU review test `view::render_studio_pixel_grid_for_review`
writes `/tmp/paintcraft-studio-pixel-grid.png`. These controls address `5image.png`;
visual and behavioral reference qualification remains incomplete.

## Shared SVG icons

Classic and Studio now use individual SVG adaptations of the supplied generated
icon sheet for tools, file/history controls and implemented menu actions. All 68
source glyphs have named standalone vector files and a local review gallery in
[the icon assets](../crates/presentation/paintcraft-ui/assets/icons/README.md). No raster image
is embedded in an SVG. Unimplemented tool/effect icons do not create placeholder
commands. The SVG renderer caches derived textures at the display's density;
accessible labels, action identities and selected/disabled behavior remain shared.

## Shared feature catch-up (2026-10-24)

The brush options now offer all nine brush media alongside round/square tips and
opacity. The Image menu exposes selection Crop, right/left 90-degree rotation,
180-degree rotation, horizontal/vertical flip and Clear Canvas. Clearing in Studio
uses transparent ground and remains undoable. The dedicated Shape selector exposes
all implemented preset figures while retaining Studio's own toolbar and floating
Tools window. Japanese settings, menus and window titles use the shared Fluent
catalogs and bundled Japanese fallback font.

The UI integration test selects every preset, checks action availability, clears
and restores the document, rotates it and restores it again. Further Studio work
includes image resampling, more advanced selections,
editable placed objects, effects and complete Paint.NET behavior qualification.


## Canvas placement (2026-10-24)

Image → Canvas size now opens a Studio-owned staged dialog with aligned pixel
fields and nine anchors. It preserves off-canvas content and raw layer pixels,
translates/clips selections, supports exact Undo/Redo, and leaves Classic startup
preferences alone. Earlier accepted edits finish before dimensions/revision are
captured; stale owners cannot receive a later Apply. English/Japanese and compact
light/dark layouts are exercised by real application GUI tests. See
[canvas semantics, validation and limits](STUDIO_CANVAS_SIZE.md). Resampling, skew,
automatic layer growth into newly uncovered margins and full native parity remain.

## Zoom tool rectangle workflow (2026-10-24)

Studio's Tools window now includes a Zoom tool with the plain `Z` shortcut.
Primary click steps in; secondary click steps out. A primary drag shows a temporary
rectangle and, on release, fits that document area inside the editing viewport.
The box retains subpixel document positions and supports either drag direction.
Current host zoom limits and scroll bounds still apply. English/Japanese tool
labels and a short usage hint distinguish this from Classic's Magnifier.

The primary workflow reference is the official
[Paint.NET View Tools documentation](https://paint.net/doc/latest/ViewTools.html),
consulted 2026-10-24 (page revision 2023-01-15). It documents click zoom, the `Z`
shortcut and fitting a dragged rectangle. No source code or artwork was copied.
Current step sizes, drag threshold and scrolling limits are PaintCraft policies;
exact per-version native interaction parity is not qualified.

The normalized view gesture lives in `paintcraft-tools::navigation`, separate from
paint gestures, selection and document commands. A host scope captures document,
tab, revision and viewport mapping. Document/view changes, disabled input, focus
loss and pointer loss cancel stale boxes. Foreign-button releases cannot complete
the owner's gesture. Four logical points separate a deliberate box from click
jitter. A one-document-pixel minimum fit extent keeps thin boxes finite.

The desktop shell renders the temporary box and applies only zoom/scroll state.
Neither drawing the box nor accepting it changes pixels, selection or history.
Escape during the box cancels the view gesture and retains pending canvas text.
Plain `Z` typed into an editable text draft continues to type text; Command/Ctrl+Z
retains Undo. Returning from Zoom to Text preserves the draft. Classic retains its
existing click workflow; this feature does not assume that Paint.NET's drag gesture
matches historical Microsoft Paint.

Validation: the full ordinary desktop GUI suite passes 254 tests (61 explicit
manual/capture tests ignored). Studio tests verify viewport fit, subpixel mapping,
reverse drags, DPI 1/2, English/Japanese, light/dark, forced multipass rendering,
no zoom before release, right-click stepping, unchanged selection/history/pixels,
Escape, focus loss, real document publication during a drag, shortcut ownership and
editable text preservation. Shared tool navigation tests and 29 ordinary shared UI
tests pass. All 68 browser library tests pass, including unchanged Classic
Magnifier behavior; the browser release distribution rebuilds. Workspace check,
strict all-target Clippy, translation, source-length and whitespace checks pass.
The final Studio caption also passes a focused strict Clippy check.

Actual app captures are in `assets/repo/studio-zoom-box-en-US.png`,
`studio-zoom-box-ja.png`, `studio-zoom-result-en-US.png` and
`studio-zoom-result-ja.png`. The English box and Japanese dark result were visually
inspected, and the English box was reviewed again after the caption correction.
Raw evidence is retained in ignored `docs/validation/studio-zoom-2026-10-24/`.
Pan, middle-button navigation and temporary spacebar pan are now implemented as
documented below. Overscroll is now available as described below; native
qualification remains unfinished. The browser currently has Classic only; these
captures do not establish Studio web parity or complete the Studio roadmap gate.

## Pan tool and temporary navigation (2026-10-24)

Studio's Pan tool now has an original standalone hand SVG, localized labels and
usage guidance, and the plain `H` shortcut. Dragging moves the view with the pointer.
Space-drag temporarily pans from other idle tools without changing the selected
tool; middle-button drag also pans, including while Zoom is selected. Open-hand
and grabbing cursors distinguish hover from capture. Text fields and editable
canvas text retain typing ownership, so `H` and Space continue to enter text.
Switching Text to Pan and back, or losing focus with Pan selected, retains the
editable draft without placing it.

The workflow reference remains the official
[Paint.NET View Tools documentation](https://paint.net/doc/latest/ViewTools.html):
it documents H, temporary Space-drag and middle-button pan from Zoom. Enabling
middle-button pan across the other idle tools is the current PaintCraft policy;
exact per-tool/version equivalence is not qualified. No artwork or code was copied
from that page. The hand vector is original repository-native SVG artwork.

The normalized pan state machine captures logical-screen displacement and its
owning button. Paint and selection state machines never receive a captured pan's
pointer events, including extra mouse buttons. Pan and Zoom now share an ordered
host routing path before ordinary painting. Escape, focus/pointer loss, tool
changes, document publication and view changes cancel capture. Cancellation stops
future movement and retains any scroll already performed. Native pan is currently
bounded by the existing scroll area, so small fitting images do not move.

Queued scroll requests carry document/view scope as well as the tab identity.
A replaced source or changed view cannot inherit an old pending pan. Independent
scroll-adapter tests reject stale requests and replace stale queued deltas rather
than accumulating them into a new scope. Studio's implicit touch-only drag-scroll
path is disabled so canvas gestures have one owner; native touch/pen behavior and
multitouch are not qualified by the mouse-input tests.

Validation: the full desktop GUI suite passes 257 ordinary tests (62 explicit
manual/capture tests ignored). The final focused Pan tests pass after strengthening
the zoom setup to send complete key-down/key-up pairs. They cover English/Japanese,
light/dark, DPI 1/2, forced multipass, exact screen displacement, one-frame
Space-down/drag/Space-up ordering, middle drag under Zoom, foreign mouse buttons,
unchanged pixels/selection/history and restored tool/cursor state. They also cover
Escape, focus loss, a real layer publication during capture, tool changes and
text-preserving H/Space input. Tool tests, 30 ordinary shared UI tests and the
Studio scroll-adapter tests pass. Workspace check, strict all-target Clippy,
translation, source-length and whitespace checks pass. All 68 browser library
tests pass and its release distribution rebuilds; the browser still exposes
Classic only.

Actual light English and dark Japanese captures are in
`assets/repo/studio-pan-en-US.png` and `studio-pan-ja.png`. Both were visually
inspected. Their fixture asserts a scrollable canvas before capturing the active
Pan tool at 1600 percent with an edited pencil stroke. These captures do not prove
OS cursor appearance or native Paint.NET parity. Raw logs are retained in ignored
`docs/validation/studio-pan-2026-10-24/`.

Overscroll is implemented in the following slice. Space-plus-arrow panning,
Ctrl-accelerated keyboard pan, automatic
edge scrolling and per-device/native qualification remain unfinished. This does
not complete the Studio or browser parity gates.

## Overscroll and navigation preference (2026-10-24)

Studio overscroll now defaults on. Small images can move halfway offscreen;
images larger than the viewport can move until either edge reaches the viewport
center. The policy applies independently on each axis, including images that are
small horizontally and large vertically. The workflow reference is the official
[Paint.NET View Tools documentation](https://paint.net/doc/latest/ViewTools.html).
The geometry implements that documented range without changing canvas dimensions
or allocating pixels for the surrounding workspace. Exact native qualification
remains separate from this implementation evidence.

Settings now has a Studio-only User Interface page. Scroll past the edge of the
image (overscroll) is staged until Apply; Cancel retains the prior preference and
view. Disabling it restores the constrained scroll range. Enabling it preserves
the visible center; resizing the viewport preserves the image's displacement from
that center. Applying the preference does not change pixels, selection or history.
Classic retains its own canvas presentation and has no overscroll setting here.

The existing settings/v3 file gains an optional `studio_overscroll` boolean.
Older v1/v2/v3 files default to enabled, disabled values round-trip, and malformed
types take the existing preserved-unusable-file path. Settings are still written
atomically. Pending pan requests include the preference in their document/view
scope, so queued displacement cannot cross an overscroll policy change.

Studio's scroll adapter now lays out the canvas inside an explicit virtual extent,
with leading/trailing room for the overscroll range. Layout changes preserve the
screen-space point under the viewport center; explicit zoom targets remain owned
by the shell. The adapter accounts for reserved scrollbar space and retains the
existing scoped pan-request admission. Physical touch/pen behavior, kinetic
navigation and native reference rendering are not qualified by these tests.

Validation: 260 ordinary desktop GUI tests pass (63 explicit manual/capture tests
ignored). New tests check exact small/large endpoint geometry, both directions,
English/Japanese, light/dark, DPI 1/2, forced multipass, unchanged document/history,
staged Cancel/Apply, disabled behavior after restart, re-enabling without a center
jump and window resize preservation. A 360 by 360 high-DPI settings test verifies
that the checkbox and footer remain reachable in both languages. Existing Studio
pan/zoom and workspace workflows pass. Four Studio library tests cover geometry
and scoped scroll requests; settings parsing/round-trip tests pass. All 68 browser
library tests pass and the browser release distribution rebuilds. Workspace check,
strict all-target Clippy, translation, source-length and whitespace checks pass.

Actual offscreen app captures are in `assets/repo/studio-overscroll-en-US.png`,
`studio-overscroll-ja.png`, `studio-navigation-settings-en-US.png` and
`studio-navigation-settings-ja.png`. The English panned canvas and Japanese dark
settings surface were visually inspected. The fixture verifies that the small
canvas extends outside its viewport before capture. Raw evidence is retained in
ignored `docs/validation/studio-overscroll-2026-10-24/`.

Space-plus-arrow panning, Ctrl-accelerated keyboard movement, automatic edge
scrolling and native/per-device qualification remain unfinished. Studio web
presentation and the complete Studio roadmap gate remain unfinished.
