# Scoped native IME lifecycle handling

The normal macOS owned desktop loop now preserves actual winit IME Enabled and
Disabled receipt boundaries that pinned egui-winit0.37 omits. Preedit, Commit,
keys and other input retain the official translator. A narrow compatibility
bridge uses egui's deprecated lifecycle DTOs without a vendor patch or backend
change; it does not invent native composition IDs or sample ordinals.

Enabled describes facility availability, not an active composition. It arms only
the current canvas text session, document publication, interface, target and
revision; it allocates no provisional text and does not exclude ordinary keys.
Accepted local text edits can advance that same scope. Focus, widget/modal/picker,
owner or draft changes revoke it. A foreign or ineligible Enabled cannot authorize
a later canvas result, even when no previous canvas binding existed.

Disabled removes provisional composition without publishing its committed base,
clears shortcut exclusion and refuses a late result. A fresh eligible Enabled
allows direct Commit for its exact canvas draft; fresh Preedit also recovers.
Initial direct-Commit compatibility remains when no revoked or foreign lifetime
has been observed. Duplicate Enabled does not erase active preedit. Allocation
failure retires unpublished ingress and composition without changing committed
document/history or introducing a new resource pool or larger limit.

## Executed evidence and limits

Focused native tests passed: **17 canvas IME tests and three desktop tests**, with
no failures or ignored tests in those two filters. This includes the ten new
causal groups. The real normalization helper feeds the actual canvas controller;
checks cover stale/foreign results, focus and widget ownership, recovery,
committed-source preservation, and one text publication with exact Undo.

The actual same-frame F11 matrix checks Preedit→Disabled→key unlock,
Disabled→Preedit→key block, and duplicate Enabled retaining active-preedit
exclusion. This tests receipt-time shortcut exclusion for a nonmutating view
operation. It does not replace the existing global-input first pass and later
canvas replay, or establish complete mixed pointer/text/action ordering.

These are headless controller and synthetic winit-normalization tests, not an
actual OS input trace, visible native window, physical IME/tablet run, latency
measurement or cross-platform delivery qualification. Windows/Linux still use
their existing shell routes; their lifecycle delivery is not qualified here.
The combined desktop library suite also passes 892 tests with 36 existing or
explicitly gated ignored cases. Strict native all-target and Main/both-Worker
Wasm checks pass on the same source. The scoped aggregate receipt is
`docs/validation/native-ime-lifecycle-root/root-final.json`.
These checks do not establish physical delivery or complete input ordering.

T02, T14 and T15 remain partial. Their original shell/comparison, physical input,
actual-pixel, complete tool/IME and named M2 acceptance requirements remain open.
See [Tablet input](TABLET_INPUT.md) and [Roadmap status](ROADMAP_STATUS.md).
