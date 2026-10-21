# Backend engineering progress

## Initial audit: 2026-10-19, revision 95db5d0

The backend work takes priority over the pending Classic toolbar work. The user
requests correctness and runnable workloads first, then measured compositing,
history resources, brushes, rendering refinement and format qualification.

| Area | Executable implementation | Gaps and immediate implications |
|---|---|---|
| Storage | Finite sparse 256×256 straight RGBA8 tiles; immutable Arc COW snapshots; staged allocation admission | Resident Vec tiles only; no logical store/decoded leases, tile summaries or compression. A tiny COW edit copies a complete tile. |
| Document | Immutable flat ordered raster layers, metadata and distinct document/state identities | No groups, effects, profiles or general selection masks; future validity keys must include these when implemented. |
| Transactions/history | Typed v1/v2 batches, atomic private staging, epoch guards, recorded undo/redo, memory accounting and explicit trimming | Linear snapshots with shared tiles; redo branches are truncated. No recorded patch deltas, compressed tiers or disk spill. |
| CPU rendering | Exact integer ordered composition, clipping, opacity and cancellation | Full output allocation per render, scratch-row copying per layer, per-pixel processing of implicit transparent spans; no dirty cache. |
| GPU rendering | Matching integer compute compositor with qualified test cases and CPU fallback | Fresh staging and readback boundary; no persistent tile residency or device epochs. No performance crossover measurement. |
| Brush | Fixed-point antialiased round coverage, separate preview staging and captured stroke options | Preview replays the path and recomposites a full frame. Pressure/flow/media policies and incremental coverage remain incomplete. |
| Scheduling | Bounded Rayon executor with owner-scoped handles, admission, cancellation and stale-result guards | No priority scheduling, GPU completion authority or interactive latency trace. |
| Formats/CLI | PCND canonical fixtures, bounded readers, image PNG/BMP/JPEG, native save/recovery pipeline, real Clap commands | Other requested formats and source-preserving foreign rewrites remain unimplemented; profile/metadata losses are explicit. |
| Tests/benchmarks | Pixel tests, v1 oracle vectors, GPU equivalence, CLI and GUI workflows, recovery failure tests | No tracked application benchmark results at audit start. Timing and resident-memory claims need new evidence. |

Retain these implementations and the exact `paintcraft.core/default-v1` arithmetic.
No correctness defect is asserted solely from a missing optimization. The row-copy
and full-frame costs above follow directly from `render_cancellable` and desktop
`sync_texture`; their latency impact must be measured before selecting a contender.

The initial workspace test run passed 488 tests (20 explicitly ignored). The
canonical fixture/oracle tests establish existing v1 behavior, not historical
application parity, compressed-history correctness or unimplemented formats.

## First active slice: CPU compositor row spans (T12 / BF29)

Compare the retained scalar row-copy renderer with a read-only borrowed-tile-span
renderer. Skip implicit transparent runs and specialize opaque runs without
changing ordered source-over or opacity rounding. Sparse authoritative storage
stays unchanged; the public full-render result remains an owned dense buffer.
A sparse layer is not a contiguous whole-image surface, and alpha-zero hidden RGB
must not leak into the transparent-black flat backdrop. A borrowed whole-image
shortcut therefore requires a separate surface contract and is not claimed here.

Use deterministic 1/5/20/100-layer sparse fixtures, dense translucent layers,
opaque images, tiny images and clipped layers. Measure steady-state rendering with
Divan, and complete prebuilt driver processes with the existing Hyperfine protocol.
The frozen baseline lives only in benchmark support. No production fallback or
shipping CLI benchmark command is introduced.

Dirty-region/prefix caches, viewport-only composition, parallel partitions,
SIMD, mipmaps, GPU residency/crossover and adaptive compressed disk-backed history
remain required subsequent slices. Neither this audit nor a renderer benchmark
completes those gates.

## First slice delivered

