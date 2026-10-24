# Native printing helper

The platform crate's optional `printing` feature compiles `Print.swift` with
Xcode's Swift compiler on macOS. Other platforms do not compile or embed this
helper. The target architecture follows Cargo; the deployment target is macOS 13.
No additional Rust dependency or unsafe Rust is required.

The compiled helper is embedded in the Rust executable. Each requested print uses
a private temporary directory containing the helper and a captured PNG. Paths and
the job title are passed as separate process arguments. No shell is involved.
The worker retains the directory until the native operation ends, then removes it.
The packaged app needs no additional print-helper resource or runtime compiler.

The helper creates an ordinary AppKit `NSPrintOperation` for the image, with native
preview, printer, page size, orientation, scale and PDF controls. A subclass of
`NSPrintPanel` records its modal response to distinguish user cancellation from
an unsuccessful operation. Successful completion means AppKit accepted/completed
the operation; it does not certify physical delivery of paper by the printer.
A captured image fits on one page by default, centered at 96 document pixels per
inch before the native panel's settings are applied. Persistent page setup,
reference-specific pagination and cross-platform printing are separate work.

The service admits at most one request, up to 64 MiB of encoded PNG and
16,777,216 decoded pixels. Titles are bounded to 4096 UTF-8 bytes. Both Rust and
the helper check PNG dimensions before native image decoding. The app must supply
a captured composite; printing never mutates the document or its saved state.

`--validate-only` validates and decodes an image without constructing an
application, print panel, print operation or print job. Platform tests run this
mode, verify private directory cleanup and argument boundaries, and use simulated
runners for cancellation, errors, panic recovery and single-flight ownership.
Tests never invoke physical printing or display native print settings.

AppKit contracts are documented in
[NSPrintOperation.run](https://developer.apple.com/documentation/appkit/nsprintoperation/run())
and [NSPrintPanel.runModal](https://developer.apple.com/documentation/appkit/nsprintpanel/runmodal(with:)).
