# PaintCraft roadmap implementation status

This is the live application ledger for the reference pack's 37 tasks. The
[roadmap](temp/paintcraft-implementation-spec-v5-hyperfine/ROADMAP.md) owns milestone
intent; [roadmap-status.json](roadmap-status.json) preserves task dependencies,
acceptance and evidence. A partial implementation does not complete its milestone.

Work proceeds on `main` in logical commits. CPU conformance, preservation, admission
and truthful file outcomes remain prerequisites. Reference builds, signing identities,
external-author trials and physical devices retain explicit evidence gaps while other
implementation work continues. The optional M5 products are separate roadmap tranches,
not implicit requirements to enable accounts or external services.

Current user priority: shared backend correctness, performance and reliability;
see the [backend audit and progress](BACKEND_PROGRESS.md). Classic toolbar/UI work
is deferred during this tranche. [Classic coverage](CLASSIC.md) retains the frontend
fidelity target, and Studio remains available on the shared editor. [Retro](RETRO.md) remains deferred, with W95FA selected as its
default UI font and planned skins for older Microsoft Paint versions and
Deluxe Paint II Enhanced 2.0.

[Dependency choices](DEPENDENCY_CHOICES.md) records the selected libraries and their
actual consumers. The current batch-v1 wire policy and canonical PCND fixture remain
unchanged by dependency and crate refactors.