The borrowed-span CPU compositor is implemented with independent all-alpha/opacity
checks, COW/edge/hidden-RGB tests and an on-disk editing/save/export workflow.
493 workspace tests, strict Clippy and a required real Metal adapter parity test
passed. The final dense render allocation remains; one 4 KiB scratch-row allocation
is removed at 1024-pixel width. No source tiles are copied for reading.

[The local decision and raw evidence](../benchmarks/results/cpu-spans-2026-10-19/DECISION.md)
record Divan and three Hyperfine sessions per workload. Sparse process medians
improve 5.5–11.3×; the implicit opaque-background case improves 15.6×. Dense
mixed-alpha composition is inconclusive (about 1.4% slower by pooled process
median). These are full CPU composition results on an M1 Pro, not GPU, frame-tail,
reference application parity or large-document residency qualification.

Next priority is regional composition and dependency-aware reuse for localized
edits, followed by bounded parallel partition experiments. These must retain the
committed cache across preview cancellation and validate structural/opacity/offset
changes. Full history tiering and broader format work remain in the user's backend
request and the original roadmap; this milestone does not reduce that scope.

## Second slice: exact regions and bounded composite reuse

`render_region` now allocates only the requested native-resolution rectangle.
The output cache stores aligned 64/128/256-grid candidates and an exact ordered
prefix below the uppermost relevant layer. Weak immutable source versions detect
changes without pixel hashes or strong references that pin decoded source tiles.
Committed and preview caches have separate byte/entry budgets, so preview
cancellation preserves committed entries. Source storage remains sparse COW.

The desktop CPU fallback now retains this cache across jobs; GPU-first dispatch
and stale document/epoch/gesture rejection are unchanged. Single relevant layers
and tiny outputs bypass caching. A small stroke in the tested 1024² layered image
recomposes 65,536 pixels, with the lower stack reused, instead of recompositing all
1,048,576 pixels. Invalidation currently follows changed storage tile versions;
smaller output tiles do not yet produce finer brush dirty rectangles.

505 workspace tests pass (20 explicitly ignored), strict workspace/all-target
Clippy passes, and required native Metal parity passes. New coverage includes
partial/huge-canvas regions, source version lifetime, exact prefix rounding,
property/structure changes, undo/redo, budget eviction and preview cancellation.
The stale-worker test's artificial 8 MiB budget now includes the new cache/key
allowance; its stale-presentation and released-reservation assertions remain.

See [the implementation contract](REGIONAL_COMPOSITOR.md) and
[the measured cache decision](../benchmarks/results/cpu-cache-2026-10-20/DECISION.md)
for timing, memory and limitations. CPU frames still assemble and upload the full
dense image. Parallel partitions, finer operation invalidation, GPU residency,
mipmaps and history compression/spill remain incomplete; T12 stays partial.

## Third slice: bounded CPU band/tile partitions and CLI export

The renderer now accepts a caller-owned Rayon pool, using disjoint horizontal
bands or safe borrowed tile-row views. Tile view metadata has its own checked
capacity budget; no second dense output or document-global lock is introduced.
Cancellation discards private output after active jobs join. Same-pool nested
execution is tested. The CLI's explicit `render --render-workers 1..64` uses
64-row bands for parallel exports and retains serial rendering by default.
An export renderer callback preserves loss acknowledgement and validates complete
output before encoding/writing; unknown loss codes now fail before renderer work.

510 workspace tests, strict all-target Clippy, formatting, file lengths and real
Metal parity pass. New integration tests independently decode layered PCND exports
at 1/2/4 workers, check a hand-calculated pixel, and reject invalid worker counts
before writing. Region, metadata budget, cancellation and COW identity tests apply.

The evidence audit found Divan 0.1.21 has no raw internal-sample CLI export. Keep
its existing allocation summaries and archived Hyperfine process evidence, but do
not call the raw-sample gate complete. Criterion 0.8.2 is added only for statistical
benchmark consumers with raw JSON/CSV; optional parallel analysis/plotting are off.
A follow-up must add raw in-process samples for the older span/cache choices too.
This is an explicit remaining gate rather than a rewritten historical artifact.

