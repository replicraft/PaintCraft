# Native decoder isolation helper

This is a real single-request executable, currently used by headless qualification
tests. Complete native packages include it, but **public image-open routes do not invoke it**. HEIF writing
remains prohibited. A native helper alone does not qualify HEIF browser support.

The caller passes an explicit absolute executable path to the existing runtime
process supervisor. There is no PATH lookup, inherited environment, filesystem
source path, network address or shell command in the request. The versioned binary
protocol captures immutable bytes, their SHA-256, a random request nonce, document
session/state/epoch and frontend. Responses must match every capture and the
application version, requested codec and sample model, then pass plane length/profile checks. These checks return
private native planes; no document, save path or source ownership is adopted.
The application publication thread must sample its current owner again.

The HEIF internal profile is a single base-layer monochrome8 IDR per coded
image, including checked primary selection, direct grid order and auxiliary alpha.
All coded SPS dimensions, hvcC/pixi/ispe declarations, grid coverage, full-range
CICP and alpha depth/association must agree. ICC, display transforms, premultiplied
alpha, high depth, RGB/YCbCr and unqualified slice variants fail closed. Decoding
returns the decoder's native Y/opacity samples directly; optional RGB conversion
APIs are never called and no sRGB label is invented.

The selected pinned `heic-decoder` 0.1.1 parser/decoder executes **inside this
process**. Its audited SPS parser is a prerequisite, not a hostile-input sandbox.
PPS/frame errors and decoder panic do not run in the GUI. The helper starts a
single-thread Rayon pool; upstream source has no other explicit thread spawn.
The host supervisor bounds input/output, enforces cancellation/deadlines, kills
and waits for the child, and joins its two pipe threads before returning. Partial
output from a failed child is discarded. The trusted helper must not spawn
subprocesses or delegate inherited pipe handles; this supervisor kills the exact
child, not an arbitrary process tree.

A fixed 256 MiB allocator ceiling applies from process startup. This is **live
requested Rust heap memory**, not physical RSS: allocator overhead, thread stacks,
loaded code, mappings and direct native allocation are outside that metric. The
parent reservation includes this exact ceiling, captured input, maximum output
and protocol overhead. It never uses reported usage to pretend a smaller heap
ceiling was enforced. The response reports a high-water snapshot during response
preparation, not final process-exit RSS.

The helper's only unsafe module forwards `GlobalAlloc` to `System`, maintaining
checked CAS accounting for allocation, zeroing and reallocation. Rejected charges
never wrap or temporarily expose capacity; underlying failures roll back charges
and preserve valid old realloc pointers. Failure exits immediately with status75
through `_exit`, without allocating a message, invoking destructors or raising a
Rust OOM abort. The supervisor maps this status to `ResourceLimit`. Tests cover the
old subtraction-underflow race, overflow/refusal, system allocation failure,
reallocation growth/shrink, concurrent accounting and actual child exit/reaping.

The separate executable is necessary to keep its global allocator and narrow
unsafe policy exception out of GUI/CLI libraries. Workspace `unsafe_code=forbid`
is unchanged. This package explicitly denies unsafe except in the documented
allocator module and mirrors the other workspace lints. The reference pack allows
narrow audited, tested unsafe interfaces where necessary.

Remaining public-reader gates include installed helper discovery and integrity/
version verification at dispatch, actual installer/platform qualification,
OS containment policy beyond this Rust heap boundary,
actual WebWorker cancellation/memory parity, cross-platform qualification and
independent full-container/color/depth oracles. Merely compiling this helper does
not satisfy those gates. Public GUI/CLI image-open entrypoints do not discover or invoke it yet.

The transient reservation remains charged through child completion and until the
caller consumes/discards the result. It is **not** an automatic resident-budget
transfer: a consumer that retains decoded planes after result consumption must
first acquire an appropriate resident lease. Frontend installation/publication
must implement that handoff before retaining pixels. Windows uses the safe
`CREATE_NO_WINDOW` command flag; Windows device verification is still required
before claiming that all release launch paths avoid visible windows.


## Native package layout and provenance

