# Complete built-in batch preparation and decoded admission

The editor now compiles a complete built-in batch before replaying decoded paint.
The public execute/session/native/CLI paths use this preparation; the old executor
is retained only in benchmark support and differential tests.

## Preparation and logical sources

The compiler runs the existing geometry, work-limit, clipping, alias and metadata
validation against a private structural document. Painting produces immutable
ordered descriptors instead of changing that document's pixel buffers. Structural
operations update the shadow layer tree, origins, dimensions and selection in
their original order. A replay record captures its original operation index and
resolved layer ID; geometry is constructed once.

Batches containing preparation-time pixel reads retain source-bound logical
programs. Flood fill sees earlier paint, including branches captured by duplication,
old bounds preserved across expansion, and the current origin/selection. Source
metadata is copied fallibly under admission while immutable pixel buffers remain
leased. Batches that only construct writes omit these logical pixel views: their
ordered blending still reads the destination during replay. An exhaustive command
classification must be updated when new operations are introduced. Dependency
scanning and source/model/geometry walks observe cancellation checkpoints.

Empty plans and operations with no indexed writes need no replay record. This
proof uses the same sparse/default index rules as actual raster replay. An indexed
write is not necessarily a semantic pixel change; existing-tile no-ops retain the
normal COW/admission behavior.

## Tile ownership model

The batch model tracks resident coordinates and ownership, without allocating
decoded tiles or reading pixels. Captured input versions require COW on their
first write. A new private version is reusable until duplication shares it; a
later write to a shared version needs another allocation. Deleting a layer or
clearing the canvas releases a simulated version when its final owner disappears.
Expansion preserves partial edge-tile ownership and allocates no pixels by itself.

The model separately tracks:

- Live new versions and their highest simultaneous count.
- Cumulative allocations, which are not replenished by deletion.
- The first operation that establishes the reported maximum peak.

The cumulative count checks the existing staged-pixel allowance during planning.
The peak prices new output capacity in the decoded realm; existing immutable
input payload is already charged separately. A later allocation can reuse capacity
freed by an earlier deletion without resetting the cumulative work allowance.
Native preparation runs this model serially. Unusual external reader/caller races
can make lower-level estimates conservative; this is not a minimal physical plan.

## Reservation, replay and publication

After compilation, the compiler releases logical source/model storage and retains
ordered original paint records and their actual admitted replay programs. It reserves the complete known new decoded
peak before replay. A quota denial therefore admits no decoded prefix, including
when earlier commands logically painted other tiles. Fixed staged-limit failures
are attributed to the operation that crosses that limit; decoded peak denials
carry the operation establishing the reported peak.

Replay starts from the captured base and replays structural commands and compiled
paint records in original order. Each operation keeps the ordinary checked row
kernel and strict reservation semantics. Runtime invariant checks reject leftover
compiled paint instead of publishing a partial schedule. The session still prepares
recorded history and validates its owner/base ticket before atomic publication.
Undo/redo restores recorded results rather than rerunning geometry or fonts.

Allocator failures, caller interference, cancellation and non-paint metadata
pressure may still fail private replay. Ordinary compiled paint reuses its actual
simulation-created index, validates target bounds/default/ordered sparse rules
without allocating, and does not build another index after decoded admission. All private output and unused reservations
are discarded; published state/history remain unchanged. A reservation is not a
guarantee that malloc succeeds or that a caller sharing it cannot consume capacity.

Source/model nodes, model-map/index payload, logical programs, compiler arrays and
reader scratch carry shared metadata accounts and fixed container limits. Known
metadata retries retain their own reservation policy. Flood bitmap/frontier/output and checked stroke unions now use the exact caller
metadata scope as well as their unchanged fixed per-call scratch bounds. Returned
flood spans retain their original account/factory through descriptor construction.
The ordinary compiled-paint replay barrier holds its actual descriptors/indices
while admitting the complete decoded peak; failures retire those owners before a
native capacity wait. Exact previous descriptors and current modeled target classification can share
one immutable index/program in the same original caller factory, before another
index is built. Every operation still simulates COW and blends in order. Unique
programs retain their own index payload and full aligned Arc/program control
accounts; repeated handles do not re-charge shared storage. Distinct programs
can still increase preparation metadata compared with the earlier temporary-index
policy; no speedup or unchanged work/callback count is claimed. This does not provide a complete upfront
geometry/font/mapped-transform/coverage/history/GPU/physical-memory admission plan. The shadow document and immutable
command input retain their existing metadata policies. Native worker scheduling,
fairness and input/display/cancellation latency remain separate qualification work.

## Executed scope

The current grammar covers fill rectangles, pencil, smooth/unsmoothed strokes,
flood, lines/rectangles/ellipses, resolved masks and built-in text, plus existing
layer/canvas/selection metadata operations. Future effects or new command families
need their own preparation and resource rules; this does not qualify them in advance.