See [the partition contract](PARALLEL_COMPOSITOR.md) and
[BF06 measurements](../benchmarks/results/cpu-parallel-2026-10-20/DECISION.md).
The desktop cache/GPU path keeps its existing dispatch; bulk pool throughput does
not prove interactive responsiveness or scheduling priority. Global CPU/resource
coordination, parallel cache misses, finer invalidation, history compression/spill
and remaining format/roadmap work are still required.

## Fourth slice: recorded cold history and decoded ownership

Older history states can now hold immutable metadata plus shared independent tile
records. Solid and sparse default-relative payloads release decoded tile Vecs;
full-extent tiles retain their original raw allocation. Weak decoded caches and
explicit reader leases keep restoration reusable without pinning packed pixels.
CRC checks guard reconstructed payloads. See [the ownership and failure
contract](HISTORY_STORAGE.md) for exact semantics and budget limitations.

Undo/redo/seek prepare the recorded target before publishing its cursor/document/
epoch. Failed restoration is visible to the UI and retryable. Editing after a cold
restore keeps the preceding state hot. Automatic pressure work caps new tile scans
and complete packed entries before ordinary retention trimming; the two-state floor
and reader-held bytes make retention a soft policy, not a physical-memory cap.

523 workspace tests (20 ignored), strict all-target Clippy and real Metal parity
pass. Tests exercise decoded release with live readers, full recorded navigation,
metadata/selection/off-canvas/hidden RGB, checksum corruption, cancellation,
admission failures, raw fallback, bounded work, table shrinking and epoch overflow.
The release harness compares identical timelines at explicit cold-age checkpoints;
these checkpoints are distinct from the automatic pressure trigger.

Global logical tile authority, previous-version photographic deltas, LZ4/Zstd,
disk spill/segment GC, background priorities, history branches and interactive
latency/resource qualification remain. T04/T07/T11 remain partial.

The [local history decision](../benchmarks/results/history-packing-2026-10-20/DECISION.md)
retains 1,200 process measurements, 800 forward/reverse internal samples and 20
separate phase traces. Sparse 1,000-edit/32-checkpoint history lowers median peak
RSS from 253.5 to 57.2 MiB while increasing complete lifecycle time by about 50%.
Full-extent noise does not save memory and costs roughly twice as much time.
Checkpoint tails and immediate reader-release RSS are reported separately. No
universal interval, historical accounting-regression or interactive latency claim
is made. Release CLI PCND creation and matching serial/four-worker exports pass.

## Fifth slice: independent LZ4/Zstd history blocks

Explicit history compression policies now compare LZ4 and Zstd levels 1/3 with
the raw/solid/default-relative records. Only a smaller encoded representation is
retained. Restores use an admitted fixed-size tile, exact-frame validation and CRC;
no decoded source stays strongly pinned by a compressed record. One scratch buffer
and codec context are reused per compaction. Conservative native headroom and
observed context sizes remain distinct from physical allocator enforcement.

531 workspace tests (20 ignored), strict Clippy and native Metal parity pass.
New cases cover compressed source release, full RGBA navigation, native frame
bounds/truncation/trailing data, corruption, insufficient staging/restoration,
solid/raw/small-patch fallback and cancellation before/after codec execution.
The default remains raw/patch until the comparative evidence supports a choice;
no disk spilling, background priority or complete physical-budget claim is made.

The [codec decision](../benchmarks/results/history-codecs-2026-10-20/DECISION.md)
retains 1,800 process measurements, 1,200 internal samples and 30 phase traces.
LZ4 lowers structured256 median peak RSS from 66.6 to 48.5 MiB but increases the
complete lifecycle from 62.8 to 87.8 ms. Zstd stores fewer bytes but costs much more
CPU and does not yield the lowest RSS. High-entropy controls retain raw allocations
and expose failed-compression overhead. No codec is selected globally.

