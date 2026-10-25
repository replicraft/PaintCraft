# Encoded native clipboard reads

The platform layer has a lazy asynchronous `ReadService` and an immutable
encoded-image snapshot. The macOS adapter reads advertised PNG or classic TIFF
without invoking an image decoder. The Windows adapter now captures registered
PNG (`PNG` / `image/png`) or standard `CF_TIFF` through safe bounded raw APIs.
Both connect to [ordinary Paste](NATIVE_FLOATING_PASTE.md) and
[Paste as new layer](CLIPBOARD_PASTE_LAYER.md), including bounded decoding and
import-loss consent. A separate [browser reader](BROWSER_CLIPBOARD_PASTE.md)
supplies browser PNG data.

Arboard remains the native write service. Its 3.7.0 public image getter decodes
before returning pixels and cannot enforce PaintCraft's pre-decode pixel limit.
Encoded native reads instead feed the existing bounded codecs. Windows DIB/DIBV5
transport and Linux/FreeBSD read adapters remain unfinished.
`ReadService::supported()` identifies compiled macOS/Windows adapters; access can
still fail at runtime. Windows code is cross-compiled and remains unqualified for
native clipboard interoperability.

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

## Windows source ownership

The Windows reader holds `clipboard-win::Clipboard` open across format discovery,
size queries and copying. It enumerates at most 257 IDs and rejects more than 256;
only actually enumerated registered PNG formats or `CF_TIFF` are selected. Preferred
PNG errors do not silently fall back to TIFF or DIB. Native lock guards close the
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
Windows PNG/TIFF transport is implemented and cross-compiled. Remaining: native
Windows execution and DIB transport, Linux/FreeBSD adapters, richer flavors and
metadata, exact internal-copy matching, permissions and cross-application/native
reference qualification. This does not complete T17, frontend parity, browser
conformance or release qualification.


The Windows follow-up passes four deterministic transport tests covering the
preferred formats, frozen capacity ownership, delayed realization, length/sequence
changes, failed copying, cancellation, quota/size/type-count denial and malformed
preferred signatures. All 74 platform host tests pass (two explicit native checks
ignored). Strict all-target Clippy passes for Windows x86-64 and ARM64 with the
clipboard feature, and for the macOS platform crate with all features. Cross checks
also exposed a pre-existing Windows-only unnecessary mutable directory builder;
the equivalent platform-specific directory creation now compiles without warnings.

The existing Windows x86-64 and ARM64 release jobs are configured to run platform
clipboard transport/lifecycle tests. That workflow has not run from this checkout. Native
Windows clipboard execution, delayed provider behavior and cross-application
PNG/TIFF compatibility are not proven by cross compilation or fake-source tests.
DIB/DIBV5 fallback, color/DPI/reference qualification, other OS readers and aggregate
physical-memory/latency qualification remain work. Evidence is local in ignored
`docs/validation/windows-clipboard-encoded-2026-10-25/`.
