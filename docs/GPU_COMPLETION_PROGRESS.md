# Completion observation without canvas repaints

Native windows own a `paintcraft_render::gpu::CompletionDriver` independently of
whether their compositor is active. Render workers use `with_admission_progress`
to attach their existing paired local/shared charge. A successful submission wait
still releases submission ownership directly. Only failure or unwind transfers
ownership to a queue callback and wakes the observer.

The worker sleeps on a condition variable while no callback is pending. Pending
work calls `Device::poll(PollType::Poll)` outside the status mutex, then waits up to
16 ms before another attempt. This requests nonblocking polling; it does not promise
that an opaque driver cannot stall. No empty submissions, rendering, document edits
or UI repaints are required to make progress. No throughput improvement is claimed.

## Identity and ownership

Each renderer creates a private Arc identity, retained by its display frames and
observer. Submissions carry that identity. An observer accepts only its exact
identity; nested scopes search outward for a matching live observer. wgpu queue
comparison alone is insufficient: independent instances can allocate equal queue
IDs. Separately constructed renderers conservatively need their own observers,
even if a host gave them the same underlying queue.

Scopes and callback tickets reference observer state weakly. The worker retains
state and a device, but not the owning driver. Dropping the final driver signals
shutdown without joining on the UI thread. A callback retains admission ownership,
not the device, queue or driver. Thus queued callbacks cannot create a permanent
context/observer ownership cycle. The native app retains its driver through
compositor quarantine/CPU fallback; an active worker also holds a driver clone.

At most 16 observer threads are admitted process-wide. This is a bounded service
policy, not a measured optimal count or a physical memory budget. Spawn failures
return a resource error and native initialization reports degraded observation.
A poll error or panic closes the worker. `progress()` reports pending callbacks,
poll attempts and closed status. Shutdown does not wait for or cancel GPU work.
An exited worker returns its slot; a stalled driver call can continue holding it.

Without a matching live observer, `with_admission` behavior remains: callback
ownership is preserved, but host polling or context teardown must release it.
This is also the fallback if native observer startup fails. Completion callbacks
may run on the observer thread, so budget wakeups must remain nonblocking.

## Verification and limits

[The validation record](validation/gpu-completion-progress-2026-10-21/validation.json)
records source, commands and logs. Required Metal tests cover idle/successful
requests with zero background polls, real invalid-index failure with nested charges,
unwind after dispatch, mismatched instances, final-owner shutdown and pending
callback teardown. The failed-wait test drops the retired compositor while keeping
a queue handle alive; only the observer polls, excluding teardown as a substitute
for observed completion. Native worker tests use the driver, and native quarantine
tests verify it survives CPU fallback without editor publication changes.

No test deliberately hangs hardware. Thread exit and callback progress on this
healthy Metal device do not qualify physical hangs, driver reset or cross-platform
behavior. Admission describes declared job bytes, not opaque allocation/RSS.
Physical all-service budgets, asynchronous render/transfer execution, automatic
native device reconstruction and input/display tail qualification remain open.
