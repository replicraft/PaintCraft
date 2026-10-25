# Flatten Image

Studio's **Image > Flatten image** command replaces a multi-layer document with
one layer containing its visible canvas composition. Ctrl+Shift+F (Command+Shift+F
on macOS) invokes the same action. The command is disabled with fewer than two
layers, in Classic, or while a modal owns the workspace.

The output uses the existing CPU compositor, including root order, layer opacity,
visibility, origins and clipping. Hidden layers and off-canvas pixels leave the
current document. Undo restores all original layers, names, origins, raw pixels,
visibility and opacity; Redo restores recorded output without rendering again.
Canvas size, document identity, title and selection are retained. The new active
layer covers the canvas, is visible and has full opacity. Its first pixel becomes
the sparse default, retaining uniform canvases without materializing uniform tiles.
With zero or one layer, the engine returns an uncommitted result without invoking
the compositor or adding history.

The host supplies composition from the transaction's immutable source. The editor
validates the output extent, admits the complete decoded tile peak before ingest,
and records one history entry before publication. Native work runs on the existing
ordered editing worker; prior edits run first and later Undo sees the flattened
source. Worker publication checks the session owner, state, epoch and resource
policy. Cancellation, rendering failure, malformed output and tile-admission denial
publish nothing. Rendering retains its existing output limit; aggregate physical
memory and large-image latency are not newly qualified by this feature.

Desktop and browser Studio resolve editable text before requesting flattening;
active IME composition prevents that transition. Selection is preserved and does
not restrict the composition: flattening operates on the complete visible canvas.
Classic does not gain a new menu command. All seven interface catalogs contain the
Studio action label.

The CLI writes a native project:

```sh
cargo run --release -p paintcraft-cli -- flatten source.pcnd -o flattened.pcnd
```

Existing destinations require `--overwrite`. `--expect-input-sha256` checks the
captured source before parsing. In-place output also uses the existing atomic
replacement guard against external changes after capture. Flattening is an explicit
editing operation; ordinary image export continues to require its separate
conversion-loss acknowledgements.

MCP exposes `flatten_image` on its current headless document. It returns the engine
receipt, supports `history` Undo/Redo and does not save automatically. This is a
session operation, not an addition to the batch v1/v2 wire grammar or PCND format.

## Validation scope

- `paintcraft-editor/tests/flatten.rs`: independent dense composition oracle,
  selection/title/identity, complete Undo/Redo, exact one-step history, no-op,
  invalid output, rendering failure, decoded denial, every observed cancellation
  checkpoint and stale/foreign/resource-policy publication guards.
- `paintcraft` controller tests: synchronous and actual native worker execution,
  target adoption, FIFO ordering and recorded navigation.
- `paintcraft/tests/gui/flatten.rs`: real egui menu and shortcut events with one
  and two layout passes, exact output, Undo and disabled single-layer invocation.
- Browser editor tests: shared transaction, target adoption, pixels and Undo/Redo.
- Real CLI/MCP process tests: PCND round trips, deterministic output, file/hash
  guards, tool discovery, no-op and complete layer restoration.

These checks establish this feature's behavior. They do not establish Paint.NET
reference parity, native pen/IME fidelity or full browser/platform qualification.