A follow-up failure audit reproduced dead weak aliases accumulating after a later
tile failed restoration. Failed restores now reclaim those private aliases while
leaving cursor/document authority unchanged. A 128-retry regression passes; the
workspace now passes 532 tests (20 ignored) and strict Clippy. The codec measurements
remain explicitly frozen to 417c0ed, before this correctness follow-up.

## Sixth slice: preserving publication and native leased history segments

Core defines opaque immutable byte storage; platform implements private locked
append segments and history prepares replacement payload references. The default
retention policy now preserves existing undo: memory/storage exhaustion rejects a
new edit before publishing its document, cursor, selection, epoch or redo removal.
Explicit TrimOldest retains the counted legacy policy. Desktop editors receive a
per-session native store; startup cleans only marked, unlocked abandoned sessions.

Writes are provisional until checked segment-rotation/final synchronization passes.
Cancellation is checked after that barrier. A closed dirty segment remains part of
the barrier, protecting concurrent preparers. Failed partial writes/rollback do not
reuse the writer; required leases survive, and uncertain/dead lengths stay charged.
Whole-segment GC preserves live extents and retries failed deletion. Session history
is not restart-persistent; durable document recovery keeps its separate authority.

550 workspace tests (20 ignored), strict Clippy, required Metal parity and release
CLI save/export pass. Native tests exercise actual file/lock/read/GC behavior and a
killed child process; fault cases cover partial disk-full, rollback/sync failures,
closed provisional barriers, quota, permissions, missing/short/corrupt segments and
readers outliving stores. Editor tests verify full raw/codec navigation, rejected
edits retaining redo, cancellation after write/sync and cleanup of unpublished data.

The [spill decision](../benchmarks/results/history-spill-2026-10-20/DECISION.md)
retains 1,200 process runs, 800 internal samples and 20 separate traces. Disk lowers
structured raw-history median peak RSS from 67.1 to 52.4 MiB at about 5x lifecycle
time; short noisy history shows almost no RSS improvement despite releasing decoded
allocations. A checkpoint reaches 159 ms in a separate trace. Internal samples have
substantial variability; no universal ranking or interactive tail guarantee follows.
The interrupted per-extent-sync pilot remains archived and explicitly excluded.

Incremental preparation of snapshots with more than 64 new sources, global logical
tile authority/physical admission, background priorities, efficient relocation,
photographic deltas, natural/mask/depth qualification and branching history remain.
The default is preserving, but those limits can still reject a large edit despite
available disk. See [the exact contract and limits](HISTORY_SPILL.md).

## Seventh slice: large-snapshot preparation and pressure continuation

Automatic pressure no longer skips snapshots requiring more than 64 new sources.
It registers immutable raw leases without scanning the remaining pixels, then
refines/transfers those sources across batches. Deferred uniform data becomes
inline and the configured codec can finish warm preparation without disk. Raw
sources still owned by hot documents are excluded from automatic transfer; manual
complete transfer semantics remain unchanged. If necessary, pressure can cool the
previous state while retaining the new current state hot. Every result remains
in the private candidate until the whole admission succeeds.

556 workspace tests (20 ignored), strict Clippy, real Metal parity and release CLI
save/export pass. Regressions cover 130-tile bases, full replacement across more
than two transfer batches, later-batch failure retaining document/redo and reclaiming
provisional files, hidden-RGB uniform refinement and warm codec completion without
disk. A unit case registers 80 sources with exactly 64 scanned and 16 deferred.

The [large-history decision](../benchmarks/results/history-large-2026-10-20/DECISION.md)
retains 360 process runs, 240 internal samples and six independent phase traces.
On 65/130/260-tile shared bases, automatic lifecycle medians are 2.1–2.2x faster
than the explicit full-scan/spill strategy under identical allowances and full
retained timelines. Peak RSS is similar/slightly higher. Automatic avoids spilling
the unchanged hot base; it does not improve every percentile. The timed boundary
checks changed bytes independently and unchanged identities, with full byte
preflight and endpoint CRCs; it does not pin source leases in fingerprints.

