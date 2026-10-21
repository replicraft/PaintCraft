# Shared worker byte admission

`paintcraft-runtime::SharedBudget` is one synchronized authority for declared
in-flight job bytes across multiple `Executor<T>` pools, including pools with
different result types. `Executor::with_budget` uses this authority in addition to
its local queue and byte limits. `Executor::new` retains an independent authority
with the pool's existing local byte ceiling.

This is scheduling admission for caller-supplied charges. It is not allocator
interception, decoded-tile residency, an exact estimate of task allocations, a GPU
memory limit or a process RSS ceiling.

## Ownership and release

A successful submission reserves shared bytes atomically. Overflow, local queue
exhaustion or shared exhaustion returns `resource.limit` before a job is inserted.
Failure does not change another pool's charge. `usage()` exposes the ceiling,
current reservation and observed high-water mark.

The reservation is one Arc-backed charge shared by the job slot and worker/message
lifetime. Cloning its guard does not charge the bytes again. Cancellation and
abandonment retain admission until work actually finishes and output is consumed
or discarded. Buffered completion results retain it until their consumer polls or
abandons them. Discarded output is destroyed before its final guard is released.

Dropping an executor cancels its jobs but cannot prematurely release a running
worker's charge. The worker still owns a guard; if its receiver is gone, sending
fails and destroys the result/guard. Queued completion messages also own guards,
so dropping the executor's job slots cannot expose their bytes prematurely. The
wake notification runs after the computation has completed.

Polling transfers a result to the caller and releases its transient job charge.
Persistent ownership after that boundary belongs to document/history/cache
services; it is not tracked by this in-flight authority. No decoded allocation
reclamation claim follows from a reduced reservation counter.

## Native integration

Real desktop edit and render pools share a process-local authority with a
provisional 3 GiB declared-byte ceiling. The existing local caps remain: one
2 GiB edit-preparation charge and a 1 GiB render-pool cap. Additional native App
instances use the same shared authority rather than multiplying its ceiling.
Harnesses get independent budgets so unrelated tests do not contend globally.
This initial ceiling is an engineering policy, not a measured resident-memory
requirement or an available-memory-derived physical budget.

The controller checks readiness before dequeuing an edit. Observable contention
keeps its completed strokes and ordered file markers queued. `can_submit` is a
readiness hint, not a reservation; a submission losing a race still returns a
truthful resource error without publishing. Atomic admission prevents overcharge
but does not implement fair reservation/retry ordering across UI contexts.

Rendering distinguishes requests that cannot fit any configured ceiling from
feasible requests blocked by current jobs. The latter retain current pixels and
request another repaint, rather than caching an error under the current render
key. An unconsumed result in another pool still blocks admission as required.
Existing paint count/point/capacity limits remain separate.

## Checks and remaining scope

625 workspace tests pass, with 21 explicit ignores. Six new runtime cases cover
heterogeneous pools sharing a cap, dropping a blocked executor, abandoned panic
cleanup, integer overflow/local rejection, ready-output destruction ordering and
eight racing pools competing for one charge. A desktop regression holds the entire
shared allowance, releases the worker without consuming its result, then consumes
it and verifies one queued-stroke publication and exact displayed/document pixels.
No test allocates 3 GiB merely to simulate a declared 3 GiB reservation.

Strict workspace/all-target Clippy, formatting/file limits and release desktop/CLI
builds pass. Required Metal parity/lifetime tests and an isolated 30-frame native
window smoke pass. This adds no dependency or measured performance improvement.

Physical tile/blob ownership, compressed/native codec buffers, I/O and recovery
services, persistent CPU/GPU caches, queued non-paint payloads, synchronous fallback
work, thread permits, priorities/fairness and process-wide allocator accounting
are not covered by this authority. Native latency/interference, memory pressure,
shutdown timing and per-charge estimation still need measurement. The broader
resource-controller and T13 acceptance gates remain partial.
