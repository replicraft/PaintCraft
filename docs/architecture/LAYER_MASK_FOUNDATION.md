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

[Selection Snapshot1](SELECTION_SNAPSHOT_TRANSPORT.md) now provides a locally
qualified private binary-selection transport with a borrowed cursor and same-scope
resumable decoder. Its 112-test Raster gate includes 13 new groups; this is a
separate scope from the earlier counts. Actual browser transport/mutation and
bounded asynchronous finalization remain open; the foundation does not activate
Web mask controls or extend Source7.

History's private memo now retains a proven table-allocation bound through
reserves and deletions. Memory weighting keeps the existing live-control allowance,
and reclamation uses the tracked size so tombstones cannot hide an oversized
table. Six new regression groups pass with the existing history and selected
Editor history suites. This remains a synchronous prerequisite: immutable history
leases, resumable pressure handling and bounded cancellation cleanup are still open.

Appearance-aware foreign export consent, actual OS print/Clipboard publication
and asynchronous browser mask editing remain open. Palette's full 132-filter
catalog, browser blend adoption and layer styles remain separate roadmap work.

## Browser Studio own-mask actions

Studio now wires selection, invert, enable/disable and remove controls to captured
Main editor operations. Owner, document, publication epoch and stable layer checks
run before private tool state can be discarded. The existing bounded transaction
owns mask construction and history publication; each change has one undo.
Semantic no-ops preserve both the editor ticket and desired display identity.

A committed change invalidates the old display identity and reopens a Normal
Source7 snapshot. Existing renderer close/ACK handling retains its credits until
retirement. No mask intent enters Source7's numeric-preview protocol. NonNormal
documents remain explicitly refused until the Source8 browser lane is available.
Statuses and guard messages are translated in all seven configured languages.

Eight targeted native tests and the complete web Rust unit suite pass: 485 passed
and three existing ignored. Native all-target and Wasm library strict Clippy,
350 Node regressions, translation, formatting and source-length checks pass.
The PCND7 roundtrip test uses explicit coverage admission; its earlier source-only
generic-reader draft is preserved. The pnpm retry retained signature verification
and succeeded after network access was allowed.

Evidence is under
`docs/validation/layer-blend-candidates/web-own-layer-masks/native-qualification`.
Actual Main browser UI execution is pending. This uses the existing synchronous
Main transaction contract; it does not qualify asynchronous history finalization,
complete physical memory accounting or latency.