This is synchronous continuation, not an interactive time bound. Global logical
tile authority/physical admission, background jobs, metadata scaling, photographic
deltas, masks/depth, segment relocation and persistent branches remain. A current
raw document exceeding its allowance can still reject an edit. T11 stays partial.

## Eighth slice: detached edit and navigation preparation

An editor can capture a private timeline, prepare an edit or recorded history
transition on a worker, and publish the result only against its original session,
epoch and resource policy. Capturing shares immutable pixels and fallibly copies
metadata; preparation includes the required history admission and spill barriers.
Save acknowledgements remain independent of pending preparation. Stale, foreign,
cancelled and dropped results leave live publication unchanged and release their
private payload leases.

Cancellation now reaches automatic pressure batches and individual cold-restore
tile boundaries. Individual command kernels and filesystem calls remain
non-preemptible. This API does not yet move desktop dispatch to workers, bound
whole-process physical memory, or qualify interactive latency. Scheduling and the
save/switch/close ordering of the desktop consumer remain next work; T11 stays partial.

567 workspace tests pass (20 explicit ignores), along with strict workspace/all-target
Clippy, formatting, the 800-line limit and actual Metal parity on Apple M1 Pro.
New regressions cover worker transfer, edit/undo ABA, identical-content foreign
sessions, policy invalidation, no-ops, saves during preparation, cancellation after
a native spill barrier preserving exact redo/quota, detached undo/redo/seek and
cancellation at every boundary of a materialized two-tile cold restoration.
Previous benchmark results remain frozen to their recorded source revisions;
these correctness checks establish no new performance result.

## Ninth slice: native desktop preparation and ordered file captures

Real desktop windows now connect every document and frontend workspace to one
shared bounded editing worker. Edits and undo/redo/history restoration prepare
privately, and only their originating controller publishes them. Runtime targeted
polling retains another owner's completion and reservation; cancelled/abandoned
consumers release admission only after actual completion. Released previews stay
visible, and layer aliases/target repair follow successful publication.

Document-owned file barriers order saves and export confirmation between earlier
and later activations. Native save I/O captures the exact revision and may finish
while later edits prepare; its acknowledgement keeps those later edits dirty.
Inactive tabs receive their own results. Close and interface-switch gates wait for
editing/file requests before testing dirty state. Export confirmation holds its
revision until it finishes; synchronous fallback saves preserve the same boundary.

584 workspace tests pass (20 explicit ignores), plus strict workspace/all-target
Clippy, formatting and the 800-line limit. New controller/application regressions
exercise shared admission, stale/cancelled/dropped work, aliases and target removal,
stale queued history clicks, retained previews, real PCND captures across later
edits and inactive tabs, close/switch gates, decoded export pixels and a real egui
two-pass gesture/publication/display cycle. Actual Metal parity passes on Apple M1
Pro. Release desktop/CLI builds pass; the native window exits cleanly after 30
frames with isolated settings/recovery, and release PCND save/reopen produces
byte-identical serial/four-worker PNG exports.

This is correctness and integration evidence, not an interactive performance
measurement. Native canvas input presently waits for its current publication with
a progress cursor. Metadata capture, command/font-mask construction, publication,
destruction and flat export still involve synchronous UI work. The preparation
charge is provisional; global physical memory, command-payload admission,
priority/fair scheduling and queue-inclusive p95/p99/max input latency remain.
See [the desktop ordering contract and limits](DESKTOP_EDITING.md). T07/T11/T13 stay
partial and historical benchmark source boundaries remain unchanged.

## Tenth slice: bounded sustained paint input and complete queued previews

Native pencil, brush and eraser gestures can now continue while earlier paint-only
strokes prepare. The preview contains each in-flight, queued and live path once in
FIFO order, then rebases when a preceding stroke publishes. Separate changed
strokes remain separate undo steps, including identical translucent paths. Other
commands and file boundaries pause new painting; previews exclude their later and
unfinished gestures from a capture.

