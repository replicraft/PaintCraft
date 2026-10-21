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
frontend workspaces and inactive tabs before the visible editor. It polls every
maintenance owner without activating that tab or frontend. Only the existing
shared worker slot is used, with the existing declared-byte admission ceiling.

Maintenance has a separate controller slot and never makes a clean document dirty.
Completed input remains queued while that owner's storage step finishes. A completed
maintenance step yields the shared worker to already queued foreground edits before
another step starts. This is a local admission-order rule, not general scheduler
priority or a preemptible latency deadline.

No-progress/failure gets a five-second per-ticket retry backoff. A changed logical
revision can try sooner. The event loop requests the next wake while pressure
persists. These thresholds are provisional engineering choices; native pressure,
interference and tail-latency measurements have not qualified them.

An installed render error caused by `resource.limit` records observed tile
residency. A later lower counter invalidates that failed render key so the same
document can render again without another edit. Successful renders clear the retry
marker. This prevents reclaimed capacity from leaving a cached failed canvas.

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
capacity than a single maintenance step releases, and a racing allocation can
still fail truthfully without publication. Request-sized pre-reservation, automatic
retry policy for failed content edits, cross-window pressure notification and
all-service fairness remain incomplete.

Current documents, explicit readers and detached preparations may legitimately pin
all decoded capacity. This coordinator cannot evict those allocations. Logical
blob authority, on-demand current-document residency, encoded/native/GPU resources,
adaptive startup ceilings, metadata bounds and process RSS accounting remain.
Physical device/driver faults, native memory pressure, queue stalls and p95/p99/max
interaction need broader qualification. No throughput, RSS or tail-latency gain is
inferred from these correctness checks. T04/T11/T13 remain partial.
