# Preserving history maintenance under decoded tile pressure

Native pressure handling can now replace recorded-history storage without editing
the document. Eligible history is packed/refined/spilled on the shared editing
worker, then published by its originating controller. No command/effect replay or
history trimming is used to make room.

## Preparation and publication

`Preparation::maintain_history` owns a private metadata fork. One step classifies
at most 64 new source tiles, packs at most 64 complete older entries and transfers
at most 64 payloads, with existing 64 MiB temporary allowances. The current and
newest snapshots remain hot. Automatic packing respects `Packing::Adaptive`;
existing cold records can still be refined/transferred. Configured codecs and
spill-store policies remain in effect. Required spill barriers finish before a
prepared result is returned. Cancellation checks surround the bounded work units;
individual codec/filesystem calls remain non-preemptible.

`Session::publish_history_maintenance` validates the source session, base ticket and
preparation policy. A positive storage replacement changes only the history's
representation. Document Arc, selection, publication epoch, state/history IDs,
cursor, redo and live saved-state acknowledgement stay unchanged. It invalidates
older detached preparations even when their logical ticket still matches, so they
cannot reinstall a pre-maintenance representation. A no-progress result does not
replace history or invalidate other preparation.

Preparation, failed durability, cancellation, stale ownership or a changed policy
cannot replace the owner's timeline. Dropped private payload leases follow the
existing cleanup contract. Old decoded allocations remain charged until the owner
publishes AND all other strong readers release them. A positive maintenance report
counts storage work; it is not a promise that those readers have freed bytes.

## Native coordination

The App polls the process-default tile realm. Below 64 MiB of free payload capacity
(the smaller of 64 MiB and a quarter of the observed ceiling), it visits parked
frontend workspaces and inactive tabs before the visible editor. A subscribed
render's known feasible footprint can raise this target across frontend contexts,
so free space above the baseline does not suppress necessary preserving work. It polls every
maintenance owner without activating that tab or frontend. Only the existing
shared worker slot is used, with the existing declared-byte admission ceiling.

Maintenance has a separate controller slot and never makes a clean document dirty.
Completed input remains queued while that owner's storage step finishes. A completed
maintenance step yields the shared worker to already queued foreground edits before
another step starts. This is a local admission-order rule, not general scheduler
priority or a preemptible latency deadline. Each owner completion is consumed only
in the explicit poll phase. Starting a step cannot consume another completion
between the coordinator's yield decision and admission; a completed unread step
must first be reported to that coordinator.

No-progress/failure gets a five-second per-ticket retry backoff. A changed logical
revision can try sooner. The event loop requests the next wake while pressure
persists. [Shared budget notifications](BUDGET_NOTIFICATIONS.md) also wake native
contexts on pressure/reclamation across App owners; each context still polls and
publishes its own state. These thresholds are provisional engineering choices; native pressure,
interference and tail-latency measurements have not qualified them.

An installed render error retries only when its typed diagnostic identifies a
feasible decoded-tile denial. It retains the exact allocation realm and the known
attempt footprint. Readiness compares currently free capacity against that demand,
including reclamation before the UI receives the error. Partial-attempt rollback
alone does not qualify when the next attempt still needs more space. Fixed staged
or work limits and footprints above the realm ceiling do not create retry markers.
Successful renders clear the marker. This is advisory retry readiness, not a
reservation or a whole-operation allocation plan.

## Executed checks

641 workspace tests pass, with 21 explicit ignores, plus strict workspace/all-target
Clippy, formatting/file limits, release desktop/CLI builds, three required Metal
parity/lifetime tests and a 30-frame isolated native startup smoke.

Four backend regressions verify owner-only allocation release, unchanged document/
history identity and saved acknowledgement, stale/foreign/policy/superseded rejection,
every observed cancellation boundary with redo preserved, and a failed required
spill barrier. Independent pixel checks follow recorded undo/redo.

Four controller/App regressions verify clean maintenance and retry backoff,
inactive-owner reclamation with foreground paint queued, parked-workspace polling,
and retry of a resource-failed frame under an unchanged document key. The inactive
case keeps pressure asserted through completion and verifies foreground yielding.
Scaled injected counter observations drive the coordinator tests; storage, worker,
owner publication and tile-reader release are real. The tests do not exhaust 2 GiB
in a native window or measure actual input-to-display latency under pressure.

## Remaining scope

Capture still copies history metadata on the UI thread. A new edit can need more
capacity than a single maintenance step releases. [Accepted decoded edit retries](EDIT_ADMISSION.md)
retain their FIFO position and reserve the known footprint before resubmission;
a racing admission keeps the command waiting. A waiting owner can maintain its
history despite a later queued file boundary. Whole-request planning, other service
retry lifecycles, native multi-window wake qualification and all-service fairness
remain incomplete.

Current documents, explicit readers and detached preparations may legitimately pin
all decoded capacity. This coordinator cannot evict those allocations. Logical
blob authority, on-demand current-document residency, encoded/native/GPU resources,
adaptive startup ceilings, metadata bounds and process RSS accounting remain.
Physical device/driver faults, native memory pressure, queue stalls and p95/p99/max
interaction need broader qualification. No throughput, RSS or tail-latency gain is
inferred from these correctness checks. T04/T11/T13 remain partial.