The release workflow builds desktop, CLI and codec worker in one locked Cargo
invocation for each target. Packagers require a regular, matching-architecture
sibling helper and inspect a referenced build record containing the helper's
actual Cargo version, protocol and fixed heap ceiling. Wrong/stale/missing build
records refuse packaging without executing foreign-target binaries. A matching
record is consistency evidence, **not a cryptographic identity guarantee**.
Runtime protocol/version matching and trusted installation validation are still
required before any decoder dispatch.

Portable desktop/CLI ZIP and tar archives carry the helper and
`paintcraft-codec-worker.json` beside the main executable. The macOS editor's
nested `Contents/MacOS` directory holds both; the helper is ad-hoc signed first,
then its signed-byte SHA-256 is written to the manifest before sealing editor and
outer bundle signatures. The exact signed app is carried in both DMG/update ZIP.
MSI installs both files beside `paintcraft.exe`; its disposable-runner verification
checks install/repair hashes and that uninstall removes helper and manifest.
Linux packages place the executable in `usr/bin` and manifest in
`usr/share/paintcraft`. AppImage verification checks both payload paths.

The manifest records the verified embedded version, target, helper SHA-256,
protocol, heap ceiling and an explicit disabled public-reader flag. Native
packaging tests use synthetic executable headers to inspect all six target
layouts and command payloads; they do not replace real Windows/Linux installers
or independent HEIF container interoperability. A real Apple Silicon helper has
also been stripped and ad-hoc signed locally, with its build record and final
manifest checked afterward. Developer-ID signing/notarization is not established
by that ad-hoc signature check.

The standalone Windows desktop `.exe` is intentionally helper-free. Copying only
a CLI executable, and current CLI `cargo install` scripts, also omit the helper.
These routes must not gain helper-dependent readers just because another package
ships it; missing/foreign/stale helpers must fail closed. Archive/install formats
retain their existing release names and checksum inventory.


## JPEG XL internal native profile

The same executable and supervisor now also handle an explicitly selected JPEG XL
request. Shared `paintcraft_formats::codec_worker` protocol2 uses `PCDEC002` /
`PCPIX002` envelopes and binds the response codec to the captured request. Typed
pixel payloads distinguish native HEIF gray/opacity from integer sRGB RGBA8;
responses cannot relabel either model as the other. Parent output reservation
covers four bytes per pixel plus bounded version/error framing for either codec.
The embedded helper build record and packaging manifest also require protocol2,
so an earlier helper refuses the new package contract instead of being invoked.

