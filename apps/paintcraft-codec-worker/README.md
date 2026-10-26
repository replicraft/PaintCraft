# Native decoder isolation helper

This is a real single-request executable, currently used by headless qualification
tests. It is **not installed or invoked by public image-open routes**. HEIF writing
remains prohibited. A native helper alone does not qualify HEIF browser support.

The caller passes an explicit absolute executable path to the existing runtime
process supervisor. There is no PATH lookup, inherited environment, filesystem
source path, network address or shell command in the request. The versioned binary
protocol captures immutable bytes, their SHA-256, a random request nonce, document
session/state/epoch and frontend. Responses must match every capture and the
application version, then pass plane length/profile checks. These checks return
private native planes; no document, save path or source ownership is adopted.
The application publication thread must sample its current owner again.

The initial internal profile is a single base-layer monochrome8 IDR per coded
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

Remaining public-reader gates include native installation/signature/version
verification and packaging, OS containment policy beyond this Rust heap boundary,
actual WebWorker cancellation/memory parity, cross-platform qualification and
independent full-container/color/depth oracles. Merely compiling this helper does
not satisfy those gates. No release scripts or GUI entrypoints install it yet.

The transient reservation remains charged through child completion and until the
caller consumes/discards the result. It is **not** an automatic resident-budget
transfer: a consumer that retains decoded planes after result consumption must
first acquire an appropriate resident lease. Frontend installation/publication
must implement that handoff before retaining pixels. Windows uses the safe
`CREATE_NO_WINDOW` command flag; Windows device verification is still required
before claiming that all release launch paths avoid visible windows.