Differential tests compare all painting families and 64 mixed structural sequences
with the source-pinned executor, including exact document semantics, aliases,
selection and decoded peaks. Tests cover zero-prefix whole-batch denial, recycled
peak versus cumulative limits, every observed cancellation checkpoint, borrowed
source capture/COW, metadata interference during real private replay, and native
FIFO/file barriers with repriced whole-attempt demand. Earlier geometry failure
tests now assert preparation fails before any decoded admission.

The batch-preparation benchmark separately compares real editor execution and
parent/current CPU-only CLI apply/history/file workflows. Exact library semantics
are cross-checked with independent raw PCND pixel reconstruction, while complete
native archives verify publication identity. Raw statistical/process samples,
metadata/decoded counters, calibration and slow controls remain required before
any performance claim. Physical RSS ceilings and native latency are not inferred
from these correctness or library measurements.

The [completed matched experiment](../docs/benchmarks/batch-preparation-2026-10-22/DECISION.md)
retains 280 statistical samples and 1,860 original process observations. Exact
outputs and decoded peaks match, but diagonal and mixed-source batches regress
substantially and retain more metadata. This is a correctness adoption; reducing
those costs is separate performance work. [Source-pinned validation](validation/batch-preparation-2026-10-22/validation.json)
records executed checks and preserves development failures.

[Exact descriptor sharing](PAINT_PLAN_SHARING.md) now reduces retained payload for
repeated equal plans without merging operations. Original timing evidence remains
pinned to its pre-sharing source. The [matched follow-up](../docs/benchmarks/plan-sharing-2026-10-22/DECISION.md)
retains memory benefits, mixed timings and slow controls separately.


## Joint paint admission checkpoint

Ordinary compiled paint now retains its exact replay index and original metadata
factory before admitting the complete decoded peak. Exact repeated programs can
share immutable descriptors, indices and fully charged program controls. Each
operation still performs its own simulated ownership changes and actual blend.
Flood and stroke scratch retain their original accounts through handoff/use,
with storage released before its charge. The independent per-call bounds remain.

The final local run passes 633 Raster/Editor tests and 892 desktop library tests,
with 36 existing or explicitly gated desktop cases ignored. Strict native
all-target, Main/both-Worker Wasm and Core/Raster/Editor rustdoc checks pass. The
original repeated-stroke 8× bound, explicit 204,800-byte case, and native
257-pixel stroke under 4,096 bytes all pass without raising their limits.

Last-run coalescing preserves the unordered geometric union before descriptor
construction. A separately declared native-shaped square-stroke case uses two
genuine same-profile benchmark artifacts and an independent complete-image
summary. Logical metadata falls from **8,448 to 2,784 bytes (67%)**; decoded
payload remains 524,288 bytes and full pixels match. This case transplants the
native stroke shape onto the benchmark's 1,000×1,000 document; the original
native 257×1 retry fixture separately passes unchanged. The original seven
benchmark cases retain identical hashes, decoded peaks and logical metadata
peaks before/after coalescing. These are accounting observations, not CPU speed,
allocator overhead, RSS, physical-device or release qualification.

Earlier failed attempts are preserved. In particular, retaining every repeated
index exceeded the original 204,800-byte limit; exact shared programs corrected
that cost. Newly admitted row scratch exposed the 4,096-byte native test's
8,448-byte demand; coalescing removes redundant runs instead of raising the cap.
The benchmark setup failure lacked its required fixture environment; the rerun
uses authenticated archived fixtures in a fresh directory.

