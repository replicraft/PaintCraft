# GPU completion ownership of admitted job bytes

Native render jobs now attach their existing runtime charge to synchronous
PaintCraft GPU calls. Successful waits release submission ownership normally.
If a wait fails or the call unwinds before observing completion, the submitted
work retains that charge through a queue-completion callback. Consuming the worker
result, cancelling it or dropping its executor cannot expose those bytes early.

## Runtime and service boundary

`paintcraft_core::types::resources::ResourceLease` groups a single or paired byte
lease with weak observation. Runtime pairs a pool-local lease with its existing
shared lease. `Cancellation::resource_lease()` explicitly upgrades the live group;
cloning a cancellation token alone does not retain capacity. Both local and shared
limits therefore remain effective while external work owns a charge. A pool can
have zero pending job handles and still report nonzero `reserved_bytes()`.

`paintcraft_render::gpu::with_admission` scopes that ownership to synchronous
PaintCraft GPU calls on the current thread. Native workers use `with_admission_progress` with a
[window-owned observer](GPU_COMPLETION_PROGRESS.md) inside their task, covering composition, scaled display and straight readback.
Scopes restore on normal return, error and unwind. Nested scopes retain every
enclosing charge, with at most 16 nested scopes. They do not propagate to spawned
threads/tasks or intercept arbitrary raw wgpu calls; those owners must explicitly
carry their admission.

Each PaintCraft submission captures the active scope before queue submission.
Observed successful completion drops that extra reference without registering a
completion callback. An unobserved submission instead transfers it to
`Queue::on_submitted_work_done`. Multiple references share the same declared charge;
they do not reserve bytes again. Failed waits still quarantine their device epoch.
Returned completed textures do not retain the task charge just because they remain
readable; persistent texture/cache ownership is a separate resource class.

Callbacks need a queue submission or device/instance polling to make progress.
Native window observers poll deferred completion without depending on repaints,
including after compositor retirement. Plain scopes, unmatched observers and
observer failures can still retain charges until external polling or teardown. Ending the queue
context releases its callback ownership; this is not proof that opaque driver
allocations were physically reclaimed. Callbacks release ownership only. Budget
observers must enqueue nonblocking wakeups rather than reenter GPU rendering;
callbacks can run on a GPU polling thread while derived-resource locks are held.

## Verification and remaining scope

[The validation record](validation/gpu-job-admission-2026-10-21/validation.json)
pins committed source, commands, compressed logs and release hashes, including the
preserved overflow-diagnostic development failure.

Runtime tests verify that an explicitly retained group keeps both limits after
result consumption, survives cancellation/executor Drop, and releases on final
ownership Drop. Cancellation tokens alone do not pin resources. Existing overflow
and retained-rejection contracts remain unchanged.

Real Metal tests verify that a failed invalid-index wait retains nested charges
after scope exit until queue completion is observed, that unwind after submission
does the same, and that queue-context teardown avoids permanent admission cycles.
Successful render/readback does not pin returned frames. Nesting-limit failure
restores the next request's scope and all declared counters. Existing required-GPU
desktop tests exercise the actual admitted worker integration.

This closes declared in-flight admission ownership for native compositor jobs;
GUI presentation and persistent textures/caches remain separate resource classes.
It is not an allocator/RSS ceiling, all-service physical GPU budget or priority
scheduler. Physical hangs/reset, native device recreation, other adapters and
input/display latency remain unqualified. No throughput improvement is claimed.
