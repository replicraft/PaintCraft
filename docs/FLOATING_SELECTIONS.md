# Moving selected pixels

Classic 7, 10 and 11 and Studio can move a rectangular or free-form selection.
Press inside an actual selected pixel and drag. Holes in a mask do not grab the
selection. The selected field, target layer, original pixels, clear color and
transparent-selection setting are captured at press. Previewing never publishes
pixels, changes the session selection or adds history.

Classic places the pixels on release. Studio keeps a floating draft after release;
repeated drags reuse the original capture. Enter places it, and Escape cancels it.
Clicking outside or choosing another editing action places a Studio draft first.
Undo or Redo cancels an active draft before navigating older history. Repeated
layout passes cannot turn that cancellation into an older undo step. Desktop view zoom
preserves a draft. Browser zoom cancels a held move. Moving back to the original position is an exact no-op.

Desktop file actions finish the draft before capturing the ordered document.
Browser Classic refuses Open, New, Save and Export while the pointer still owns a
move. Releasing or cancelling the drag makes those actions available again.
Changing the source revision, tool or target invalidates an old capture. A foreign
pointer-button release cannot place it.

## Pixel policy

Movement cuts the whole selected source field and pastes from one immutable
snapshot at an integer offset. Classic clears the source to its canvas color;
Studio uses its captured eraser policy. Paste overwrites selected destination
samples, including exact alpha and hidden RGB. Destination holes preserve the
original target pixels, subject to the original source cut where they overlap.
Other layers and their metadata remain unchanged. Selected samples outside the
finite target layer are transparent. Off-canvas layer storage is preserved and
expanded where required, while the visible selection is clipped to the canvas.

Transparent selection skips paste samples whose RGB matches Color 2. Alpha is
ignored for this comparison; matching samples are still removed from the source.
This is an explicit initial color-key policy, without a claim of matched native
Paint fidelity. Changing Color 2 or the setting during a drag does not change its
captured behavior. The selection field itself does not lose keyed pixels.

## Commands and resource ownership

The strict v2 operation is `raster.move_selection`, with `layer_ref`, integer
`offset: {x, y}`, `clear` and optional `transparent_key` colors. V1 rejects it.
The editor retains geometry once through compilation and replay, admits decoded
output before publication, and records one undoable change. Late validation,
resource denial, cancellation and stale publication discard private results.
A zero offset validates the active selection and target but needs no geometry or
pixel admission and creates no history.

Color-key classification creates a separately admitted frozen paste mask; it does
not reread a progressively edited destination. Source classification and replay
share the cumulative source-read bound. Temporary run capacities use paint
metadata admission. Clipped and translated selection views share mask storage.
Session selections remain outside PCND persistence.

## Evidence and remaining scope

Raster tests compare 288 independent dense cut/paste cases against both logical
and decoded output. They cover mask holes, overlap, clipping, RGB keys, hidden RGB,
sparse tile seams, cumulative bounds, cancellation and ownership. Engine tests
check compound edits, replay, no-op identity, atomic rollback and recorded history.
CLI tests execute 216 actual apply-and-reopen cases and check exact archive
identity for no-ops and sentinel preservation on late failure.

Native controller and GUI tests cover release, repeated Studio drags, explicit
placement, cancellation, color capture, stale sources, pointer ownership, view
changes and duplicate Undo activations. The existing 28 native skin/language
profiles exercise the transparent-selection checkbox. Browser tests use actual
pointer events across Classic skins and languages, compare every raw pixel with
an independent oracle, and check file, text and source guards.

Custom-angle rotation, fractional mask combination, interactive scaling, clipboard,
matched native fidelity and physical device/latency qualification remain separate
work. This tranche does not complete T14, T15, T19 or the full selection acceptance
gates. Raw local logs are in ignored
`docs/validation/floating-selection-2026-10-24/`.

## Completed validation, October 24, 2026

The complete workspace sweep exercised every package and recorded 1,470 passing
tests, 75 explicit ignores and two failures in older UI expectations that still
required Transparent Selection to be disabled. Those two tests were updated to
verify the enabled control, captured help, popup closure and unchanged document
state; their historical 28-profile and modern four-profile reruns both pass.
The original sweep and successful reruns remain separate logs. Three additional
worker/feedback regressions and the native controller lifecycle tests are included
in the sweep. These figures describe regression evidence, not native device or
interactive performance qualification.

Strict all-target workspace Clippy passes after the test updates. Browser Wasm
Clippy, the actual browser release build, formatting, the 800-line source limit
and all 933 message mappings across seven languages pass. Python bundle tests
pass with the opt-in native-event test skipped. The debug macOS bundle was rebuilt
and passes strict deep signature and plist validation; its editor linker UUID
matches the current native build. No physical printer job was submitted.