The aggregate receipt is
`docs/validation/t04-t11-joint-admission-root/root-final.json`
(SHA-256 `c9be9442a7c76550ffb4e3f61dce575d3e372583d40b0687a508a40b70857f3d`).
Genuine before/after artifacts and policies are retained under that directory.
Browser runtime requalification now passes on the fresh normal website build;
see the [fresh browser checkpoint](#fresh-browser-checkpoint).
Mapped transforms, fonts, coverage/history bookkeeping, other services/GPU,
allocator/reallocation controls and physical memory remain unfinished. T04 and
T11 remain partial; this is not a complete upfront all-resource request plan.

## Fresh browser checkpoint

The normal `pnpm --dir apps/paintcraft-web build` at `ce32aa1f` preserves all
3,986 selected source inputs before and after compilation. The five existing
Main browser suites pass, covering blend display, own-layer masks and Source7/
Source8 filter-mask workflows. The standard codec lifecycle suite and all 410
web Node tests pass. The actual production render Worker separately passes its
42 Node cases and 35 headless Chrome scenarios, including all 26 original pixel
goldens, without changing their work, profile or memory limits.

A new headless Chrome harness uses real File/Open, pointer, keyboard and download
events on the production page. Four groups pass on an independently constructed
257 × 3 opaque Normal document. A held Pencil preview is visible, but Save cancels
it and both downloads retain the exact original archive. Releasing the stroke
commits the complete middle row once. A rectangular selection restricts the red
flood to the 129 pixels at the top right, preserving the black barrier and white
bottom row. All 771 saved pixels are independently decoded and compared. Undo
and Redo reproduce the exact earlier archives. Browser downloads leave committed
edits unsaved, as intended; cancelling the held gesture restores the clean state.

The initial Chrome profile-field mismatch, incorrect harness dirty expectation
and Studio toolbar coordinate calibrations remain preserved as failed attempts.
The successful attempts keep the original pixel and history assertions. Main's
public generated API stays exact; two private callback labels were checked
against the fresh module. The aggregate receipt is
`docs/validation/post-admission-browser/root-final.json`
(SHA-256 `535f44fb4fb990e81a3c585ce5e66459cbd869116a0aa7e99f4f0bcb8f275a2b`).

This finite browser checkpoint does not establish numeric Main credit balances,
all-resource admission, layer-style browser support, immediate physical Worker
termination, allocator/RSS ceilings, latency or physical input qualification.
It does not complete T04, T11 or the broader roadmap.

## Returned mapped-reader retirement checkpoint

A returned mapped reader now drops both its chain and optional coordinate scratch
before releasing their original metadata account. Releasing this account
notifies a waiting caller only after those owners retire. The private
inline carrier adds no heap allocation, factory, fee or capacity; constructor
failure paths and cancellation checkpoints remain unchanged.

Four focused native regressions pass, and a separate Raster/Editor run passes
642 tests with none ignored. The focused four are included in that broader run.
The two-node conditional fixture needs **288 bytes**: each vector first reserves
four slots, giving `4 × 56-byte Frame + 4 × 16-byte coordinates`. Exactly 288 free
bytes succeed and 287 refuse under the same original 4,096-byte budget; a uniform
reader still needs zero scratch. This is a fixture-specific demand, not a fixed
fee for every reader. Safe wake/reentry witnesses establish owner retirement,
not direct allocator deallocation or an RSS bound.

Native and Wasm test-target compiler assertions preserve complete reader
size/alignment against its prior defining fields. Strict native/Wasm Clippy and
Raster/Editor rustdoc checks pass. Adjacent target-fixture repairs pass 13 native
JXL tests (one existing ignored) and 30 appearance tests; they preserve native
assertions and do not manufacture a browser result lease. Earlier failures and
the initial zero-matching-test run remain archived, not counted as qualification.
The [scoped aggregate receipt](validation/mapped-reader-retirement-root/root-final.json)
is SHA-256 `c78c9c023a92eb446971f4713bdbda43d52fcaab765e588d2fbc4d9f725f74b3`.

Wasm test compilation is not Wasm/browser execution. Earlier browser receipts
remain tied to their pre-fix builds. Complete mapped/all-resource admission,
other retained controls and services/GPU, physical memory and latency remain
unfinished. T04 and T11 remain partial.

## Mapped-plan ownership checkpoint

Editor compiler records now own each mapped plan directly rather than wrapping
it in an otherwise unshared outer `Arc`. Existing inner mapping and source
lineage sharing remains intact. The larger compiler record is charged through
its actual `size_of` by the existing metadata store under the original budget;
this change does not grant extra capacity or establish a speed improvement.

Temporary candidate tables now retire their payload before releasing their
original metadata account, including cancellation and constructor errors. A
historical-order negative control reproduces the early wake; the new owner
invokes its release callback after payload retirement, before same-scope reentry.
Original candidate
fees, checks, source-read limits and pixel results remain unchanged.

Before the separate fixed-slot rectangle-difference change, Raster/Editor passes
647 tests across 89 result groups, with none ignored. The desktop metadata-retry
subset passes nine tests, including the original 4,096-byte success case. Strict
native all-target and Wasm test-target Clippy, and Raster/Editor rustdoc, pass.
These counts overlap other focused checks and must not be added as unique tests.
The ownership changes are commits `09da45f3` and `dcb5cb34`.

The rectangle-difference helper separately replaces a temporary vector with four
inline optional rectangles in the same order, preserving caller loops and
sampling. Its library and five crop/selection/orientation consumer suites pass
191 tests with none ignored. A named test-case type resolves the only new Clippy
warning; its unchanged literals pass again. Final strict native/Wasm checks and
formatting pass. This separate result overlaps the earlier suite. The
[scoped aggregate receipt](validation/mapped-plan-owner-root/root-final.json)
is SHA-256 `2cc8c387d9c660ec76dcf8607e86a8456bb285d9c3be1e375bed3624734d4a79`.

No new browser execution, allocator/RSS bound, latency or performance result is
claimed. Node/Mapping ownership headers and replay scratch remain outside full
pre-admission. Complete mapped/all-resource admission is unfinished; T04 and
T11 remain partial.
