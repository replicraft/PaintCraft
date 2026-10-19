# PaintCraft Studio

Studio is the current development priority. Its first presentation slice lives in
`paintcraft-studio`, with the desktop shell supplying the editor and native services.
Select **Studio** in the application header; the choice is saved for the next launch.
Selawik remains the interface font. RetroPaint's selected font, W95FA, is recorded
separately in the dependency choices and is not bundled yet.

## Implemented presentation slice

- Compact File/Edit/Window menus, file and undo/redo toolbar, and tool options.
- Movable Tools and Colors windows, with real tool selection, primary/secondary
  colors, palette, brush size/tip, shape style, fill tolerance and bitmap text size.
- A movable/resizable Layers window using the existing add, select, visibility,
  rename, opacity, reorder and delete operations. Window menu toggles restore panels.
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

1. Multiple document tabs with independent editor sessions, bindings, dirty state,
   recovery ownership and close/save handling.
2. Recorded history labels and a functional History window, with exact undo/redo
   navigation and explicit trimmed-history presentation.
3. Image/layer editing operations and richer selection interactions through typed
   commands, with clipping, atomic failure and history coverage.
4. Region-aware adjustments/effects, followed by qualified GPU implementations.
5. Shaped system-font text and IME interactions; broader codecs and PDN compatibility.

This slice is not Paint.NET parity or a completed T19/M3 gate. There is one active
document; no document tabs, History window, effects UI or PDN importer is advertised.
Reference-build captures, the remaining engine prerequisites and native qualification
remain separate acceptance work. Tests cover saved experience migration, preservation
on switching, floating-window input ownership, brush undo/redo, multipass action
delivery and reopening Layers. An ignored native-GPU test renders a review image:

```sh
cargo test -p paintcraft --test gui studio::render_studio_workspace_for_review -- --ignored
```

The image is written to `/tmp/paintcraft-studio-workspace.png`; it is a review artifact,
not an independently captured reference or cross-platform visual-parity result.
