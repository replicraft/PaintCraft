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

Browser Source7 rendering, its lifecycle/resource accounting, appearance-aware
foreign export consent and asynchronous browser mask editing remain unfinished.
No browser mask controls are activated by this milestone. This does not establish
Palette's full 132-filter catalog, blend-mode or layer-style parity.
