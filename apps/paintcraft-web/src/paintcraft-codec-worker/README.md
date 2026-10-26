# Internal browser codec Worker

This thin Wasm binding calls the same `paintcraft_formats::codec_worker::execute`
implementation and protocol2 as the native helper. It initializes no editor, GPU,
UI, filesystem adapter or second decoder engine. Host workspace checks build an
empty binding module; `wasm32` builds expose the actual Worker functions. HEIF
writing and public HEIF/JXL readers remain disabled.

Build with `pnpm --dir apps/paintcraft-web run build:codec-worker`; the ignored
`codec-worker-pkg` contains generated bindings, Wasm and Cargo-derived build
metadata. An alternate `--out-dir` supports private qualification without touching
Astro's `dist`. The controller requires an explicit captured `applicationVersion`
from the real Rust application host when connected. Tests use the generated
Cargo metadata, never the independently maintained JS package version. Missing
or mismatched host/helper versions refuse before returning private pixels.

The final module is linked with a maximum **256 MiB linear memory**. The build
inspector validates the actual Wasm binary: exactly one defined, unshared Wasm32
memory with maximum 4096 pages, and no imported memories. Actual Chrome testing
also attempted to grow beyond the maximum and received `RangeError`. The first
optimized binding is 1,734,092 bytes, with 18 initial pages (1,179,648 bytes). These
figures apply to the recorded build, not an immutable size promise.

Linear memory is not physical RSS, native requested-heap accounting, or a total
browser memory cap. JavaScript ArrayBuffers, browser compiler/crypto scratch, native engine
objects and platform overhead exist outside it. Wasm responses explicitly report
zero for the native requested-heap snapshot; that value must never be interpreted
as Wasm memory usage. Rayon uses only the Worker's current thread for HEIF;
the JPEG XL adapter keeps its existing sequential runner.

`src/lib/codec-worker.js` admits one logical job/prepared result across controller
calls in that JS module instance. It freezes input before awaiting its digest,
bounds source and copied/transferred buffers, checks source SHA/nonce/owner/codec,
checks actual output dimensions/lengths/model and retains a conservative logical
reservation until `release()`. Separate module realms or duplicated module URLs
must be covered by the application's resident/admission policy before activation.
A retained document needs a resident lease **before** releasing its prepared
result. Failure returns no partial frame and requests Worker termination.
If stop/completion is uncertain (deadline, cancellation, stale owner, invalid
response, message failure or trap), the module enters an explicit quarantine:
it retains its whole logical charge and refuses subsequent jobs until realm
teardown. No timer clears quarantine or pretends to acknowledge reaping. Validated
success and canonical, identity-checked completed codec refusals have returned from
the decoder and may release on consumption; failure before Worker creation releases
immediately. This deliberately incomplete failure UX must be resolved before
public activation.

Crucially, `Worker.terminate()` exposes no join/reap acknowledgement. Cancellation,
deadline and stale-owner rejection do not prove physical
thread shutdown or memory reclamation. Host timers can also be delayed by a busy
or suspended browser event loop. Native kill+wait has a stronger lifetime boundary.
No hard aggregate browser resident bound, exact stop walltime or synchronous
termination guarantee is established here.

Actual headless Chrome 155.0.8059.85 (CDP revision
`0e66b94549a8bd85f03fbdf189b752d6e7b63c32`, V8 15.5.35.25) passed the independent
HEIF primary/grid/alpha and JXL RGBA8/single-pixel oracles, unsupported-profile
refusals, memory-growth rejection and protocol/owner checks. However, both an
immediate-stop assertion and a 2 second atomic-counter stabilization assertion
**failed**. Independent JS and actual Wasm infinite-loop diagnostics continued
advancing counters after `terminate()` returned; the longer Wasm observation
became stable only after roughly 20 nominal 100 ms polls. Exact poll timestamps were
not recorded in that first diagnostic, so it does not establish an exact tail.
The matching [official Chromium source](https://raw.githubusercontent.com/chromium/chromium/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/third_party/blink/renderer/core/workers/worker_thread.cc)
defines a 2 second forcible-termination delay, consistent with this observation.
The [HTML algorithm](https://html.spec.whatwg.org/multipage/workers.html#terminate-a-worker)
runs in parallel with the Worker and exposes no joined completion to the caller.

Failed assertions, original source snippets, full raw counter samples and browser
metadata are preserved locally in the ignored
`docs/validation/codec-worker-2026-10-25/browser` directory. They are not discarded
as flaky or replaced by a passing wider deadline. The explicit
`--verify-execution-stop` regression retains the failed execution-bound check;
the separate primitive smoke reports `executionStopQualified:false` and tests
only its stated lower-level guarantees.

Run Node checks with `pnpm --dir apps/paintcraft-web test`. For the explicit
headless smoke, set `PLAYWRIGHT_MODULE` to an existing Playwright ES module and
run `pnpm --dir apps/paintcraft-web run test:codec-worker-browser --pkg path`.
No Playwright package is shipped or added as a runtime dependency. Qualification
uses private localhost/Chrome profiles, never the editor or user's browser data.
The normal UI does not import this controller yet; production bundling/CSP,
installed-host version capture, resident handoff, disposition of post-terminate
execution and broader engine/platform qualification remain activation gates.
