# Canvas clipboard Copy

Copy now works in native Classic 7/10/11 and Studio and in browser Classic.
Use Edit > Copy or Ctrl/Cmd+C; historical Classic skins also expose the ribbon
Copy button. Focused text widgets and text drafts keep keyboard ownership.
Cut, Paste and Copy visible layers remain disabled pending their own transactions.

## Captured pixels

Copy takes the active layer's raw straight RGBA8 samples. An inactive selection
copies the canvas area. A rectangle or mask copies its exact tight canvas-clipped
bounds. Mask holes and samples outside the finite layer become transparent black;
selected samples retain their exact alpha and hidden RGB. Layer opacity and other
layers are not composited. An empty field cannot replace the clipboard.

The document, selection, clean state and history remain unchanged. Native capture
pins an immutable source revision and runs on a separate thread. Copy refuses an
unfinished canvas gesture, floating selection, text draft or pending edit rather
than placing private pixels. The source remains fixed if later edits or tab
changes occur while the write is pending.

## Acknowledgment and ownership

Native writes use arboard on one persistent worker. Successful completion means
`set_image` returned successfully. The backend remains alive to serve clipboard
ownership on platforms that require it. One outstanding request includes its
unconsumed acknowledgment. Cancellation or shutdown during an opaque OS call
reports failure even if that call already changed the clipboard; this deliberately
cannot authorize deleting source pixels for a future Cut operation. Panicked
backends are not reused. Native completion status belongs to the originating tab.

Browser writes offer PNG through `navigator.clipboard.write`. The call registers
an unresolved Blob promise before deferred PNG encoding, retaining transient user
activation. The promise acknowledgment determines success. Unsupported contexts,
permission rejection and encoding failure produce an error instead of a success
message. Secure-context and browser permission policies still apply.

Only acknowledged writes replace the editor's exact internal image capture.
This internal bundle is separate from the external OS representation. Native
clipboard formats or receiving applications may normalize alpha/color; successful
write acknowledgment does not establish cross-application pixel fidelity.

## Limits

Shared images admit dimensions, pixel count and vector capacity before capture.
The per-image limit is 16,777,216 pixels and 64 MiB. Image clones share one payload
reservation. Cancellation/source failures release private allocations. Tight mask
bounds use the existing 2,097,152-work guard and never substitute a bounding-box
approximation after denial. Native captures share a 128 MiB process budget for
retained clipboard pixel capacities.

Browser capture retains its existing 4,194,304-pixel document limit and a 64 MiB
capture budget; PNG output has a separate 32 MiB size check. These are declared
payload policies, not bounds on OS libraries, codec scratch, allocator overhead
or physical process memory. Native OS calls and individual codec calls are not
interruptible. No copy-speed or physical-budget qualification is claimed.

## Evidence and remaining scope

Core tests cover capacity-based admission, shared final-reader lifetime,
malformed input, read failure and cancellation. Independent editor oracles cover
rectangles, concave/inverted masks, holes, negative origins, hidden RGB, finite
layer boundaries, quotas and every observed cancellation checkpoint. Native
service tests use injected backends to cover write failure, cancellation,
shutdown, panic, retained ownership, busy state and completion/drop races.

Native host tests cover frozen asynchronous capture, one in-flight operation,
failed writes retaining the previous internal capture, tab ownership, draft
preservation and Copy-event/key deduplication. Actual controls run across Classic
7/10/11 and Studio in English/Japanese with fake acknowledgments; they never
write the OS clipboard. Browser host tests cover raw capture, deduplication,
acknowledgment/failure and text ownership. PNG adapter tests roundtrip hidden RGB
and partial alpha independently of screenshot pixels.

A fresh release browser build was exercised through actual Edit > Copy clicks in
Chromium headless shell 156.0.8078.4. Its isolated in-memory clipboard
was established from the exact matching Chromium source before testing. The first
write acknowledged success and yielded a 1000 × 486 PNG containing the drawn brush
mark. A second write with permission denied produced `NotAllowedError` and the
visible failure message. No unhandled browser exceptions occurred; the document
remained dirty. This opaque-image runtime fixture does not replace the portable
hidden-RGB/partial-alpha tests. The test context, browser and loopback server were
closed; the user's OS clipboard was never read or changed.

Raw logs, the exact headless-shell isolation sources, screenshots, decoded pixel
counts and the rerunnable runtime harness are local in ignored
`docs/validation/clipboard-copy-2026-10-24/`.

Paste needs admitted external pixels, floating placement and one atomic undoable
transaction. Cut needs acknowledged clipboard ownership before a source-bound
clear transaction. Copy visible layers, a CLI clipboard command, external flavor import, large-image
placement policies, Safari/Firefox, actual OS cross-process behavior, Windows
DIB/PNG alpha, Wayland/X11 and matched native Paint/Paint.NET behavior remain
unqualified. This tranche does not complete the clipboard or frontend roadmap.

## Completed validation, October 24, 2026

The unified workspace sweep recorded 1,504 passing tests and 75 explicit ignores
across 127 result summaries. Two older GUI selectors failed because they required
Copy's former disabled label without a shortcut. The tests now locate the enabled
Copy control while retaining all spacing, icon, contrast, scrolling and document
invariants; both targeted reruns pass. The original sweep and corrected rerun are
retained separately rather than relabeling the sweep as an all-pass run.

Strict workspace all-target Clippy, Wasm Clippy, formatting, the 800-line source
limit and 938 message mappings across seven languages pass. A fresh release browser
build passed the isolated runtime checks above. A fresh debug macOS bundle passes
strict deep signature and plist verification. Compiler disk exhaustion during
intermediate validation was resolved by deleting only disposable native incremental
caches; the successful retries disabled incremental compilation. No source assets,
user clipboard contents or installed application preferences were changed.
