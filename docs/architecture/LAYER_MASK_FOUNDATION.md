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
retirement. No mask intent enters Source7's numeric-preview protocol. That initial
checkpoint refused NonNormal documents; the later activation below supersedes it.
Statuses and guard messages are translated in all seven configured languages.

Eight targeted native tests and the complete web Rust unit suite pass: 485 passed
and three existing ignored. Native all-target and Wasm library strict Clippy,
350 Node regressions, translation, formatting and source-length checks pass.
The PCND7 roundtrip test uses explicit coverage admission; its earlier source-only
generic-reader draft is preserved. The pnpm retry retained signature verification
and succeeded after network access was allowed.

Evidence is under
`docs/validation/layer-blend-candidates/web-own-layer-masks/native-qualification`.
At that checkpoint, actual Main browser UI execution was pending. This uses the existing synchronous
Main transaction contract; it does not qualify asynchronous history finalization,
complete physical memory accounting or latency.

## Actual browser actions and blended-layer activation

The production browser now passes six Normal-layer action groups: a real
selection drag creates the independent two-pixel mask; repeated creation is a
no-op; inversion, disabling, removal and Undo/Redo preserve saved coverage and
raw raster identity. A real Invert preview is cancelled and then applied while
the mask is present. Saved PCND1/7 semantics and full Source7 RGBA agree with
independent literal fixtures. Apply deliberately promotes its validated preview
texture; its covered screenshot pixels and subsequent stored-filter Undo/Redo
are checked. Downloads remain dirty after edits. These groups also pass Linux CI.

Dedicated own-mask actions now accept Normal, Multiply and Screen documents in
Studio. Only two Normal-specific eligibility guards were removed. Captured owner,
state, publication epoch, selected target, private-input and transaction admission
checks remain. Ordinary browser construction and the old snapshot, private filter
and derived consumer guards are unchanged. An actual defining CurrentStamp8 test
rejects the retained old source after a mask change and after Undo/Redo ABA.

All 550 native web tests pass, with three existing ignores, plus strict native
all-target/Wasm checks and documentation. Independent Normal/Multiply/Screen
pixels, PCND persistence, no-op, history, stale/private-input and admission
refusal controls cover this path. The initial new test had an optional-target
indexing error; its failed build and the corrected test are retained separately.

Five actual browser groups on a two-pixel Multiply scene verify PCND8 semantics,
all RGBA bytes, repeated no-op and mask/history operations. Actual Source8 OPEN
bytes match each downloaded project's SHA. Every adjacent source Worker closes
and terminates before its successor is created, including the selection
transition. The Main uses its original application callbacks and factories;
the test does not replace them or inject a grant. Screenshots are retained,
without automatic semitransparent physical-pixel acceptance. Native Screen
coverage is separate from this finite Multiply browser scene.

The fresh normal production build passes with 6,804 nonignored repository file
entries unchanged during the build. Main is 22,189,865 bytes; Worker and Codec
artifacts are byte-identical to their prior qualified versions. Public Main
declarations remain exact; generated glue changes only internal closure indices.
The five Main and six Normal-mask regressions also pass against this Main, and
all 402 Node tests pass. This does not establish hidden numeric S/C balances,
physical memory, GC, latency, filter-mask controls or mask painting.

Evidence is under `docs/validation/browser-mask-runtime/`,
`docs/validation/browser-blend-mask-activation/`,
`docs/validation/browser-blend-mask-production/attempt-1/`, and
`docs/validation/browser-blend-mask-runtime/blend-mask-VK4kTo/final.json`.
The initial missed toolbar drag, inclusive-endpoint calibration, incorrect fresh
Apply-frame expectation and incorrect two-row button offset remain preserved.
