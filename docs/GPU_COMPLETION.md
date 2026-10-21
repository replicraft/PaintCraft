# Quarantine after uncertain GPU completion

All compositor, batch, scaled clear/reduction, presentation and explicit readback
waits pass through one completion boundary. A failed `Device::poll` retires the
shared `DeviceEpoch` before returning `CapabilityUnsupported`. Both timeout and
invalid-submission errors follow that policy. Readback callback timeout/disconnect,
mapping failure and inaccessible mapped views also retire the epoch. Existing
unmap guards still release readback mappings on every exit.

Retirement is logical and irreversible. It prevents future PaintCraft submissions,
frame registration and readback through that lifetime, including already returned
frames that share the epoch. Renderer entry-point errors clear source entries and
idle private output retention. A failure through an independently retained frame
invalidates the epoch immediately; renderer-held cache/pool storage is cleared on
its next rejected render or Drop. No failed private output is marked recyclable.

Successful waits do not retire an epoch. Caller cancellation after a completed
wait, host output-allocation failure and ordinary input/resource admission errors
remain recoverable without declaring the device lost. The GPU retains no authority
over committed document/history data. Reconstruction uses host-backed pixels and a
fresh device lifetime, never a reactivated old token.

## Native boundary

The native coordinator observes epoch retirement, releases its native texture
registration and compositor handle, invalidates pending render identity and prepares
CPU output from the current host state. Stale device completions cannot disable a
replacement or publish pixels. CPU fallback still requires a working native
presentation device. Quarantine does not recreate eframe's window/surface/device or
repair a hung driver.

## Executed evidence and limits

A real Metal test obtains an invalid submission index from a second device and
uses it in the shared wait boundary. Unlike explicit destruction, this exercises
an actual poll error without relying on a device-lost callback. It verifies epoch
retirement, rejection of an old frame and subsequent renderer calls, source/pool
cleanup, unchanged host pixels and exact reconstruction on a fresh device.
Device-independent policy tests cover timeout/invalid-index errors and readback
callback timeout/disconnection. Existing full GPU tests cover normal completion,
recoverable cancellation, batching, scaled reduction and texture ownership.

A real native-renderer test retires the shared epoch after registration, checks
that CPU pixels and editor publication identity are preserved, and completes an
actual managed-texture upload using the still-working presentation device. This
tests coordinator routing, not physical adapter removal or automatic native recovery.

A failed wait is not proof that submitted GPU work stopped. Quarantine prevents
new PaintCraft submissions but does not establish physical resource reclamation or
transfer an ended worker's declared charge to an outstanding-driver lease. Timeout
retirement is policy-tested, not tested by deliberately hanging hardware. Outstanding
work accounting, unexpected driver reset, native device reconstruction, all-service
physical budgets and input/display tail latency remain open. No performance gain
is claimed; the successful numerical path and rendering defaults are unchanged.
