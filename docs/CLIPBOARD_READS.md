# Encoded native clipboard reads

The platform layer has a lazy asynchronous `ReadService` and an immutable
encoded-image snapshot. The macOS adapter reads advertised PNG or classic TIFF
without invoking an image decoder. The Windows adapter now captures registered
PNG (`PNG` / `image/png`), packed `CF_DIBV5` / `CF_DIB`, or standard `CF_TIFF` through safe bounded raw APIs.
The Wayland and X11 adapters on Linux/FreeBSD request encoded PNG or TIFF streams.
These adapters connect to [ordinary Paste](NATIVE_FLOATING_PASTE.md) and
[Paste as new layer](CLIPBOARD_PASTE_LAYER.md), including bounded decoding and
import-loss consent. A separate [browser reader](BROWSER_CLIPBOARD_PASTE.md)
supplies browser PNG data.

Arboard remains the native write service. Its 3.7.0 public image getter decodes
before returning pixels and cannot enforce PaintCraft's pre-decode pixel limit.
Encoded native reads instead feed the existing bounded codecs. [Packed DIB decoding](DIB_CLIPBOARD.md) now runs in formats after capture;
additional DIB variants and broader clipboard interoperability remain unfinished.
`ReadService::supported()` identifies compiled macOS/Windows adapters and named
Wayland or X11 sessions on Linux/FreeBSD; access can still fail at runtime.
Named Wayland sessions take priority over X11; acquisition failure does not
silently switch backends. Windows/Wayland native interoperability and X11
cross-application fidelity remain unqualified.

## Snapshot and admission

`EncodedImage` owns one vector and its capacity lease. PNG and classic little- or
big-endian TIFF signatures must match the advertised type. Signature checking is
not full image validation: the result remains untrusted encoded data. The caller
must still sniff/parse it under codec dimension/pixel/metadata limits, resolve
losses and freeze admitted RGBA before constructing a Paste draft.

For known-length providers, advertised length must be nonzero and no greater than 64 MiB. The snapshot reserves
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

## Windows source ownership

The Windows reader holds `clipboard-win::Clipboard` open across format discovery,
size queries and copying. It enumerates at most 257 IDs and rejects more than 256;
only actually enumerated registered PNG, DIBV5, DIB or `CF_TIFF` formats are
selected, in that preference order. Preferred representation errors do not
silently fall back to a lower-priority type. Native lock guards close the
clipboard on every result/panic path. This adapter does not empty or write it.