Logical native input admission caps pending/live painting at 32 strokes, 262,144
points and 524,288 captured point-vector slots. Live vector growth prepares a
fallible replacement before changing the captured prefix; actual capacity is
checked against the remaining allowance. Rejection leaves the prefix releasable,
and fixed-point conversion failure permits subsequent valid movement. Known
activation tokens are deduplicated before another admission attempt. These counters
do not cover preview/command copies or constitute a process-wide physical budget.

A separate fix gives retained text previews their captured layer, so committing
text entry or changing the selected target cannot retire or redirect the preview.
Raster text still commits pixels; editable text objects remain pending.

594 workspace tests pass (20 explicit ignores), strict workspace/all-target Clippy,
formatting and the 800-line limit pass, and release desktop/CLI builds succeed.
Actual Metal parity passes on Apple M1 Pro; an isolated native window starts and
exits cleanly after 30 frames. New tests check independent translucent pixel values,
undo/redo, a live stroke spanning an earlier actual native spill barrier/publication,
structural/file boundaries, point/capacity rejection and recoverable coordinates.
Real egui two-pass input delivers three coalesced strokes and a fourth gesture over
later frames while the worker is held, then verifies the complete preview, final
pixels and four undo steps. Bundled Selawik text retains its preview on the original
layer across text-entry retirement and target changes.

These are correctness/integration checks, not native p95/p99/max input, allocator
RSS or GPU residency measurements. Preview/batch construction still makes copies
and involves UI work; global admission, preview allocation reduction, broader tool
input and native interference/latency qualification remain. T14 stays partial.

## Eleventh slice: worker-side preview/tool preparation and compatible progress

Render requests now share immutable preview snapshots instead of recollecting a
second path copy and expanding/validating batches during submission. Native render
workers prepare point/span values, canonical batches and font masks before private
execution/composition. Native editing workers use the same builder for stroke and
styled-text commands, so release dispatch also avoids this work. Typed validation
and the existing long-path chunk/joint semantics remain in place.

Empty tool output has an explicit guarded no-change preparation result rather
than a fabricated wire batch. It preserves document, epoch, cursor, redo and save
acknowledgements. Empty text can still prepare caret geometry. Native caret lookup
uses only an available cached mask, and completed font discovery remains retained
until it can activate without waiting for a worker-held cache.

Progressive render acceptance now requires an unchanged owner and an input extension.
Deletion, style/replacement changes, cancelled/retried plans, failed preparation and
font activation retire incompatible work; identical-value retries get a new lineage.
Useful in-flight brush/text extension remains supported. Already displayed pixels
can remain until a replacement finishes; this is not instantaneous display latency.

606 workspace tests pass (20 explicit ignores), plus strict workspace/all-target
Clippy, formatting and the 800-line limit. Release desktop/CLI builds pass. Required
Metal parity passes on Apple M1 Pro; the native window exits cleanly after 30 frames
with isolated settings/recovery. New regressions cover frozen preview inputs and
stale tickets, mid-preparation cancellation, font-cache acquisition held across UI
submission, worker-only text masks, deferred discovery activation, empty caret/no-op
results, preserved redo/save state, deletion and identical-value cancel/retry. These
are correctness/worker-integration checks, not native latency or RSS measurements.

UI snapshot gathering/comparison still copies point data, history capture still
copies metadata, and other command construction, font-catalog activation and data
destruction retain UI costs. Global physical admission, device epochs, priority/fair
scheduling and native p95/p99/max/interference qualification remain. T13 stays partial.


## Twelfth slice: reuse unchanged desktop preview snapshots

Tool state now supplies process-local preview revisions, including captured path,
shape constraint, text content/style, release and cancellation changes. Repeated
points, rejected path growth and unchanged text styling keep their revision. The
desktop caches immutable previews by these inputs, queue revision, document epoch
and cancellation lineage. Accepted commands, file barriers and consumed worker
results invalidate queue inputs. Public future tool settings do not mutate a live
captured gesture.

