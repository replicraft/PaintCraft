# Saved layer masks

Updated 2026-11-01. This is a partial Palette integration milestone.

A layer can retain an optional grayscale coverage mask independently of the live
selection and each filter's mask. Studio can capture a selection as a layer mask,
enable or disable it, invert it within its saved domain, and remove it. Source
pixels stay intact. An active empty selection creates a real zero mask; an
inactive selection cannot be captured. Each successful change is undoable.

## Document and rendering behavior

- Enabled masks modify the filtered layer's alpha before layer opacity.
- Pixels outside the saved domain, including sparse gaps, have zero coverage.
- Disabled masks retain their data and editability while having no visual effect.
- Geometry operations preserve or transform mask coordinates under their contracts.
- PCND7 stores masks, including disabled and empty masks. Documents without layer
  masks retain the previous minimum-version encoding.
- CPU composition and GPU presentation share the same appearance semantics.
  This does not introduce a GPU filter kernel.
- Foreign project/image export refuses unsupported preservation or baking rather
  than silently discarding mask intent. Explicit flattening remains destructive.

CLI batch operations and four MCP tools use the same transactional editor path.
Native Studio prepares mask changes through the editing worker, waits for accepted
predecessor edits and rejects stale document/layer captures. Canceling the wait
keeps the accepted predecessor intact. Capturing or inverting coverage runs off
of the UI thread.

## Verification and remaining scope

Local foundation qualification passed 1,512 native tests with 30 ignored, including
53 actual Metal presentation tests. The subsequent command/automation slice passed
809 native tests with 15 ignored and six Python schema groups. These are separate
qualification scopes; their counts must not be added as distinct test coverage.

The UI slice passed 579 Tools/Studio/Web package tests with three ignored, then
seven targeted native tests, including five new real-control/worker tests.
Workspace all-target and actual Main/codec/render Worker Wasm strict checks,
formatting, seven-language translations and the source-length check passed.
Earlier module-registration, trait-import and spinner test failures are preserved.

Evidence lives in `docs/validation/layer-mask-integration/final-freeze.json`,
`docs/validation/layer-mask-actions-integration/final-freeze.json` and
`docs/validation/layer-mask-native-ui-integration/final-freeze.json`.

Browser Source7 has a locally qualified rendering and visual-consumer checkpoint.
The fresh production build binds 4,579 source inputs, 55 artifacts and 28 assets.
Actual Chrome passes 26 independent straight-RGBA comparisons, two independently
decoded PNG outputs and 23 refusal/lifecycle controls. Two 1000 by 486 sessions
each complete and release 20 views on one OPEN, including real panning. Earlier
view-8/view-11 exhaustion failures are retained with their original evidence.

Three actual application cases pass stored-mask display/save, private Invert
Cancel/Apply/Undo/Redo, Image sampling proved through a Pencil edit, and Flatten
with Undo/Redo. Independent project decoding checks source pixels, masks and
metadata. Captured caller work stays cumulative, transferred quotas are consumed
once, and retained memory survives uncertain Worker retirement. The native and
mock transport checks cover additional ownership boundaries; these browser cases
are finite correctness checks, not latency, RSS or physical-GPU measurements.
The aggregate receipt is
`docs/validation/source7-complete-consumer-final-freeze.json`.

Appearance-aware foreign export consent, actual OS print/Clipboard publication
and asynchronous browser mask editing remain open. Browser mask mutation controls
remain closed. Palette's full 132-filter catalog, blend modes and layer styles
remain separate roadmap work.
