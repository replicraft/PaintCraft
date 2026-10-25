# Printing and image export

## Export

Desktop Export opens a format/options dialog before the native destination picker.
Supported outputs are PNG, BMP and JPEG. The dialog describes each format and
shows the current image dimensions. JPEG has a quality slider from 1 to 100; it
places transparent pixels on white. PNG and BMP retain alpha. Export flattens
visible layers; save a PCND project to keep editable document data.

The existing loss/replacement confirmation follows destination selection and uses
the actual returned extension. A native picker changing PNG to JPEG therefore
receives the appropriate JPEG warning. The chosen quality survives deferred edits
and confirmation. Cancel leaves the document and history unchanged. Exporting an
unbound project does not change the subsequent project-save destination.

Browser export offers the same formats and JPEG quality before preparing a
localized download. Downloads retain editable draft text in the editor and include
it in the downloaded copy. Neither host claims that an image copy preserves all
project data.

JPEG XL is not exposed by these frontend export dialogs. A separate internal
decoder/writer adapter has scoped pixel-oracle evidence; frontend integration and
codec confinement remain work. See [format interoperability](FORMAT_INTEROPERABILITY.md).

## Native macOS printing

File > Print and Cmd+P capture the committed visible composite as a PNG, then open
an AppKit print panel with printer selection, paper, orientation, scaling and
preview options. The capture waits behind earlier edits and belongs to its
originating document. Subsequent edits do not change an already captured job.
There is one active native print request per application instance.

An embedded Swift helper owns the print panel and operation in its own process.
It receives the PNG and title as separate process arguments, without a shell. A
private temporary directory holds the helper and image until the operation ends.
Print cancellation is reported separately from submitting a job or a failure.
The helper build requires Xcode command-line tools on macOS; the Swift runtime
ships with supported macOS systems. Rust contains no unsafe bridge code.

Capture is limited to 16 megapixels and an encoded image limit of 64 MiB. Printing
on native Windows/Linux remains unavailable. Standalone Page Setup
and Print Preview entries remain disabled; the working print panel contains its
own settings and preview. Printer-driver fidelity and physical output have not
been qualified.

## Browser printing

The browser now prints a private image copy through its own print dialog. Draft
text remains editable, the editor chrome is excluded, and cancellation or denial
does not change saved state. See [browser printing](BROWSER_PRINTING.md) for the
ownership contract, production evidence and qualification limits.

## Validation

Tests exercise locale/format combinations, export cancellation, chosen JPEG
codestream quality, deferred revision capture, destination handling, exact
PNG/BMP/project preservation, print payload ownership, single-flight admission,
cancel/failure/panic completion and malformed/oversized print rejection. The real
compiled Swift helper decodes a PNG in `--validate-only` mode, which creates no
application, panel or print job. No physical printer job is submitted by tests.

The initial workspace sweep passed 309 native unit tests (8 ignored) and 272
native GUI tests (63 ignored), with three GUI failures from expectations for the
previous export flow and disabled Print command. Those expectations were updated
and all three targeted reruns passed, including 42 historical export combinations.
The remaining workspace packages passed 768 tests (2 ignored) across 114 result
summaries with `cargo test --offline --workspace --exclude paintcraft`. These runs
cover distinct feature graphs; the figures are not a unique combined test count.
The original failing sweep and successful rerun evidence are retained separately.

Strict workspace Clippy, formatting, seven-locale catalog coverage, the 800-line
source check and the Wasm build pass. A fresh browser release distribution and a
fresh debug macOS bundle were built. The real nested macOS bundle passes strict
deep code-signature verification and plist validation. Finder cold/running native
event delivery passed using isolated dummy editors; no default-handler setting or
physical printer job was changed by validation. Raw local logs are in ignored
`docs/validation/file-integration-2026-10-24/`.