Rendering reuses an unchanged snapshot by pointer identity; overlays skip path
cloning. Equal-value input refreshes replace the owned snapshot without inventing
a render revision. The cache itself holds weak references, so it cannot retain
large point buffers in inactive editors once render consumers release them.
Changed or expired snapshots still collect point data and may compare paths on
the UI thread. This does not establish a latency improvement or physical memory
bound; allocation and native latency qualification remain.

613 workspace tests pass (20 explicit ignores), with strict workspace/all-target
Clippy, formatting and the 800-line limit. Release desktop/CLI builds, required
Metal parity on Apple M1 Pro and an isolated 30-frame native-window smoke pass.
Added regressions cover immutable old snapshots after growth, idle reuse,
expired cache reconstruction, text and shape updates, rejected moves, matching
button release, queued paint, ordered file barriers, cancellation, completion and
an equal-value source refresh. T13 remains partial.


## Thirteenth slice: measure preview snapshot reuse

The [local process decision](../benchmarks/results/preview-reuse-2026-10-20-measured/DECISION.md)
compares real controller capture with frozen collect/compare behavior using short,
long, queued and changing paths. Three Hyperfine sessions per case retain 60 raw
samples per variant, setup/destruction, correctness preflights, resource logs and
artifact identities. Unchanged process medians improve 1.35×, 2.40× and 3.17× on this
M1 Pro. Changing-input medians are a tie; the cached changing maximum is slower and
remains in the data. RSS results describe complete benchmark processes, not global
editor residency. No native input/frame latency or GPU performance is qualified.

The process-only driver is compiled only into library tests and ignored in normal
runs. It adds no shipping dependency or public API. 613 workspace tests pass with
21 explicit ignores. Strict desktop/all-target Clippy, formatting and the 800-line
limit pass. Changed previews, history metadata capture, global physical admission,
priorities, device epochs and sustained native interaction remain. T13/T37 stay
partial; the broader roadmap is not complete.


## Fourteenth slice: GPU lifetime guards and host reconstruction

GPU renderers now carry an irreversible process-local device epoch. Loss callbacks
retire that epoch; hosts can preserve their own callback through `with_epoch`.
Desktop callbacks request repaint without mutating editing state. Native frame
acceptance checks device identity as well as job/document/preview freshness,
including CPU fallback results. Backend replacement cancels outstanding work and
invalidates its handle, retaining the published host document and history.

618 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting, file limits and release desktop/CLI builds. Three required Metal
tests pass on M1 Pro: exact parity, explicit device destruction with fresh-device
host-history reconstruction, and host callback ownership/loss during staging.
Desktop regressions reject unrelated-epoch success and fallback and preserve host
state while replacing an admitted backend. A 30-frame isolated native smoke passes.

[The lifetime contract](GPU_LIFETIME.md) distinguishes these checks from unexpected
driver/device loss and full native-window recovery. Automatic eframe window/device
recreation, persistent GPU residency, transfer services, physical budgets and
performance/latency qualification remain. This slice adds no dependency or GPU
performance claim; T13 remains partial.


## Fifteenth slice: shared in-flight worker admission

Native edit/render pools now use one process-local declared-byte authority while
retaining local queue/byte caps. Runtime submission reserves atomically, and shared
lease guards keep a running worker or unread result charged after executor drop.
The controller holds observable contention in its edit queue; rendering retries a
feasible contended request instead of caching an error. Failed admission never
publishes a partial transaction.

625 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits and release desktop/CLI builds. Runtime tests cover
heterogeneous and racing pools, overflow, local rejection, cancelled/panicking workers,
executor drop and output destruction ordering. The desktop contention regression
retains a completed stroke through worker completion and unread-result admission,
then checks one publication and exact displayed pixels. Required Metal lifetime/
parity checks and a 30-frame isolated native smoke pass.

[The admission contract](WORKER_ADMISSION.md) explicitly limits the 3 GiB ceiling
to declared in-flight charges. This does not establish global decoded/native/GPU
allocation bounds, thread priorities, fairness, reclamation or latency. No new
dependency or performance improvement is claimed. Persistent physical ownership,
all-service admission and native pressure/interference qualification remain;
T13 stays partial.


