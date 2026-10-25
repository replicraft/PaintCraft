# Encoded native clipboard reads

The platform layer now has a lazy asynchronous `ReadService` and an immutable
encoded-image snapshot. Its macOS adapter reads advertised PNG or classic TIFF
bytes without invoking an image decoder. Desktop/browser Paste controls remain
disabled until read results, decoder/loss admission and interactive placement are
connected to the [shared Paste draft](PASTE_IMAGES.md).

The existing arboard write service is retained. Arboard 3.7.0's public image getter
decodes before returning pixels; it does not expose PaintCraft's pre-decode pixel
limit. The new reader obtains encoded native data so the caller can use the
existing bounded codecs. Windows/Linux/FreeBSD encoded-read adapters and browser
clipboard reads remain unfinished. `ReadService::supported()` is true only for the
compiled macOS adapter; native access can still fail at runtime.

## Snapshot and admission

`EncodedImage` owns one vector and its capacity lease. PNG and classic little- or
big-endian TIFF signatures must match the advertised type. Signature checking is
not full image validation: the result remains untrusted encoded data. The caller
must still sniff/parse it under codec dimension/pixel/metadata limits, resolve
losses and freeze admitted RGBA before constructing a Paste draft.

Advertised length must be nonzero and no greater than 64 MiB. The snapshot reserves
that length from the caller's shared budget before invoking the byte-copy callback.
Actual returned length and vector capacity are checked; capacity beyond the
advertised length requires its own reservation. Clones share one payload charge
until the final reader drops. Debug diagnostics expose format/length, not payload
bytes. Mismatched lengths/types, denied admission,
cancellation and source errors expose no partial snapshot.

This accounts for Rust encoded payload capacity. Native pasteboard acquisition,
provider work, immutable native copies, allocator overhead and future codec scratch
are not physical-memory bounded by that lease. Opaque native copy/acquisition calls
are not interruptible; cancellation is checked before and after them. No throughput,
wall-clock cancellation-tail or RSS qualification is claimed.

## Native source ownership

The macOS adapter captures `changeCount`, obtains the actual advertised type list
and rejects more than 256 types. It prefers PNG over TIFF, requests only an exact
advertised type and checks the returned length before copying it into Rust-owned
storage. Foundation's immutable `NSCopying` snapshot and safe `to_vec` API retain
source ownership through the copy. The workspace `unsafe_code = forbid` policy
remains unchanged.

A changed pasteboard count before the final return rejects the captured result.
An invalid preferred PNG does not silently fall back to TIFF. No text, file-list,
image-conversion or execution fallback is attempted. The reader uses the locked
objc2/AppKit/Foundation versions already present through native dependencies;
these direct dependencies are conditional, optional and workspace-inherited.

After a successful capture, the Rust-owned bytes remain frozen independently of
later clipboard changes. This does not establish exact internal-copy matching or
cross-application alpha/color fidelity. Those remain separate clipboard work.

## Asynchronous lifecycle

Construction starts a worker but does not access the pasteboard. Only explicit
submission initializes and invokes its backend. Each read service accepts one
outstanding operation, including an unconsumed completion. Its generic `Job` shares
the existing write service's completion/retirement lease protocol.

Cancellation and shutdown suppress successful results, including operations whose
opaque native call has already finished. Dropping a job cannot release its slot
while the worker is still running. Failed initialization or reads permit retry;
a panicked backend disables that service. Wake-callback panics do not poison a
healthy backend. Shutdown does not block the UI on an opaque OS call.

Read and write services have separate queues. Their application integration must
serialize conflicting activations, retain one shared encoded budget and guard the
originating document/session before staging pixels. No ambient clipboard polling
or read is introduced by this tranche.

## Executed evidence and remaining work

Twenty-two portable/service tests pass, including the existing write lifecycle
suite. New tests cover capacity admission before the copy callback, final-reader
ownership, malformed lengths/signatures, every observed snapshot cancellation,
lazy off-thread execution, single in-flight ownership, initialization/read failure,
factory/backend/wake panics, cancellation during initialization/opaque reads or
after completion, job drop and shutdown.

The separately executed opt-in native test uses only a uniquely named private
pasteboard on macOS 14.7.2 arm64. It checks exact encoded transport, PNG/TIFF
priority, refusal of spoofed preferred data, changed ownership, denied quota,
cancellation, empty sources and excessive advertised types. Its synthetic payloads
have format signatures; they deliberately do not qualify pixel decode or native
image interchange. It clears that private pasteboard and never accesses the user's
general clipboard. No window is opened.

Strict platform/desktop all-target Clippy and Intel cross-compilation pass. The
standalone CLI's normal dependency graph still excludes arboard, AppKit, egui,
eframe and wgpu. No Intel runtime or Windows/Linux/FreeBSD read execution is claimed.
Logs are local in ignored `docs/validation/clipboard-read-2026-10-24/`.

Remaining: desktop read/decode/loss-consent/placement wiring, browser reads and
placement, other OS adapters, richer flavors and metadata, exact internal-copy
matching, unsupported-capability reporting in the frontend, permissions and
cross-application/native reference qualification. This does not complete T17,
frontend parity, browser conformance or release qualification.
