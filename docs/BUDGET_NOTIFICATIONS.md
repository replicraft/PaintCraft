# Shared budget wake notifications

Native frontend contexts subscribe to the process-default decoded tile realm and
the shared in-flight worker realm. Another context's allocation pressure or
capacity release can wake an idle consumer so it polls its own recorded-history
maintenance and pending work. A notification contains no editing command or
foreign document state.

## Registration and coalescing

`SharedBudget::watch(headroom, wake)` returns an owned `BudgetWatch`. Headroom must
be positive. A budget holds weak registrations, with at most 64 live registrations.
Exhaustion returns `resource.limit`; dead registrations are pruned before new
registration and during event collection. Dropping the watch releases its callback
captures once already collected notifications finish. One final collected wake
may therefore arrive after unregistering. A budget does not pin closed frontend
contexts through a strong registration cycle.

Each observer has one pending wake bit. Events coalesce until `observe()` clears
that bit and samples current counters. The sample is live state, not a captured
allocation event. Multiple accepted strokes and file requests remain separate;
only wake notifications coalesce.

Events include entry below the requested headroom, releases while below it and
feasible reservation denial. Registering under existing pressure wakes immediately.
Requests larger than the entire ceiling do not wake consumers merely because
reclamation cannot make those requests fit. `wait_for_release(true)` additionally
wakes for releases above the threshold when a consumer is waiting for capacity.
Zero-byte releases do not manufacture an availability event.

## Shared known demand

`BudgetWatch::wait_for_capacity` publishes one feasible allocation-denial demand
for that exact realm. Foreign realms and requests above the ceiling clear the
interest rather than triggering reclamation. No bytes are reserved. Demand lives
in the existing bounded weak registration; clearing interest or dropping the watch
removes it, even while an already collected wake temporarily retains the observer.

`wait_for_capacities` combines a context's render and editor demands into that
same registration without allocating a watch for each tab. Editor demands retain
their own owner/base guards and clear on success, cancellation or stale ownership.

`observe_pressure()` acknowledges notifications and returns live usage plus the
maximum known demand across consumers. Concurrent requests are not summed: the
hint seeks enough headroom for a single attempt, without promising that multiple
attempts can proceed together. Explicit demand changes wake subscribers. Dropping
a watcher clears its demand immediately; existing pressure polls observe that
withdrawal without requiring a shutdown callback. Entry/release
notifications use the larger of static headroom and shared demand, including
releases above the original threshold. Unchanged interest does not generate wakes.
New subscribers observe existing demand too.

Snapshot collection and fanout remain bounded to 64 registrations. Observer and
callback-capture destruction stay outside the counter mutex. The pressure sample
is advisory and may be superseded by concurrent allocations or demand changes;
actual allocation still reserves under the normal ownership rules.

## Dispatch and ownership

The budget counter mutex is released before invoking callbacks AND before dropping
upgraded observer/callback captures. Fanout collection uses a fixed stack array;
resource release does not create an unbounded notification queue. Callback panics
are isolated so they cannot poison counters or prevent other collected observers
from receiving a wake.

Callbacks run synchronously on the producer thread and must only enqueue a poll.
The native callback requests an egui repaint; it never publishes an edit or
navigates another window's history. The lock guarantee concerns the budget mutex,
not arbitrary application locks held by its caller. Callback and application
allocations retain their own policies. Dispatch cost and the longest callback
segment still need measurement.

Charges retain their existing ownership rules. Completing a worker without
consuming its result does not return its bytes or announce a release. Actual
consumption/discard and final decoded-reader release can return capacity, wake
other contexts, and allow those consumers to retry admission under fresh guards.

## Activity hints for repricing

`SharedBudget::activity` advances for successful nonzero admission and shared
capacity release, independently of the visible usage values. Denials, observer
notifications, zero-byte leases and pool-internal recycling do not advance it.
A denial retains an observed stamp; its change check is advisory and conservative
at saturation. This distinguishes resource activity from repeated self-denial wakes
and detects a release/reallocation that returns to the same usage values.

Native waiting navigation uses this hint to replan cache-dependent demand on its
worker. A five-second fallback catches cache warming in other realms without an
activity event here. It is a provisional progress policy, not a latency guarantee.

## Native wiring and checks

Real native service initialization installs tile and worker subscriptions. The
tile threshold matches the pressure coordinator; worker headroom matches the
largest standard 2 GiB preparation charge. Polling acknowledges both observers.
Resource-failed rendering and pending edits enable additional tile-release wakes.
A current render or waiting edit's feasible decoded denial publishes its known footprint.
Every subscribed context uses shared demand when deciding whether to run its own
history maintenance. Success, stale document/preview identity and watcher shutdown
remove that demand. A render failure from another realm cannot pressure this one.
The existing periodic pressure polling remains as a fallback; a registration
failure is reported rather than silently claiming idle notification support.

650 workspace tests pass, with 21 explicit ignores. Strict workspace/all-target
Clippy, formatting/file limits, release desktop/CLI builds, three required Metal
parity/lifetime checks and an isolated 30-frame native startup/shutdown smoke pass.

Six core regressions cover pressure/waiting coalescing, initial pressure and denial,
reentrant counter observation and callback panic isolation, bounded/weak
registrations, concurrent release fanout, and unregistering during dispatch with
reentrant capture destruction. Three App regressions use two independent egui
contexts and real shared budgets to check cross-context wakes without document
changes, unread worker-result admission, and releases above the pressure threshold.
Unused harness texture deltas are explicitly cleared; these tests do not present
native windows or measure OS event delivery.

## Remaining qualification

This implements wake wiring, not request-sized admission, content-edit retry,
window scheduling guarantees or a global priority/fairness service. Real multiple
native windows, minimized/suspended windows, event loss, callback overhead, memory
pressure and queue-inclusive p95/p99/max interaction still need qualification.

Notifications cover only the subscribed declared-job and decoded-payload realms.
Codec, encoded, metadata, output, GPU and process RSS accounting remain incomplete.
A wake grants no resource, bypasses no publication guard and forces no pinned
reader to release its allocation. No throughput, RSS or latency improvement is
inferred from these correctness checks. T04/T11/T13 remain partial.


[Scoped metadata demand and native waiting](METADATA_WAITING.md) now retain
metadata-blocked edits with their original realms, FIFO position and file barriers.
Distinct metadata watches do not request decoded history reclamation. The known
attempt peak supports repricing; upfront batch planning and atomic metadata
reservations remain separate work.
