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

[Dependency choices](DEPENDENCY_CHOICES.md) records the selected libraries and their
actual consumers. The current batch-v1 wire policy and canonical PCND fixture remain
unchanged by dependency and crate refactors.

| Task | Gate | Scope | Status | Remaining work / evidence |
|---|---|---|---|---|
| T01 | M0 | Compiled Rust scaffold and dependency lock | implemented | Native/headless workspace builds and default desktop run are verified. |
| T02 | M0 | Native input and presentation spike | partial | Actual-pixel and multipass tests exist; direct-shell comparison, native pen/IME traces and hardware changes remain. |
| T03 | M0 | Reference acquisition feasibility | external-evidence-pending | Legal pinned Windows Paint/Paint.NET reference binaries and repeatable captures are not present. |
| T04 | M1 | Raster domains and blob leases | partial | Sparse COW tiles and admission work; logical blob handles, resident leases and full residency accounting remain. |
| T05 | M1 | Immutable document and split identity | partial | Flat immutable snapshots and split state/history identity exist; full session identity and richer nodes remain. |
| T06 | M1 | Atomic batch and operation oracle | implemented | Batch v1 oracle vectors, atomic failure and no-op behavior pass; v2 remains an independently documented extension. |
| T07 | M1 | Recorded history navigation | partial | Recorded linear undo/redo exists; branching nodes and randomized navigation evidence remain. |
| T08 | M1 | PCND and headless round trip | partial | Canonical PCND and CLI round trips pass; full native subset/hostile-archive scope remains. |
| T09 | M1 | First complete editor slice | partial | Complete pencil/session/display/save slice passes; native input qualification depends on T02. |
| T10 | M2 | Ordered save and recovery journal | partial | Streaming native saves, bounded Tokio FIFO, stale binding/session guards and captured-state acknowledgement pass; ordered recovery journal and full fault/platform matrix remain. |
| T11 | M2 | Bounded history and store GC | partial | Shared snapshot retention is bounded; leases, segment GC, compaction and physical resource evidence remain. |
| T12 | M2 | Ordered renderer and region effects | partial | Ordered scalar and GPU flat composition exist; ROI effects, dirty-prefix cache and group plans remain. |
| T13 | M2 | Scheduler and GPU preparation | partial | Bounded Rayon preparation, owner-scoped handles, cancellation and stale-result tests exist; priorities, async I/O, device epochs and physical budget/fairness evidence remain. |
| T14 | M2 | Tool completeness tranche | partial | Brushes, fill, shapes, rectangle selection and bitmap text work; complete selections, system text and IME remain. |
| T15 | M2 | Classic v11 usable preview | partial | Classic menus, grouped tools, vector widgets, palette, right layers and persistent choice exist; reference fidelity, preserving-target flow and complete M2 prerequisites remain. |
| T16 | M3 | Classic historical parity | not-started | Per-operation evidence records; original menu/eraser/canvas/tool lifecycles and allowed OS differences. |
| T17 | M3 | PDN and PAINT compatibility | not-started | Read/write/metadata tested independently; malicious parsing bounds; no .NET deserialization/runtime. Fresh creation and source-preserving rewrite qualify independently; retain source bindings across repeated saves. |
| T18 | M3 | Wasm effect SDK for contributors | not-started | ABI range/trap/halo/mask tests; one complete external-author workflow before SDK stabilization. |
| T19 | M3 | Studio parity tranches | not-started | Release only named tested tranches; high-depth/groups not misrepresented as source features. |
| T20 | M3 | Codec breadth | not-started | Per-backend/variant write+read evidence, metadata/index and worker failures; no Rust-only delays. |
| T21 | M3 | Performance decision experiments | not-started | Raw benchmark artifacts; select thresholds/index/atlas layout; no claims from theoretical speed. |
| T22 | M4 | Browser conformance | not-started | Native-equivalent fixtures, interruption tests and unsupported-capability reports. |
| T23 | M4 | RetroPaint and historical formats | not-started | Era-specific fixtures and explicit additional capabilities without changed default muscle memory. |
| T24 | M4 | Compact PCND container study | not-started | Benchmark justification, corruption tests and old-v1 corpus preserved. |
| T25 | M5 | Mobile adaptation | not-started | Device execution, orientation/background/quota/thermal evidence; not cross-compile-only. |
| T26 | M5 | Optional AI and MCP | not-started | Consent/provider/model boundaries and stale-result/undo safety; no mandatory account. |
| T27 | M5 | Paint 3D research | not-started | Separate design and capture corpus before implementation claims. |
| T28 | RELEASE | Public release readiness | not-started | Target-specific manual/native tests, security protocol, license notices and final capability manifest. Capability manifest claims require profile-scoped executed evidence, not model-test status. |
| T29 | M1 | Version-aware document admission | partial | Exact v1 admission and unsupported rejection exist; all VER cases and future preview modes remain unqualified. |
| T30 | M3 | Foreign-source lineage and preservation planner | not-started | PRES-01..10; preserved opaque bytes AND relationships; dependent/unknown edit refusal; alternating original-consumer edits and source-removal recovery. |
| T31 | M2 | Frozen resource snapshots | not-started | RES-01..09; replaced/deleted source path cannot change pending output; missing final input fails; native DPI/clock transitions get real tests. |
| T32 | M2 | Truthful file receipts and save associations | partial | Stable receipt IDs, real queue sequences, binding generations and captured-state acknowledgement pass; alias/fingerprint guards, reconciliation and filesystem qualification remain. |
| T33 | M1 | Capability discovery and admission checks | partial | Truthful CLI manifest exists; shared generation-aware GUI/worker capability service remains. |
| T34 | M1 | Hash-guarded one-shot automation | partial | Immutable read and pre-parse hash guard exist; complete cancellation/retry lifecycle cases remain. |
| T35 | RELEASE | Profile-scoped release qualification | not-started | QUAL-01..10; synthetic or missing oracle cannot promote a release; foreign fresh/preserving support qualified separately; relevant backend changes invalidate scope. |
| T36 | M3 | Bounded shared-component extraction review | not-started | REUSE-01..06; independent product policies/versions and CLI dependency graph; no generic Craft runtime, foreign UI, P2P or database engine requirement. |
| T37 | M0 | Scoped adoption and benchmark evidence workflow | in-progress | Live dependency/task ledgers are established; comparative benchmark evidence is still pending. |