The pinned selected `jxl` 0.8.0 decoder is from
[libjxl/jxl-rs](https://github.com/libjxl/jxl-rs), source revision
`9a08ca643fd2d01f803dd0be6a5bebc9cc11e597` in its published VCS record,
BSD-3-Clause. The existing adapter uses the upstream event API, Main5 profile,
sample allowance, unpremultiplied output and our sequential task runner. Its
`BasicInfo` event makes original color profile/depth/alpha metadata available;
there is no custom allocator or comprehensive upstream scratch budget exposed.
This internal stage relies on fixed-heap subprocess execution for hostile input;
it does not establish a complete OS sandbox. ICC/header scratch may allocate **before** a subtype is refused.
No subprocess or production thread spawn was found in the decoder source; its
thread-scoped buffer splitter code is under the upstream test module. Its work
runner is sequential here, separate from the one-thread HEIF Rayon pool.

The initial JXL worker profile accepts original-profile, identity-oriented,
static integer RGB sRGB8 with optional straight alpha and no preview or discarded
container metadata. ICC, linear/HDR, 16-bit/float, XYB requiring profile conversion,
associated alpha, animation and non-alpha extra channels refuse before PaintCraft
allocates output planes. Existing broader internal adapter behavior is unchanged.
Native helper tests compare exact channels, hidden RGB and partial alpha against
checked-in independent `cjxl` 0.12.0 fixtures, also covering one pixel, malformed
inputs, explicit cross-codec rejection, limits, cancellation and deadlines.

This is internal native qualification, not public JXL Open support. Browser
WebWorker execution, installed-helper dispatch/integrity policy, broader adversarial
corpora and platform qualification remain separate gates. Existing JXL exports
retain their current qualified lossless writer subset.

## Installed native dispatch qualification

The native platform adapter now locates the helper only relative to the running
application: adjacent executable/manifest for portable CLI, desktop and nested
macOS editor bundles; Linux `bin` plus `share/paintcraft` manifest. It never searches
PATH or accepts a format-selected executable. The host supplies the format contract,
and the platform layer checks strict bounded manifest JSON, the released host
environment/target declaration, executable OS/architecture headers, actual
embedded build record, streaming SHA and retained file
identities. It rechecks helper and manifest before returning a launch path and
after the subprocess. These observed-byte consistency checks do not authenticate
a publisher, lock writers, provide an atomic filesystem snapshot or eliminate the
path-based spawn race. Installation files require the same trust as the app.

The hidden developer `codec-inspect` CLI command is a real consumer of that
verified path and the existing Executor/process supervisor. Use a complete
packaged CLI installation, not a helper-free standalone executable:

```sh
paintcraft-cli --json codec-inspect source.jxl --codec jxl
paintcraft-cli --json codec-inspect source.heic --codec heif
```

It admits fixed child heap, parent source/output and IO verification slack before
capturing source bytes from the acquired descriptor. Concurrent cloned identities
use positional reads. Source identity/hash and response owner/source/codec/version
are checked before a bounded diagnostic is reported. Hashes and native gray/CICP
metadata are returned without creating or saving a document or assigning HEIF an
invented sRGB interpretation. Pixel/source/admission/deadline limits and source
SHA expectations can reject the whole job; no partial result is reported.

Tests copy the actual CLI/helper into private installation directories and run
that copied CLI with independently encoded JXL/HEIF fixtures. They also reject
missing/stale/changed helpers, inadequate admission, source SHA conflicts and pixel
limits. Platform tests separately cover synthetic headers for all six release
targets, symlink/FIFO rejection, file identity replacement, bounded records and
concurrent same-descriptor captures. Workspace tests build the real helper;
focused CLI installation tests require it to be built first.

This is not public Studio Open/import activation. The hidden qualification
composition can now write a new PCND copy:

```sh
paintcraft-cli --json codec-import source.jxl --output converted.pcnd
```

It accepts only the worker's metadata-free straight sRGB RGBA8 profile, preserving
hidden RGB and alpha in one editable Image layer. Metadata, ICC, depth and other
unqualified variants are refused rather than silently converted. Receipts mark
this as internal qualification and a new PCND copy. Normal `import` still consults
the unchanged registry and refuses JXL reads.

The existing Executor reservation is retained by the prepared document through
PCND staging, final source/owner checks, publication and document destruction.
Its logical declaration includes the fixed decoder heap, captured source/output,
up to 64MiB of padded tiles plus 17MiB of tile-index/writer slack and 1MiB of IO slack;
this is not an allocation-enforced cap on the host/native writer or physical RSS.
The guard rejects captured-source aliases, changed source content, foreign owners
and helper failures. Installed-CLI tests verify the independent six-pixel oracle
and preserve sources/existing destinations on refusal. Public OS admission and
containment policy remain release gates.

Native
transport BUILD_V2 / STDIN_PARENT_LEASE1 keeps the supervisor's sole stdin writer
open after the exact bounded request; EOF, IO failure or an extra byte makes a
separate helper watchdog exit without allocating (status76). The request reader
releases its stdin lock before starting that watchdog. Shared binary protocol v2
and browser transport are unchanged. Packaging checks this native build record
and the matching parent-lease1 profile, rejecting old EOF-framed helper records.

Actual parent-process kill tests on this macOS host observed the sleeping child
close its inherited output after losing the pipe lease. The same test is portable
to Windows/Unix but has not been executed on Windows here. This is a liveness
mechanism, not a synchronous parent-death join or a descendant/job-object sandbox;
forwarded or inherited writer handles would invalidate its sole-writer premise.
While the parent is alive, supervisor cleanup kills/reaps then joins both pipe
workers even if a check callback panics. Requested heap is not physical RSS or
filesystem/process containment.
