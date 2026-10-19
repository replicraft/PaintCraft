# PaintCraft Studio

Studio is the current development priority. Its first presentation slice lives in
`paintcraft-studio`, with the desktop shell supplying the editor and native services.
Select **Studio** in the application header; the choice is saved for the next launch.
Selawik remains the interface font. RetroPaint's selected font, W95FA, is recorded
separately in the dependency choices and is not bundled yet.

## Implemented presentation slice

- Compact File/Edit/Layers/Window menus, file and undo/redo toolbar, and tool options.
- Document tabs with independent editors, histories, targets, tools, colors, zoom,
  scroll positions and native-file bindings. New/Open/recovery preserve other tabs.
  Close supports save, discard and cancel; closing the last tab leaves an empty workspace.
- Movable Tools and Colors windows, with real tool selection, primary/secondary
  colors, color wheel, swap, palette, expanded RGBA/hex controls, brush size/tip, shape style, fill tolerance and bitmap text size.
- A movable/resizable Layers window using the existing add, select, visibility,
  rename, opacity, reorder, duplicate and delete operations. Window menu toggles restore panels.
- A movable History window with recorded action labels, current/redo rows, direct
  snapshot navigation and an explicit notice when older entries were trimmed.
- Centered canvas when the viewport is larger than the document; scrolling and
  actual-pixel zoom retain the common mapping.
- Shared staged brush previews, CPU/wgpu composition, transactions and undo/redo.
  Open, Save As and Export retain the native file-dialog and preservation paths.

Frontend controls/actions live in `paintcraft-ui`. Studio has no dependency on the
Classic presentation crate. The shell executes actions through its existing outbox;
the presentation never writes files or mutates published document state directly.
Clicks in floating windows cannot start strokes on the canvas underneath them.

Settings v2 saves the active experience independently of the Classic version.
V1 settings migrate to Classic with their original version retained. Launching a
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