| Task | Gate | Scope | Status | Remaining work / evidence |
|---|---|---|---|---|
| T01 | M0 | Compiled Rust scaffold and dependency lock | implemented | Native/headless workspace builds and default desktop run are verified. |
| T02 | M0 | Native input and presentation spike | partial | Actual-pixel and multipass tests exist; direct-shell comparison, native pen/IME traces and hardware changes remain. |
| T03 | M0 | Reference acquisition feasibility | external-evidence-pending | Legal pinned Windows Paint/Paint.NET reference binaries and repeatable captures are not present. |
| T04 | M1 | Raster domains and blob leases | partial | Sparse COW tiles retain byte leases through documents/history/readers; default decoded payload admission is process-shared at 2 GiB with atomic exhaustion tests. Logical blob authority, cross-session pressure, adaptive metadata/allocation budgets and RSS qualification remain. |
| T05 | M1 | Immutable document and split identity | partial | Flat immutable snapshots and split state/history identity exist; full session identity and richer nodes remain. |
| T06 | M1 | Atomic batch and operation oracle | implemented | Batch v1 oracle vectors, atomic failure and no-op behavior pass; v2 remains an independently documented extension. |
| T07 | M1 | Recorded history navigation | partial | Recorded hot/cold linear undo/redo and guarded worker restoration preserve IDs and pixels, including leased disk payloads. Branching graphs, process-wide residency and full metadata admission remain. |
| T08 | M1 | PCND and headless round trip | partial | Canonical PCND and CLI round trips pass; full native subset/hostile-archive scope remains. |
| T09 | M1 | First complete editor slice | partial | Complete pencil/session/display/save slice passes; native input qualification depends on T02. |
| T10 | M2 | Ordered save and recovery journal | partial | Streaming native saves, bounded FIFO, captured-state guards and claimed recovery pass; write-ahead snapshots/receipts survive process exit; restarted destination inspection and explicit durability confirmation retain historical identity without adopting current state; degraded-store policy, benchmarks and full fault/platform matrix remain. |
| T11 | M2 | Bounded history and store GC | partial | Preserving history handles large snapshots through bounded classification and selective transfer batches. Native edits/navigation use a shared worker with owner/epoch/policy guards and ordered save/export barriers. Owned decoded tile capacity is now shared across normal edit/import/restore paths. Adaptive all-service admission, cross-session pressure, background priorities, segment relocation and broader corpus/input-latency qualification remain. |
| T12 | M2 | Ordered renderer and region effects | partial | Ordered CPU/GPU flat composition, exact regional CPU output, bounded output/lower-prefix caches and caller-owned CPU band/tile partitions exist with local measurements; CLI exports accept explicit worker limits. Finer dirty rectangles, ROI effects, parallel cache-miss integration, multiresolution rendering, persistent GPU residency and group plans remain. |
| T13 | M2 | Scheduler and GPU preparation | partial | Shared native render/edit workers prepare validated stroke/text plans from immutable previews. Revision-keyed weak snapshots avoid recollecting unchanged repaints; queued edits, file barriers and completion invalidate inputs. Guarded no-op results and compatible progress have tests. Preview-capture process costs are locally measured; changing-input cost is inconclusive. Device epochs reject replaced/retired GPU completions; Metal destruction and fresh-device host-history reconstruction pass. Native edit/render pools share declared-byte admission with worker/result guards surviving executor drop. Native window-device recovery, priorities, physical/all-service budgets, changed-preview costs and native latency/interference qualification remain. |
| T14 | M2 | Tool completeness tranche | partial | Antialiased brushes, bounded queued paint with ordered compound previews, Studio primary/secondary gestures, fill, shapes, rectangle selection and captured-layer Cosmic Text/Swash previews work. Complete selections, secondary eraser/text policies, editable text objects, IME and native sustained-input latency qualification remain. |
| T15 | M2 | Classic v11 usable preview | partial | Classic menus, grouped tools, vector widgets, palette, right layers and persistent choice exist; brush/transform popup presentation and disabled-state tests follow supplied photos; pixel rulers, Gridlines routing, status visibility and fullscreen commands have application tests; execution of extra brush media/transforms, reference fidelity, preserving-target flow and complete M2 prerequisites remain. |
| T16 | M3 | Classic historical parity | not-started | Per-operation evidence records; original menu/eraser/canvas/tool lifecycles and allowed OS differences. |
| T17 | M3 | PDN and PAINT compatibility | not-started | Read/write/metadata tested independently; malicious parsing bounds; no .NET deserialization/runtime. Fresh creation and source-preserving rewrite qualify independently; retain source bindings across repeated saves. |
| T18 | M3 | Wasm effect SDK for contributors | not-started | ABI range/trap/halo/mask tests; one complete external-author workflow before SDK stabilization. |
| T19 | M3 | Studio parity tranches | partial | Studio toolbar, floating Tools/Colors/Layers, guarded History navigation and independent document tabs with owner-routed save/recovery and close handling work; shared descriptors and menu/shortcut dispatch cover existing actions; full action coverage, image operations, effects, PDN and reference parity remain. |
| T20 | M3 | Codec breadth | not-started | Per-backend/variant write+read evidence, metadata/index and worker failures; no Rust-only delays. |
| T21 | M3 | Performance decision experiments | not-started | Raw benchmark artifacts; select thresholds/index/atlas layout; no claims from theoretical speed. |
| T22 | M4 | Browser conformance | not-started | Native-equivalent fixtures, interruption tests and unsupported-capability reports. |
| T23 | M4 | RetroPaint and historical formats | not-started | Historical Paint and Deluxe Paint II Enhanced 2.0 skins; era-specific fixtures and explicit additional capabilities without changed default muscle memory. |
| T24 | M4 | Compact PCND container study | not-started | Benchmark justification, corruption tests and old-v1 corpus preserved. |
| T25 | M5 | Mobile adaptation | not-started | Device execution, orientation/background/quota/thermal evidence; not cross-compile-only. |
| T26 | M5 | Optional AI and MCP | not-started | Consent/provider/model boundaries and stale-result/undo safety; no mandatory account. |
| T27 | M5 | Paint 3D research | not-started | Separate design and capture corpus before implementation claims. |
| T28 | RELEASE | Public release readiness | not-started | Target-specific manual/native tests, security protocol, license notices and final capability manifest. Capability manifest claims require profile-scoped executed evidence, not model-test status. |
| T29 | M1 | Version-aware document admission | partial | Exact v1 admission and unsupported rejection exist; all VER cases and future preview modes remain unqualified. |
| T30 | M3 | Foreign-source lineage and preservation planner | not-started | PRES-01..10; preserved opaque bytes AND relationships; dependent/unknown edit refusal; alternating original-consumer edits and source-removal recovery. |
| T31 | M2 | Frozen resource snapshots | partial | Native recovery verifies exact input identity and retains decoded state under a source claim; general resource snapshots, source bindings, fonts/profiles and DPI/clock cases remain. |
| T32 | M2 | Truthful file receipts and save associations | partial | Stable receipt IDs, sequences, binding guards, alias checks and live/resumed reconciliation pass; native reacquisition validates path-free persisted evidence and isolates historical outcomes from current document adoption; open-to-save conflict guards, diagnostics/repair and filesystem qualification remain. |
| T33 | M1 | Capability discovery and admission checks | partial | Truthful CLI manifest exists; shared generation-aware GUI/worker capability service remains. |
| T34 | M1 | Hash-guarded one-shot automation | partial | Immutable read and pre-parse hash guard exist; complete cancellation/retry lifecycle cases remain. |
| T35 | RELEASE | Profile-scoped release qualification | not-started | QUAL-01..10; synthetic or missing oracle cannot promote a release; foreign fresh/preserving support qualified separately; relevant backend changes invalidate scope. |
| T36 | M3 | Bounded shared-component extraction review | not-started | REUSE-01..06; independent product policies/versions and CLI dependency graph; no generic Craft runtime, foreign UI, P2P or database engine requirement. |
| T37 | M0 | Scoped adoption and benchmark evidence workflow | in-progress | Dependency/task ledgers, BF29 process evidence and BF06 raw Criterion/process measurements exist; prior Divan-only raw-sample gates, other relevant comparisons and cross-platform/interactive qualification remain. |