The safe `raw::size` API obtains the advertised allocation length before the
encoded capacity lease is acquired. The copy callback uses fallible exact reserve,
a fixed-length initialized slice and [raw::get](https://docs.rs/clipboard-win/5.4.1/clipboard_win/raw/fn.get.html), which copies into that slice rather
than growing an unbounded output vector. Short copies, a changed allocation size
or a changed sequence number reject the complete result. Final signature,
capacity and cancellation checks remain in `EncodedImage::capture`.

Windows may realize delayed clipboard content during `GetClipboardData`; its
[sequence number changes when that content is rendered](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber).
The reader therefore pins the sequence after the first size query has realized
the data, then verifies it after copying and immediately before returning. The
held clipboard lock follows the [native ownership rules](https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations).
Opaque provider work and native storage remain outside the Rust payload lease.

`clipboard-win` 5.4.1 was already locked as an arboard dependency. It is now a
Windows-only optional direct dependency, inherited from the root workspace. No
library version was upgraded, and workspace `unsafe_code = forbid` is retained.
No upstream implementation code was copied into PaintCraft.

## Wayland stream ownership

On Linux/FreeBSD, a nonempty `WAYLAND_DISPLAY` enables the Wayland reader. It
uses the already-locked `wl-clipboard-rs` 0.9.4 and its
[typed clipboard pipe API](https://docs.rs/wl-clipboard-rs/0.9.4/wl_clipboard_rs/paste/fn.get_contents.html).
It requests only `image/png`, then `image/tiff` if PNG is not offered. Acquisition
errors and malformed preferred data stop the operation. No text or X11 fallback
is requested. A compositor without the required data-control protocol can reject
the request. Each MIME attempt acquires its own offer; bytes from different
attempts are never combined.

Unknown-length streams reserve a private 64 MiB capacity grant before opening
the provider. A 32 KiB scratch buffer feeds fallible geometric vector growth
within that grant. The exact-limit boundary probes one excess byte without
growing storage. Only EOF plus signature validation freezes the snapshot; excess,
read errors and cancellation discard it. The final payload retains only its
actual vector-capacity charge, shared by clones. The unused grant is released.

Safe `rustix` 1.1.5 nonblocking pipe reads poll in intervals of at most 50 ms,
checking cancellation between waits. A transfer with no byte progress for 30
seconds fails. Initial Wayland acquisition/roundtrips remain opaque and cannot
be preempted through this library API. Offer/MIME metadata, provider memory,
protocol buffers, allocator reallocation overlap and codec scratch are outside
the payload lease; neither total RSS nor a whole-operation cancellation deadline
is qualified.

`Seat::Unspecified` provides a single-seat baseline, not verified active-seat
selection for multi-seat systems. Inherited `WAYLAND_SOCKET`-only sessions remain
unsupported by this adapter. X11-only sessions now use the separate reader below.
No clipboard read occurs until
an explicit Paste submission. The existing application pipeline performs bounded
decode, loss consent and source-owned atomic placement after transport.

## X11 selection ownership

When `DISPLAY` is nonempty and no named Wayland session is present, the
already-locked pure-Rust `x11rb` 0.13.2 client reads `CLIPBOARD`. It never requests
`PRIMARY`, text, files, conversion side effects or execution. No Xlib dependency
or unsafe workspace code is introduced. The implementation follows the
[ICCCM selection and INCR conventions](https://xorg.freedesktop.org/archive/X11R7.7/doc/xorg-docs/icccm/icccm.html#incr_properties).

Each explicit read has a fresh connection owning its invisible, unmapped
InputOnly requestor windows. Every conversion uses a different window, a fresh
server event timestamp and an initially absent property. Selection notifications
must match requestor, selection, target, time and property. The owner window is
pinned before target discovery and checked between conversions/data chunks and
before final return. If `TIMESTAMP` is advertised, one nonzero 32-bit integer is
captured and rechecked after image EOF. A changed/refused timestamp fails closed.
An absent timestamp permits owner-window checking only. This is not a universal
generation guarantee for a dishonest owner, same-window ABA changes or in-place
changes that do not update the owner's timestamp.

`TARGETS` is limited to 256 atoms. PNG is preferred over TIFF, which is selected
only if PNG is absent. Preferred conversion/signature errors never fall back.
Direct and incremental transfers are supported for image and metadata properties.
GetProperty requests retrieve at most 32 KiB, use word-aligned offsets and validate
actual type, format, item count, remaining length and partial-read continuity.
An INCR length is a lower bound, not an allocation request. Whole-property
deletion acknowledges a chunk; a correctly typed empty property terminates the
stream. Unaligned final image bytes are preserved. Limits apply to aggregate
bytes across all chunks, not just the first property or INCR hint.

The existing private 64 MiB stream grant precedes connecting or requesting data.
Only a complete signature-checked snapshot reaches the bounded decoder and
existing atomic Paste pipeline. Cancellation, refusal, malformed properties,
changed ownership and missing completion discard all partial bytes. Dropping the
connection destroys its requestor windows on every path. Event waits check
cancellation around 50 ms polls and expire after 30 seconds without the expected
event. Connection setup, synchronous protocol replies and flushes remain opaque;
the wait timeout is not a whole-operation deadline. Protocol reply buffers,
metadata, the chunk buffer and allocator overlap remain outside the encoded
payload lease. Whole-process RSS, cancellation tails and latency are unqualified.

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

Desktop macOS and browser read/decode/loss-consent/placement are now connected.
Windows PNG/TIFF and packed DIB transport is implemented and cross-compiled. Remaining: native
Windows/Wayland execution, additional DIB variants, richer flavors and
metadata, exact internal-copy matching, permissions and cross-application/native
reference qualification. This does not complete T17, frontend parity, browser
conformance or release qualification.


The initial Windows PNG/TIFF follow-up passed four deterministic transport tests covering the
preferred formats, frozen capacity ownership, delayed realization, length/sequence
changes, failed copying, cancellation, quota/size/type-count denial and malformed
preferred signatures. Its 74 platform host tests passed (two explicit native checks
ignored). Strict all-target Clippy passes for Windows x86-64 and ARM64 with the
clipboard feature, and for the macOS platform crate with all features. Cross checks
also exposed a pre-existing Windows-only unnecessary mutable directory builder;
the equivalent platform-specific directory creation now compiles without warnings.

The existing Windows x86-64 and ARM64 release jobs are configured to run platform
clipboard transport/lifecycle tests. That workflow has not run from this checkout. Native
Windows clipboard execution, delayed provider behavior and cross-application
PNG/TIFF compatibility are not proven by cross compilation or fake-source tests.
Packed DIB fallback is now connected separately; additional variants, color/DPI/
reference qualification, other OS readers and aggregate
physical-memory/latency qualification remain work. Evidence is local in ignored
`docs/validation/windows-clipboard-encoded-2026-10-25/`.

The later [packed DIB follow-up](DIB_CLIPBOARD.md) adds transport priority and
strict decoder/loss-consent evidence with updated test totals.

The Wayland follow-up passes 82 platform tests with all features (two opt-in
native checks ignored), including real Unix pipes, EOF/hangup draining, idle
timeout, cancellation through the asynchronous service, reservation release and
a successful fresh snapshot after cancellation. Shared stream tests cover
fragmentation, exact/excess limits, spoofed signatures, provider errors, every
observed cancellation point and final-clone capacity ownership. Strict host
all-target Clippy and Linux x86-64/ARM64 plus FreeBSD x86-64 cross checks pass.
These are transport/compile checks, not compositor, multi-seat, cross-application,
pixel/DPI or physical-device qualification. Linux/FreeBSD release jobs now run
the portable clipboard suite; those remote workflows have not run from this
checkout. Raw local evidence is ignored under
`docs/validation/wayland-clipboard-2026-10-25/`.

The X11 follow-up passes 87 platform tests with all features (four explicit
native checks ignored), strict host all-target Clippy, and Linux x86-64/ARM64,
FreeBSD x86-64 and Windows x86-64/ARM64 cross checks. Independent property fixtures
cover word offsets, final tails, incremental target lists, lower-bound and total
limits, changed formats/types/counts and partial-property mutation.

Two opt-in tests also pass against an isolated Xvfb server on macOS 14.7.2 arm64,
using only private UUID selection names. They exercise direct and incremental
100,011-byte image streams, incremental TARGETS, PNG priority, TIFF when PNG is
absent, malformed preferred data, refused conversion, changed owner/timestamp,
cancellation, exact transport bytes and shared-clone capacity release. These
synthetic encoded payloads test transport, not codec or native pixel interchange.
The server was extracted into temporary storage from official XQuartz 2.8.6,
with its upstream release asset digest verified. Test socket/helper paths were
relocated and binaries ad hoc signed; no installer/scripts, system integration,
visible windows, TCP listener or user clipboard were used. Task-owned server
files were removed afterward. Provenance and raw logs are ignored under
`docs/validation/x11-clipboard-2026-10-25/`.

CI now configures an isolated Xvfb run on Linux; that remote job has not run from
this checkout. Actual Linux/FreeBSD execution, clipboard managers, cross-process
application pixels/profiles/DPI and broader platform/release gates remain open.
