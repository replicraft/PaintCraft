# Internal browser codec Worker

This thin Wasm binding calls the same `paintcraft_formats::codec_worker::execute`
implementation and protocol2 as the native helper. It initializes no editor, GPU,
UI, filesystem adapter or second decoder engine. Host workspace checks build an
empty binding module; `wasm32` builds expose the actual Worker functions. HEIF
writing remains prohibited. Browser HEIF/JXL reader workflows remain disabled;
native Studio JXL explicit imports are qualified separately.

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
response, constructor attempt, message failure or trap), the module enters an explicit quarantine:
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
No Playwright package is shipped or added as a runtime dependency. The primitive smoke
uses private localhost/Chrome profiles without starting the editor or using the
user's browser data.
The normal production build now bundles the real lean Worker and Wasm in Vite's
hashed asset graph. `main.js` creates a typed service object; `WebHandle` checks
its captured Rust application version/protocol before retaining it. This avoids
wasm-bindgen snippet copies with unresolved relative module imports. There is
still no public image-picker gate or document adoption workflow.

The app route's production CSP permits same-origin module Workers and Wasm
compilation while blocking blob Workers. It does not apply to `/download`, whose
release lookup and automatic download remain independent. Actual headless Chrome
production checks verified the six independent RGBA samples, unsupported-depth
refusal, version checks, memory growth refusal, CJK startup, cancel/stale quarantine,
PCND download bytes while quarantined, and the desktop download page. The recorded
2026-10-26 build contains 25 assets (largest 18,873,157 bytes) and a 1,734,486-byte
lean Wasm module; these are measurements, not immutable size promises. Browser/CDP
metadata and raw results live locally in ignored
`docs/validation/codec-worker-2026-10-26/production-proof/attempt-*/production.json`,
with source/artifact hashes, per-attempt source snapshots and failure records.

Run the actual production check after `pnpm --dir apps/paintcraft-web build`:
set `PLAYWRIGHT_MODULE` to the existing Playwright module and run
`node apps/paintcraft-web/scripts/codec-production-browser.mjs`. It uses a private
headless browser/server and no user profile. It reports public reader and
execution-stop qualification as false. This does not replace or weaken the
failed execution-stop regression above.

File-backed admission checks File.size before arrayBuffer and freezes the owner
before any file read. The logical declaration includes source/capture/request
copies, response/GUI-pixel copies and padded tile handoff space. A stuck File read
or WebCrypto digest cannot be joined or stopped by this API: logical cancellation
or deadline returns promptly but quarantines the charge until realm teardown.
Known completed read failures may release; constructor attempts are uncertain.
Canonical codec refusals release only with validator-owned proof for that exact
captured request. Public Error flags, abort reasons and replayed refusals cannot
forge that proof, and held Errors retain only an opaque token, not source pixels.

Resident adoption, user-facing reload/discard policy, actual picker integration
and workflow failure tests remain necessary before any browser reader is enabled.
