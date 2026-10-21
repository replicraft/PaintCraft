# Native editing preparation and file ordering

A real desktop window connects every editor to one shared `paintcraft-runtime`
executor. It has one worker and admits one transaction across all open documents,
including completed results waiting for their owner. Creating or switching tabs
and frontend workspaces does not create another editing pool. Deterministic
controller/GUI harnesses keep synchronous dispatch unless they explicitly connect
this service.

## Preparation and publication

Accepted activation tokens enter the controller outbox once, then a FIFO action
queue. The next action constructs a typed batch against the latest published
session and captures a detached timeline. Command execution, automatic history
packing/spill and cold history restoration run on the worker. Only the originating
controller consumes its job handle and publishes against the captured session,
epoch and resource policy. Other tabs cannot consume its completion. Runtime
reservations remain charged while results await consumption.

Document, selection, history and epoch change at the editor publication point.
New-layer aliases and target repair apply after successful publication. A failed,
cancelled or stale result leaves the published state intact. Released stroke,
shape and text previews remain available until their transaction finishes. Text
previews retain their captured layer independently of live text entry or the current
editing target. Queued menu commands still execute in order.

Pencil, brush and eraser gestures may continue while earlier paint-only strokes
prepare. A compound preview applies the in-flight stroke, accepted queue prefix
and live gesture exactly once in FIFO order against the published base. When a
stroke publishes, it leaves that prefix and the preview rebases on the new document.
Each changed stroke remains its own transaction and undo step. Other pending
commands, including structural edits, selection, fill/text and history navigation,
and file barriers pause additional painting with a progress cursor. Speculation
never crosses their layout/publication boundary.

Native painting admits at most 32 pending/live strokes, 262,144 points and 524,288
slots in their captured point vectors. Admission includes queued, in-flight and
live paths. Vector growth prepares a fallible replacement before changing the
captured prefix and checks actual Vec capacity against the remaining allowance.
Excess movement reports its limit while preserving the prefix for release; full
queues pause new gestures. Duplicate activation tokens do not become new admissions.
Fixed-point conversion failure preserves the live brush and allows valid movement
to continue. Preview and command copies are additional allocations outside these
logical input counters; they do not establish a process-wide physical-memory bound.

Escape cancels private preparation and queued editing activations, preserving
already published work and file-request barriers. Cancellation checks reach
history source classification, transfer, synchronization and tile restoration.
Individual command kernels, codec calls and filesystem calls are not preemptible.
Dropping a controller abandons its pending job. The runtime cancels it and retains
admission until actual completion; polling reclaims abandoned results even when
no documents remain open.

## File boundaries

A native save/export request inserts a document-owned barrier after its preceding
activations. Later activations remain behind that barrier. A save captures the
published immutable document at the barrier, submits it to the existing ordered
native I/O coordinator, then releases the barrier. Editing may advance while I/O
runs. A save acknowledgement names its captured state and does not make a later
edit clean. Distinct barriers preserve distinct capture positions; repeated layout
passes do not duplicate an activation.

Export planning and confirmation keep the barrier until the user finishes or
cancels the modal, so later queued edits cannot change the pixels being confirmed.
The modal activates its originating tab. Inactive tabs retain their own deferred
requests and receive their own worker/I/O results. At most four file requests per
document may wait in this coordinator; excess requests report rejection.

Tab/window close and Classic/Studio switching wait for accepted edits and file
requests before deciding which work needs saving. Explicit document replacement
can abandon private editing after discard, but cannot silently remove queued file
requests. Saves made without the native I/O service retain their synchronous
fallback while respecting the same barrier and captured-state acknowledgement.

## Validation and limits

Runtime tests exercise targeted consumption, reservation retention, foreign and
expired handles, cancellation and abandoned consumers. Controller tests cover
FIFO activation, alias/target publication, stale history clicks, preview retention,
cancellation, competing publication and file barriers. Paint regressions verify
identical translucent strokes, a live stroke spanning an earlier native spill and
publication, capacity/point rejection preserving prefixes and fixed-point failures.
Application tests reopen real PCND files to verify preceding/later-edit boundaries
with native and fallback I/O,
route inactive-tab captures, wait through close/switch gates, and decode an export
held at its confirmed revision. A real egui two-pass input test pumps background
publication and checks the final document and displayed pixel exactly once. A
second two-pass case delivers three coalesced strokes plus a live gesture across
later frames, verifying ordered translucent preview/final pixels and undo depth.

The fixed 2 GiB preparation charge is provisional scheduling admission, not a
physical allocation reservation or process RSS bound. Captured timeline metadata,
command/batch construction, font masks, publication and destruction still involve
the UI thread. Flat export planning/encoding/writing keeps its existing synchronous
path. Non-paint activation payloads have no global byte admission policy yet. Input
counters and fallible vector admission do not cover allocator overhead, preview
copies, native codec state or all other sessions. Priority/fair scheduling,
process-wide memory accounting, finer cancellation and native
queue-inclusive input/p95/p99/max latency qualification remain. No performance
improvement is inferred from the correctness tests or the startup smoke.