## Sixteenth slice: ownership-based decoded tile residency

Every decoded tile now owns a shared byte lease until its Vec payload and final
reader are released. Standard native/headless admissions use one default 2 GiB
payload realm, covering fresh tiles, COW, imports, previews and cold restoration.
Weak cache/history tokens do not retain capacity. The byte primitive moves to
core types for actual raster/runtime consumers; runtime exports remain compatible.
Private execution also accepts an explicit admission realm. Pixel and wire policy
remain unchanged.

633 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits and release desktop/CLI builds. New regressions
cover reader-pinned capacity, failed COW, unshared weak-token transfer, decode and
partial-import rollback, racing allocations, shared defaults and atomic failure at
the second batch operation. A real history compaction/reader/undo/redo case checks
exact live payload counters and independent pixels. Metal parity/lifetime tests and
a 30-frame isolated native smoke pass.

[The residency contract](RASTER_RESIDENCY.md) distinguishes payload ownership from
worker declarations and process RSS. The fixed ceiling is a provisional guardrail;
metadata, codec/encoded/output/GPU buffers, adaptive admission and cross-tab pressure
handling remain. No allocation-throughput or RSS improvement is inferred from the
correctness checks. T04/T11/T13 remain partial.


## Seventeenth slice: guarded owner pressure maintenance

Detached preparation can now pack/refine/spill a bounded preserving history step
and return a typed storage-maintenance result. Owner publication changes no content
revision, cursor, redo or saved acknowledgement, but retires older private storage
preparations. Required spill barriers and cancellation complete before publication.
Native pressure coordination polls inactive tabs and parked frontend workspaces
first; clean maintenance stays separate from edits and yields to queued foreground
work. Resource-failed render keys retry when tile residency drops.

641 workspace tests pass (21 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits, release desktop/CLI builds, three required Metal checks and
an isolated 30-frame native smoke. New regressions cover delayed ownership release,
identity/save preservation, all observed cancellation boundaries, failed durability,
staleness, clean/no-progress behavior, inactive/parked ownership, foreground yielding
and unchanged-key render retry. Pressure telemetry is scaled/injected in coordinator
tests; there is no native 2 GiB exhaustion or event/frame latency qualification.

[The pressure contract](HISTORY_PRESSURE.md) records provisional 64 MiB headroom,
64-item work batches and retry timing. Request-sized admission, content retry,
cross-window signaling, pinned current/read data, all-service physical budgets and
native pressure/interference/tail-latency evidence remain. T04/T11/T13 stay partial;
no throughput or RSS improvement is inferred from these correctness checks.


## Eighteenth slice: bounded budget wake subscriptions

Byte budgets now offer weak, bounded, coalesced wake subscriptions. Pressure entry,
feasible denial and relevant releases notify consumers outside the counter mutex;
callback/capture destruction also occurs after unlocking. Unregistration, callback
panics and concurrent release events retain counter ownership guarantees. Native
contexts subscribe to shared tile and worker realms so another owner's pressure
or returned capacity can trigger a fresh local poll.

650 workspace tests pass (21 explicit ignores), plus strict workspace/all-target
Clippy, formatting/file limits, release desktop/CLI builds, three required Metal
checks and a 30-frame isolated native startup/shutdown smoke. Six core regressions
cover coalescing, denial, initial pressure, weak/bounded fanout, callback reentrancy/
panics and concurrent unregister/release. Three independent-context regressions
check wake routing without edits, unread-result retention and above-threshold
release interest. These use real counters/egui callbacks, not multiple OS windows.

[The notification contract](BUDGET_NOTIFICATIONS.md) records the 64-registration
bound, wake-only callback rule and remaining qualification. Request-sized admission,
content retry, native multi-window/suspension behavior, all-service physical budgets,
fairness and measured callback/input/frame latency remain. No throughput or RSS
gain follows from these correctness checks. T04/T11/T13 stay partial.
