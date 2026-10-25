# Browser printing

File > Print and Ctrl/Cmd+P print the visible document composite in Classic 7,
10, 11 and Studio. The browser owns printer/PDF destination, paper, orientation
and scaling choices. Standalone Page Setup and Print Preview remain disabled.
Native Windows/Linux printing remains unimplemented; physical printer output
remains unqualified.

The host prepares an immutable PNG copy within the browser's existing
4-megapixel canvas limit. Layer order, visibility, opacity and offsets use the
shared compositor. Completed draft text is included in that copy without placing
it in the editor or changing history. Active IME composition, unfinished shapes,
floating selections and pending paste placement must finish first. Printing
never marks a project saved or changes its name.

A dedicated same-origin iframe contains only a stylesheet and the captured image.
Its styles request white paper and a centered image bounded by the page area;
physical DPI matching and page-count fidelity are not qualified. The title is assigned as text, never inserted as
HTML. The helper waits for image decoding before calling the frame's `print()`.
No popup or editor toolbar is part of the printable document. Browser default
Ctrl/Cmd+P is suppressed by eframe's keyboard handling.

There is one active request. Its frame, image URL and listeners remain owned
until the print lifecycle closes or the application cancels it. Editor focus is
restored when `print()` returns and during cleanup. The waiting dialog offers
Cancel and Escape, including for a browser that cannot complete the request.
Destroying the editor also cancels its request. Image preparation times out after
30 seconds; an accepted open print dialog has no arbitrary expiry.

The [HTML printing steps](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#printing)
allow browsers to refuse a request and do not expose the user's eventual printer
or cancellation result. A silent refusal reports an unavailable/denied status.
`afterprint` reports only that the print dialog closed, never that a page was
printed. These statuses and waiting controls are translated in all seven locales.

## Evidence and limits

The browser host suite passes 132 tests. Print tests exercise exact layered
pixels and admission bounds, editable text preservation, IME refusal, cancellation,
unchanged dirty/history state, and actual menu/shortcut dispatch for both frontends
and all three Classic skins. The shared UI suite passes 49 tests with one explicit
capture ignored. A fresh full desktop GUI run passes 279 tests with 64 explicit
captures ignored, including both corrected menu checks and the historical Print
regression. All 16 JavaScript tests pass, including asynchronous/synchronous
printing, one-request admission, late decode fencing, cancellation, silent denial,
exceptions, image failure, preparation timeout, focus restoration and cleanup.
Wasm checking, strict web/shared-UI Clippy, Rust/Biome formatting, source-size
limits and 975 Fluent messages across seven locales pass. The production build
contains 22 assets, each below Cloudflare Pages' single-file size limit.

Isolated HeadlessChrome 156 exercised the actual production `print()` lifecycle
for blank and pencil documents. The shortcut's browser default was prevented,
the temporary frames were removed, focus returned, and printing retained dirty
state. Injected pending/denied requests exercised Escape cancellation and cleanup.
An independent Python PNG and binary-PCND decoder recovered exactly the same 192
black pencil pixels from the printable copy and a 140-byte version-1 download.
Opening that project through the real browser picker left it clean, updated the
title and produced a byte-identical subsequent save. No browser exceptions were
recorded. Evidence is local in ignored
`docs/validation/browser-print-2026-10-25/`.

This is browser lifecycle and captured-image evidence. A headless print call does
not qualify a visible printer dialog, physical output, browser-specific print
settings, mobile devices, Safari or Firefox. Browser conformance remains unfinished.
